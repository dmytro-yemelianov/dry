# `dry` + Blender — round-trip workbench, Increment A: bridge + toolpath visualizer

**Date:** 2026-06-30
**Status:** Approved design — **preserved for later** (not yet scheduled for build)
**Branch (when built):** `feat/dry-blender-visualizer`
**Program:** Direction 1 (CAD-connected workbench) realized through Blender.

## Program context (decided with the user, 2026-06-30)

"dry + Blender" is a **round-trip workbench**, decomposed into three increments, each its own
spec → plan → build:

- **A. Bridge + Visualizer** *(this doc)* — load the `dry` wheel inside Blender, open a toolpath
  (raw `.gcode` or Dry IR), render it in the viewport, overlay verify findings. Read-only QA.
- **B. Authoring** — Blender geometry (curves / grease-pencil strokes / mesh edge-loops) → Dry Ops →
  `dry.resolve_*` → verify + emit g-code.
- **C. Round-trip glue** — author → verify-gate → `balanced` rewrite → re-import → re-visualize,
  iterating in one session.

Build order **A → B → C** (visualizer first: lowest risk, de-risks the bridge, ships standalone QA
value). **Hard scope boundary throughout:** we author and visualize *toolpaths/paths*, never slice a
mesh — consistent with dry's stated non-goals (`docs/00:49-51`, `docs/14:12-13`).

## Decisions (resolved with the user)

1. **Input:** Dry IR JSON **and** raw `.gcode` (the latter via a new `dry.import_gcode_ir()` bridge fn).
2. **Packaging:** a **Blender 4.2+ extension** that **vendors the cp39-abi3 `dry` wheel** via
   `blender_manifest.toml` (abi3 runs in Blender's bundled 3.11; drag-to-install, no pip).

## Architectural home

New top-level `blender/` directory, sibling to `py/` and `sdk/ts/`, **excluded from the Rust
workspace**. Depends only on the published abi3 `dry` wheel. **No `dry-core` changes** for any
increment — the engine stays pure; the wheel is the bridge.

## Two new Python bridge functions

Both are thin wrappers over **existing** pure dry-core public APIs; they keep `dry-core` untouched
and are reused by Increments B and C.

Add to `py/src/lib.rs` (+ `m.add_function`) and surface in `py/python/dry/__init__.py`:

```python
def import_gcode_ir(gcode_text: str, profile_json: str | None = None) -> dict:
    """Lift a raw slicer .gcode into Dry IR ({version, segments}). Profile (optional) supplies
    import params (filament diameter, line width, layer height). Wraps dry-core
    import_gcode_reader_with_map."""

def verify_ir(ir_json: str, profile_json: str | None = None) -> dict:
    """Verify ANY Dry IR (imported or authored) against contracts built from a dry profile
    (mirrors `review-gcode --profile`; no profile => structural rules only). Returns
    {findings:[{rule, severity, segment, message}]}. Wraps dry-core verify()."""
```

- `import_gcode_ir` wraps `dry_core::import_gcode_reader_with_map`; returns `tp.to_json()` parsed to a
  dict in the Python wrapper.
- `verify_ir` parses the IR via `Toolpath::from_json`, builds `Contracts` from the optional profile
  (reuse the existing `Profile`→`Contracts` mapping used by `review-gcode --profile`), calls
  `verify(&tp, &contracts)`, returns the `Report` JSON.
- Errors surface as `ValueError` (never panic), matching the existing bindings' convention.

## Data contracts (grounded in the engine)

- **Dry IR** (`crates/core/src/ir.rs`): `{ version, meta?, segments: [Segment] }` where a `Segment`
  carries `start:[x,y,z|null]`, `end:[x,y,z|null]`, `travel:bool`, `speed`, `length`, `volume`,
  `filament`, `width?`, `height?`, `kind` (`line`/`arc`/`spline`/`dwell`/`manual`), `centre?`,
  `clockwise`, and optional channels (`flow?`, `temperature?`, `fan?`, `tool?`, `orientation?`,
  `control_points?`, `dwell_s?`, `manual_gcode?`). The first segment's `start` axes may be `null`.
- **Verify report** (`crates/core/src/verify.rs`): `{ findings: [{ rule:str, severity:"error"|"warning",
  segment:int|null, message:str }] }`. `segment` is the offending **segment index** — markers map
  directly via `segments[finding.segment].end`. No source-line correlation needed.

## Add-on structure (pure core + thin bpy glue)

Mirrors the repo's "pure logic + thin binding" pattern so the logic is testable without Blender.

- `blender/dry_blender/core.py` — **pure, bpy-free, pytest-tested**:
  - `ir_to_polylines(ir: dict) -> list[tuple[list[Vec3], bool]]` — one polyline per contiguous run,
    flagged extrude vs travel; expands arcs from `centre`/`clockwise` (sampled to a fixed chord
    tolerance) and splines from `control_points`; carries forward the last known axis when a
    coordinate is `null`.
  - `findings_to_markers(ir: dict, report: dict) -> list[Marker]` — `Marker{position:Vec3,
    severity:str, label:str}` from each finding's `segment` index → `segments[idx].end`.
  - `metrics_summary(...)` — formats the simulate metrics for the panel.
- `blender/dry_blender/__init__.py` + operators/panel — thin bpy glue: a "dry" N-panel; an
  **Open Toolpath** operator (file picker for `.gcode` / `.dry` / IR `.json`) that calls the bridge
  (`import_gcode_ir` for `.gcode`, direct JSON parse for IR), runs `verify_ir`, and hands the results
  to `core.py`.
- `blender/blender_manifest.toml` — extension manifest, `wheels = ["./wheels/dry-…-abi3.whl"]`.

## Rendering

- **Toolpath:** a `gpu` + `gpu_extras.batch` **draw handler** renders the polylines each redraw —
  extrude = solid color, travel = dim/dashed. Fast, non-destructive, no scene clutter, scales to
  large toolpaths. (A future "bake to Curve objects" is deferred.)
- **Verify findings:** each becomes a Blender **Empty** (persistent, selectable, framable from the
  outliner), colored by severity (error = red, warning = amber). Listed in the N-panel with
  click-to-frame.
- **Panel:** metrics summary (time / material / peak flow) + the findings list + an `N findings`
  verdict line.

## Data flow

```
Open .gcode ─▶ dry.import_gcode_ir(text, profile?) ─▶ IR dict ┐
Open .dry/.json IR ──────────────────────────────────────────┤
                                                              ├▶ dry.verify_ir(ir, profile?) ─▶ report
                              ir_to_polylines(ir) ────────────┘        │
                                     │                                 ▼
                                     ▼                       findings_to_markers(ir, report)
                              GPU draw handler                         │
                                     │                                 ▼
                                     └────────── viewport ◀──── Empty markers + N-panel list
```

## Testing

- `core.py` transforms (`ir_to_polylines`, `findings_to_markers`, arc/spline expansion, null-axis
  carry-forward) unit-tested with **plain pytest, no bpy import**.
- The two bridge fns get py/Rust tests under `py/tests/` (round-trip a known `.gcode` → IR; verify a
  known-bad IR → expected findings).
- bpy glue (panel, operator, draw handler, Empty creation) verified manually; a headless
  `blender --background --python smoke.py` test is **CI-optional** (Blender is not in CI).
- Determinism is inherited — every number comes from the deterministic engine.

## Scope / YAGNI (deferred to B / C or later)

Authoring (geometry → Ops); the `balanced` rewrite loop; flow-as-thickness ribbon rendering;
path animation / scrubbing; an in-panel contract editor (Increment A sources contracts from a profile
JSON); non-planar / 5-axis orientation rendering; baking the toolpath to Curve/mesh objects;
visualizing the `dry explain`/forensics bundle.

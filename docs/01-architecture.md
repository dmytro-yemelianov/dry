# Dry — architecture

The system is a **multi-level IR with a Rust engine and thin multi-language front-ends**. Data flows
through dialects that lower into each other; passes transform within and between levels; the engine is
one Rust codebase compiling to native and wasm.

```
 authoring SDKs           the engine (Rust → native + wasm)            targets
 ────────────────         ──────────────────────────────────          ─────────────────────
 Python │ TS │ Rust  ─►   L0 design  ──lower──►  L1 path               FFF g-code (Marlin/Klipper/Duet)
   (emit L0/L1 Dry IR)              │                  │                 CNC (RS-274 / STEP-NC intent)
                                  │              ──lower──►  L2 motion  laser (GRBL)
 importers ─► g-code/3MF/STL ─►   │                  │  ▲              robot (per-vendor)
                                  │           [opt passes] │           3MF Toolpath
                                  └──────────────────┴─────┴──lower──► L3 target ─► machine code
                         simulate · verify · optimise · emit · parse · reverse
```

## 1. Dry IR — the multi-level IR

Not one flat structure: **dialects**, each a typed node set, with explicit lowering between them
(the MLIR idea — progressive lowering, each level verifiable).

- **L0 — design / feature dialect.** High-level intent: `Vessel`, `InfillRegion`, `Feature@pose`,
  `Repeat`, `Group`. Carries parameters + provenance. This is what an authoring SDK emits.
- **L1 — path dialect.** Parametric geometry with typed channels: `Line`, `Arc`, `Spline`, `Clothoid`,
  `Dwell`, `ToolChange`. Per-point/per-segment channels (§3). Still resolution-independent (a curve, not
  points).
- **L2 — motion dialect.** Resolved, machine-agnostic moves with absolute state — the equivalent of the
  fork's `Toolpath`/`Segment`, but **toolframe-general** and columnar. This is where simulate / verify /
  optimise operate. Most behavioural parity with the oracle is established here.
- **L3 — target dialect.** Machine-specific ops (g-code words for a flavor; STEP-NC working-steps; GRBL;
  robot motion). `emit` serialises L3 to text/bytes.

Each dialect has a **verifier** (well-formedness + declared invariants). Lowering L(n)→L(n+1) is a pass
that must preserve the level's invariants.

## 2. The toolframe (not XYZ)

Every motion carries a **toolframe**: `position: Point3<Length>` + `orientation: Rotation` (unit
quaternion; or an axis tuple for machine-native B/C). Planar 3-axis FFF is the **constraint**
`orientation == identity ∧ tool ∥ +Z`, not a special case. Consequences:
- **Non-planar** (z varies within a move) and **5-axis / robotic** (tilting toolframe) are *native*, not
  `lab/`-experimental.
- L3 lowering projects the toolframe to the machine's kinematics (3-axis: drop orientation; 5-axis:
  inverse-kinematics to B/C; robot: to joints).

## 3. Typed quantities & channels

**Units are types, not convention.** `Length` (mm), `Speed` (mm/s internally; emit converts to mm/min),
`Volume` (mm³), `Flow` (mm³/s), `Temperature` (°C), `Angle` (rad). Mixed-unit arithmetic is a *compile
error* in the Rust core and a typed wrapper in the SDKs. (Contrast: FC carried units as comments and a v2
header bolted on after the fact.)

**Channels** are the typed, *extensible* per-point/per-segment state a move carries, beyond geometry:
`extrusion` (deposited volume / filament), `speed`, `width`, `height`, `temperature`, `fan`, `flow`,
`tool`, `color`. Channels are a registry (new processes add channels without changing the core), each
with a type and a default-propagation rule. Variable-everything (width, flow, temp along a path) is the
*normal case*, not special-cased as FC's per-segment `ExtrusionGeometry` hacks were.

## 4. Pure functional core & the pass framework

**Authoring is `design(params) → L0 IR`, a pure function.** The deposition math + state propagation
(FC's running point, extruder e-ratio, primer injection) becomes an explicit **lowering pass**
(`resolve: L1 → L2`), *not* entangled with authoring. Benefits: deterministic, content-addressable
(hash the IR), cacheable, diffable, trivially parallel. No mutable global authoring state; no
`Date.now()`/`random` in the core.

**Passes** are the unit of transformation. Each declares: input dialect, output dialect, **preconditions**,
**invariants preserved**, and ordering constraints. Categories:
- *Lowering* (L0→L1→L2→L3): `expand_features`, `resolve` (deposition/state), `to_target`.
- *Optimisation* (within L2, semantics-preserving unless intended): `merge_collinear`, `arc_fit`,
  `simplify` (RDP), `travel_reorder` (TSP), `adaptive_speed`, `coasting`, `z_hop`, `retract_on_travel`.
  (Reimplemented; oracle = the fork's `ir/passes.py` + `gcode_engine/passes/`.)
- *Analysis* (no transform): `simulate`, `verify`, `reverse`.
A pass manager runs a declared pipeline; every pass is independently testable against its invariants.

## 5. Storage: columnar & streaming

L2 has both compact column-major storage and chunked streaming storage. The columnar shape
(struct-of-arrays, Arrow-compatible) stores start/end as N×3, plus
travel/speed/length/volume/filament/width/height/toolframe columns. The chunked form stores bounded
compressed row blocks so CLI analyses and emitters can consume large paths without inflating the full
archive body. Two serialisations of the same IR:
- **JSON** — human-readable interchange (versioned, with units/provenance/invariants header).
- **Binary** — `DRY0` compact columnar blocks for maximum compression and `DRY1` chunked blocks for
  streaming archive reads. `Toolpath::from_bytes` accepts both; `dry pack` writes `DRY1`.

## 6. Provenance & invariants (first-class)

- **Provenance:** every node records what produced it (design name + params, or source line for parsed
  machine code), making `reverse_engineer`/`identify` *exact* and enabling diff/blame.
- **Invariants / contracts:** designs *declare* contracts (`monotonic_layer_z`, `within_build_volume`,
  `no_cold_extrusion`, `bounded_flow`, `non_self_intersecting`, `bounded_overhang`, …); the verifier
  enforces them — checking is part of the *type/contract layer*, not a backend you remember to call.
  (Behavioural reference: the fork's `check_invariants` + `verify_gcode` rules.)

## 7. The engine API (Rust core, one surface)

```
lower(ir, pipeline)        -> ir           # run a lowering/optimisation pipeline
simulate(ir)               -> Metrics       # time, distances, material, peak flow, …
verify(ir, contracts)      -> Report        # invariants + rule checks, with locations
optimise(ir, passes)       -> (ir, Report)  # opt passes + before/after metrics
emit(ir@L3, target)        -> bytes/text    # serialise to machine code
parse(bytes, dialect)      -> ir@L2         # machine code → IR (inverse of emit)
reverse(ir|toolpath)       -> design        # toolpath → parametric design/params
```

One codebase, two builds: **native** (CLI, server, PyO3) and **wasm** (browser, TS). The fork already
proved a Rust kernel serving both PyO3 and wasm (`rust_kernel/` with feature-gated `python`/`wasm`
bindings) — evidence the approach works; Dry reimplements it clean-room.

## 8. Authoring SDKs (thin, logic-free)

Each SDK is a pleasant builder DSL that **emits L0/L1 Dry IR and contains no business logic** — the engine
lowers. So SDKs stay tiny and never drift (they all target the Dry IR spec + a conformance suite):
- **Python** — FC-flavored ergonomics for the existing community (migration path); now just a front-end.
- **TypeScript** — browser/Node native (the fork's `ts/` binding is the prototype: it already emits IR
  that the engine consumes, byte-matching Python to 1e-9).
- **Rust** — for embedding / high-performance generators.
Bindings call the engine via PyO3 (Python), wasm-bindgen (TS/browser), or directly (Rust).

## 9. Targets & interop (back-end dialects)

L3 lowering is target-specific and pluggable:
- **FFF g-code** — flavor vocabulary (Marlin/Klipper/Duet); ports the fork's `gcode/flavor.py` +
  `rust_kernel/src/gcode.rs` (byte-identical Marlin is the first parity gate).
- **CNC** — RS-274; optionally **STEP-NC (ISO 14649)** intent export (carry features/working-steps).
- **Laser** — GRBL.
- **Robot** — per-vendor motion (KRL/RAPID) from the toolframe + IK.
- **Interchange** — import/export **3MF Toolpath** (the strategic standard target), import **mesh**
  (STL/3MF) for hybrid design. (The fork's `ir/threemf.py` is the prototype.)

## 10. Behavioural reference map (clean-room — inspiration & oracle, not code)

Dry is **reimplemented from scratch** (proprietary and independent of GPLv3 FullControl—see
`CLEANROOM.md`).
The table below is **not** a "copy this code" map; it names, per Dry component, the FullControl module
whose *observed behaviour* Dry reproduces (the **oracle**, run at dev/CI to generate target outputs) and
*ideas* Dry takes as inspiration. No FullControl source is copied; the FC column is a reference only.

| Dry component | FullControl behaviour to reproduce (oracle) / idea (inspiration) |
|---|---|
| L2 motion dialect + columnar | the resolved-toolpath shape & deposition math (`ir/{toolpath,columnar}.py`, `walk.rs`) |
| simulate | the metric definitions & outputs (`metrics.rs`, `simulate/run.py`) |
| emit (FFF g-code) | the exact g-code bytes per flavor (`gcode.rs`, `gcode/{flavor,dialect}.py`) — the strictest oracle |
| parse (g-code → IR) | the round-trip behaviour (`parser.rs`, `gcode_engine/parser.py`) |
| optimisation passes | the pass effects & invariants (`ir/passes.py`, `gcode_engine/passes/`) |
| verify / invariants | the rule semantics & messages (`validate/`, `gcode_engine/rules/`, `ir/invariants.py`) |
| reverse-engineer | the recovery behaviour (`examples/reverse_engineer.py`) |
| serialisation (JSON+binary) | the format *idea* (Dry defines its own Dry IR spec) |
| TS front-end | the multi-front-end *idea* (`ts/`) |
| device profiles (~695) | regenerated from **primary sources**, not lifted; FC profiles as a cross-check only |
| conformance corpora | **generated by the oracle** (`docs/03-conformance.md`), not vendored from FC tests |
| web/wasm runtime + demo | the in-browser *idea* (`web/`) |

The architecture is therefore not a blank page **and not a copy** — it is an independent re-layering and
generalisation, *measured against* FullControl's behaviour until the diff is zero, then independent.

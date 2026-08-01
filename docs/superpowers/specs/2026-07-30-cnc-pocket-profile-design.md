# CNC pocket/profile vertical slice — design (P5.3)

**Date:** 2026-07-30 · **Task:** P5.3 (`docs/04-tasks.md`) · **Issue:** [#179](https://github.com/dmytro-yemelianov/dry/issues/179)
**Acceptance:** *a pocket/profile emits a valid CNC program.*

## 1. Problem

The `rs274` emit flavor and the STEP-NC sidecar landed, but P5.3's acceptance is not achievable
today for two independent reasons:

1. **No CAM operation exists.** There is no pocket or profile operation anywhere in the tree, so
   there is nothing whose emitted program could be judged.
2. **No program frame.** RS-274 output today is bare motion (`G0`/`G1`/`G2`/`G3` + `F`): no units
   (`G21`), plane (`G17`), distance mode (`G90`), WCS (`G54`), tool change (`T`/`M6`), spindle
   (`S`/`M3`/`M5`), or program end (`M30`). A real controller (LinuxCNC et al.) rejects or
   misinterprets such a file regardless of the geometry.

This slice closes both halves at once (decision 2026-07-30: *vertical slice*), so the acceptance
criterion becomes honestly true end-to-end.

## 2. Non-goals

- **No IR change.** No new `Op` variant, no new `Segment` field, no codec/spec-vector churn. The
  per-operation spindle/laser **power channel** is explicitly deferred to
  [#181](https://github.com/dmytro-yemelianov/dry/issues/181) (it should serve laser and CNC with
  one mechanism).
- **No general polygons, no islands, no cutter compensation (`G41/G42`), no helical/ramp entry, no
  zigzag/raster strategy, no stock model.** Rectangle + circle, contour-parallel only
  (decision 2026-07-30).
- **No new verify rules** (5-axis/CNC-specific rules are
  [#180](https://github.com/dmytro-yemelianov/dry/issues/180)). Existing contracts (bounds, speed
  range, finite, travel-extrudes…) apply unchanged.
- **No controller-in-CI.** Validity is gated by construction + tests against LinuxCNC's documented
  RS-274/NGC word set; a manual LinuxCNC check is documented, not automated.

## 3. Architecture

Two units, one seam each:

```
generate/pocket.rs ──(Vec<Op>)──▶ resolve ──▶ L2 IR ──▶ emit(flavor=Rs274, cnc frame) ──▶ program
        │                                                        ▲
        └── PocketOptions (validated)          Profile.machine.cnc ┘  (CLI wires profile → EmitParams)
```

### 3.1 The generator — `crates/core/src/generate/pocket.rs`

Follows the TPMS pattern exactly ("pure L1 sugar", `generate/mod.rs`): a pure function from a
validated option bundle to `Vec<Op>`, upstream of `resolve`, inheriting verify/simulate/emit.

```rust
pub enum PocketShape {
    /// Axis-aligned rectangle: min corner + size (mm).
    Rect { x: f64, y: f64, width: f64, height: f64 },
    Circle { cx: f64, cy: f64, radius: f64 },
}

pub enum CutMode {
    /// Clear the interior with contour-parallel passes.
    Pocket,
    /// Single finishing contour on the boundary (tool inside the shape).
    Profile,
}

pub struct PocketOptions {
    pub shape: PocketShape,
    pub mode: CutMode,             // default Pocket
    pub tool_diameter: f64,        // required, > 0
    pub stepover: Option<f64>,     // fraction of tool_diameter in (0, 1]; default 0.5
    pub depth: f64,                // required, > 0 (total, below z_top)
    pub depth_per_pass: Option<f64>, // default = depth (single pass); > 0
    pub z_top: Option<f64>,        // default 0.0 (top-of-stock in program coords)
    pub safe_z: Option<f64>,       // default z_top + 5.0; must be > z_top
    pub cut_feed: Option<f64>,     // mm/min, default 300
    pub plunge_feed: Option<f64>,  // mm/min, default cut_feed / 3
}

pub fn try_pocket_ops(o: &PocketOptions) -> Result<Vec<Op>, PocketError>;
pub fn pocket_ops(o: &PocketOptions) -> Vec<Op>;        // panicking convenience, mirrors tpms
pub fn try_pocket_design / pocket_design                // Design wrappers, mirrors tpms
```

`PocketError` mirrors `TpmsError` (message string, structured failure at binding boundaries, never
a panic on user input). Validation rejects: non-finite/non-positive dimensions, a tool that does
not fit the shape (`tool_diameter > width || height` / `> 2·radius` in Pocket mode), stepover
outside `(0, 1]`, `safe_z ≤ z_top`.

**Emitted op prologue:** `Geometry { width: tool_diameter, height: depth_per_pass }` (the
engagement cross-section — keeps the bead/flow model meaningful), `Extruder { on: true }` for
cutting moves so they classify as work moves (RS-274/GRBL already suppress `E` words via
`FirmwareFlavor::has_extruder()`), `Extruder { on: false }` around rapid links so they emit as
`G0` travels. `Speed { print: cut_feed }` / `Speed { print: plunge_feed }` bracket plunges.

**Geometry, per depth pass** (`z = z_top − n·depth_per_pass`, last pass clamped to `z_top − depth`):

- *Rect pocket:* concentric rectangles. The outermost ring is the wall inset by `tool_diameter/2`;
  each inner ring insets by `stepover · tool_diameter` until the remaining half-extent
  ≤ that inset (then one final center pass — a line or point — if uncovered). Two coverage
  corrections apply (2026-07-30 review):
  - The inset is clamped to `tool_r · (1 + 1/√2)` (≈ stepover 0.854). A ring's swath ends in a
    *sharp* inner corner `tool_r` inside the ring, but the ring inward of it only reaches that
    corner through a `tool_r` fillet, so a larger inset leaves an uncut cusp in the three corners
    the ring-to-ring link does not cross. The clamp only ever reduces engagement; `stepover` stays
    accepted over the whole documented `(0, 1]`.
  - When the series ends on a ring whose smaller half-extent still exceeds `tool_r` (possible for
    stepover > 0.5), one more ring is added, shrunk so that half-extent is exactly `tool_r`;
    otherwise the ring's own interior is an uncut island.
  Rings are cut innermost→outermost (conventional roughing order: full-width first cuts happen at
  the center), linked by straight feed moves. Those links cut *outward into uncut stock* — a link
  crosses the corner diagonal, so its engagement peaks near `step·√2`; that is the cost of the
  innermost-first ordering, not a claim that the link runs through cleared material.
- *Circle pocket:* concentric full circles of the same inset series (no corner clamp — concentric
  annuli overlap for any inset ≤ the tool diameter — but the same centre-clearing rescue: an extra
  ring at exactly `tool_r` when the innermost radius exceeds it, or the pocket keeps an uncut
  centre post), each authored as **two half-circle `Arc` ops**. A single full-circle arc is not
  rejected — `resolve` and `verify` both
  normalise a `start == end` arc to a full `TAU` sweep rather than treating it as degenerate — but
  it would emit as a zero-displacement `G2/G3`, which leans on each controller's full-circle
  convention; two half-circles are unambiguous in every dialect Dry emits and give each ring an
  explicit midpoint. This path exercises `G2/G3` + `I/J` emission, including the five-axis arc
  frame regression (`23f2d73`).
- *Profile (both shapes):* the single boundary contour inset by `tool_diameter/2`, per depth pass.
  (External profiles — tool outside the shape — are out of scope for this slice; "Profile" here is
  the internal finishing contour, and the doc comment says so.)
- Entry is a straight plunge at `plunge_feed` at the start of each pass's innermost ring
  (no ramp/helix in this slice). Between passes and at the end: retract to `safe_z` (extruder off
  ⇒ `G0`).

All math is closed-form offsets of rectangles/circles — exact, deterministic, `libm`-only (no new
dependency), matching the engine's cross-SDK byte-identity rules.

### 3.2 The program frame — `Profile.machine.cnc` → `EmitParams.cnc_frame`

Decision 2026-07-30: **profile-driven, constant per program.** No IR involvement.

```rust
// crates/core/src/profile/mod.rs
pub struct CncFrame {
    pub wcs: Option<u8>,          // 54..=59 → G54..G59; default 54
    pub tool: Option<u32>,        // → "T{n} M6" when present
    pub spindle_rpm: Option<f64>, // > 0 → "S{rpm} M3" … "M5"; absent ⇒ no spindle words
    pub coolant: Option<bool>,    // true → "M8" … "M9"
}
// MachineProfile gains: pub cnc: Option<CncFrame>
```

`Profile::validate()` gains the corresponding checks (finite positive rpm, wcs in 54..=59).
`Profile::emit_params()` copies the block into a new **additive, serde-defaulted** field
`EmitParams { pub cnc_frame: Option<CncFrame>, … }` — additive with `#[serde(default)]`, so every
existing `EmitParams` JSON (bindings, fixtures) deserializes unchanged, and `EmitParams::default()`
semantics are untouched (same rule that governed the BC-fallback work, see the 2026-07-30 P5.2
handover).

**Rendering (rs274 flavor only; other flavors ignore `cnc_frame` in this slice):**

- Preamble: `G21` `G17` `G90` (always — dry is mm-only, XY-plane, absolute), then `G{wcs}`, then
  `T{n} M6` if `tool`, then `S{rpm} M3` if `spindle_rpm`, then `M8` if `coolant`.
- Postamble: `M9` if `coolant`, `M5` if `spindle_rpm`, `M30` (always for rs274 when a frame is
  present).
- When `cnc_frame` is `None`, rs274 output is byte-identical to today (no drift on existing
  fixtures/goldens).
- A profile's `start_gcode`/`end_gcode` procedures do **not** compose with the frame in this slice:
  they parse and validate, but nothing consumes them — no emit path reads either field (they exist
  only as `Profile` struct fields plus a parse test), so the frame is neither inside nor outside
  any such content. Wiring them into emission is separate work.

### 3.3 CLI — `dry generate pocket`

New subcommand (the CLI's contract is L2-IR-centric, so the command resolves internally):

```
dry generate pocket --shape rect --x 0 --y 0 --width 60 --height 40 \
    --tool-diameter 6 --depth 5 --depth-per-pass 2.5 [--mode pocket|profile] \
    [--stepover 0.5] [--cut-feed 300] [--plunge-feed 100] [--safe-z 5] \
    [--profile cnc.json] -o pocket.json
```

> [Editorial note, added post-review] `--mode` shipped as `--cut-mode`, to keep it distinct from
> the `--profile <machine.json>` flag on the same subcommand. The design text above is left as
> written.

Outputs resolved Dry IR JSON (generator → `resolve_checked`, feeds every existing subcommand).
The end-to-end recipe becomes:

```
dry generate pocket … -o pocket.json
dry verify pocket.json --bounds …
dry emit pocket.json --format rs274 --profile cnc.json -o pocket.ngc
```

`ResolveParams` for the internal resolve come from `--profile` when given, else defaults with the
generator's feeds.

## 4. Error handling

- All option validation errors are `PocketError` values (structured, message-carrying) surfaced
  before any geometry is produced — same contract as `TpmsError`.
- Profile `cnc` block validation failures are `ProfileError`s at load time.
- The generator never panics on user input; `pocket_ops` (panicking) documents "valid Dry pocket
  options" as its precondition, mirroring `tpms_ops`.

## 5. Testing

1. **Unit (generator math):** ring inset series for rect + circle (counts, innermost-first order,
   final-pass clamp, tool-too-big rejection, stepover bounds); full-coverage property — the union
   of ring swaths (width `tool_diameter`) covers the pocket interior for representative option
   grids (checked by max-gap ≤ stepover·d over sampled points, not by a clipping library).
2. **E2E acceptance (`crates/core/tests/cnc_pocket_e2e.rs`):** generate → `resolve_checked` →
   `verify` (clean) → `emit` rs274 with a full `CncFrame` → assert program shape: preamble order,
   exactly one `M30` as final word, `G2/G3` with `I/J` present for the circle case, no `E` words,
   rapids are `G0` at safe-Z, and the word set ⊆ LinuxCNC's documented RS-274/NGC vocabulary.
3. **Byte-drift guards:** a golden pocket program under `conformance/` (drift-gated like the
   compare/simulate corpora); existing rs274/GRBL fixtures unchanged when `cnc_frame` is absent.
4. **CLI regression (`crates/cli/tests/cli.rs`):** the three-command recipe above runs green;
   `--mode profile` emits a single contour per pass. [Editorial note: shipped as `--cut-mode`.]
5. **Docs:** `15-cli-cookbook.md` recipe + regenerate `docs/site/reference/generated` (the drift
   gate that failed on 2026-07-30 — regeneration is part of the task, not an afterthought).
6. **Manual (documented, not CI):** load the golden `.ngc` in LinuxCNC sim; note the result in the
   PR.

## 6. Alternatives considered

- **Spindle as an IR channel now** — rejected for this slice: touches the frozen IR spec, codecs,
  vectors and three SDK surfaces; deferred to #181 where laser power needs the same mechanism.
- **Frame via `start_gcode`/`end_gcode` templates only** — rejected: not validated, not
  deterministic, and every user would hand-write the same ten lines. (Those fields are also inert
  today — parsed and validated, but read by no emit path — so they are not even an escape hatch
  yet; see §3.2.)
- **Full-circle single-arc rings** — rejected on emitted-program grounds, not validator grounds:
  `resolve`/`verify` normalise a `start == end` arc to a full `TAU` sweep, so one would resolve and
  verify fine, but it emits as a zero-displacement arc whose meaning depends on the controller's
  full-circle convention. Two half-circles are unambiguous, and each still emits a `G2`/`G3` with
  incremental `I/J` offsets.
- **Zigzag clearing / arbitrary polygons / external profiles** — deferred (scope decision
  2026-07-30); contour-parallel on rect+circle is the smallest geometry that exercises lines *and*
  arcs.

## 7. Follow-ups (tracked)

- [#180](https://github.com/dmytro-yemelianov/dry/issues/180) 5-axis hardening (limits, verify
  rules, BC singularity, import round-trip).
- [#181](https://github.com/dmytro-yemelianov/dry/issues/181) power channel (laser + per-op
  spindle) — the IR-touching increment.
- [#182](https://github.com/dmytro-yemelianov/dry/issues/182) clothoids.
- [#183](https://github.com/dmytro-yemelianov/dry/issues/183) non-planar helpers + oriented
  conformance design.

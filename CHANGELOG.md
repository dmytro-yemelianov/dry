# Changelog

All notable changes to the Dry compiler project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Dry IR and the
profile/report contracts version independently (see `docs/10-dry-ir-v0-spec.md` and
`docs/11-profiles-and-reports.md`).

## [Unreleased]

### Added
- **The reference 5-axis machine has limits, and three rules that check them (#180 gaps 1–2).**
  `REFERENCE_FIVE_AXIS_MACHINE` described a B/C kinematic mapping and nothing else — no rotary travel,
  no rotary rate, no envelope — so "emits valid 5-axis g-code" was not a checkable claim. Profiles gain
  an optional `machine.rotary` block (`travel_deg` per axis letter, `max_feed_deg_min`, `envelope_mm` in
  *machine* coordinates), and `dry_core::REFERENCE_FIVE_AXIS_LIMITS` is a worked set of them for the
  reference machine. **Those numbers are illustrative, not any real machine's datasheet**, and nothing
  applies them implicitly: a profile with no `machine.rotary` block is unaffected.
  - `rotary-travel` (error, gated on `machine.rotary.travel_deg`) — a rotary word outside its axis's
    travel range.
  - `rotary-feed` (**warning**, gated on `machine.rotary.max_feed_deg_min`) — the rotary sweep a segment
    asks for, over the segment's own motion time, exceeds the axis rate. A warning because a controller
    does not refuse: it slows the whole synchronised move down, so the plan is wrong, not the geometry.
  - `orientation-reachability` (error, gated on `machine.rotary.envelope_mm`) — the machine position an
    orientation implies, through the same transform the emitter applies, is outside the reachable
    envelope. Distinct from `bounds`, which checks programmed workpiece coordinates.

  All three are **contract-gated, never always-on**: each states a property of a machine, not of the IR,
  so no 3-axis report changes and every existing golden is byte-identical. They resolve orientations
  through the profile's own `machine.five_axis` model (the reference machine when it names none — the
  same fallback `emit_params` uses), so the angles judged are the angles the emitter writes.
- `Contracts` gained an optional `rotary` object and the rule catalog went 24 → 27, both additive in
  `spec/dry-reports-v1.schema.json`; `machine.five_axis` and `machine.rotary` are documented in
  `spec/dry-profile-v1.schema.json`. As with the 0.5.0 report fields, this is breaking only for a
  consumer validating against the *previous* published schema text, since both objects are
  `additionalProperties: false`.

## [0.5.0] - 2026-08-01

### BREAKING
- **TPMS: `maxFieldSamples` now refuses negative and `NaN` values (H1.4).** Both previously disabled
  the field-sample guardrail *silently*, and the raw-JSON path wasm and PyO3 share made that reachable
  from untrusted input — measured accepted at 8,120,601 samples against a 6,000,000 budget. **`0`
  still means unlimited** and is unchanged: it is the sentinel `sdk/ts` encodes `Infinity` as, so it
  is a wire contract. The accepted residual is recorded deliberately — a caller who can send `0` can
  still disable the guard.
- **TPMS: `adaptiveMaxDepth` is validated and capped at 16.** It was the only integer option with no
  validation. Refinement bisects, so the depth is an exponent.
- **TPMS: `perimeterInset` is now refused when it leaves a rectangle narrower than the 1e-6 mm
  emission grid**, rather than only at an exact `2·inset >= width` boundary. An inset one epsilon
  below the old boundary produced 176 coincident extruding moves with zero extruded length that
  `verify` reported clean. Jobs relying on a sub-grid "perimeter" were producing nothing.
- **`verify` gained six rules, five of them always active, so a toolpath that was previously reported
  clean may now report errors (H1.3).** Each states a property no Dry producer can violate, so the only
  reports that turn red were already describing a program that does not match its own IR:
  - `continuity` (error) — a segment must start where the previous one ended, per axis, within
    `1e-6 mm` absolute below 1 mm and relative above. An undefined (`null`) axis inherits and never
    counts as a violation; `manual_gcode` resets the tracked position rather than being compared.
    This matters because the emitter writes endpoints only: a gap produces **no repositioning move**,
    so the machine cuts a straight line across it along a path no other rule inspects.
  - `segment-length` (error) — for straight or stationary primitives, declared `length` must equal the
    distance between the segment's own endpoints, same tolerance. `arc` and `spline` are excluded.
  - `arc-length` (error) — an arc's `length` must equal `hypot(radius × swept_angle, Δz)`, same
    tolerance.
  - `negative-quantity` (error) — `length`, `volume` and `speed` must not be negative, and `width` /
    `height` must be positive when present. Negative `filament` is a retraction and is excluded.
  - `filament-consistency` (**warning for one minor release**, then error) — `volume / filament` is the
    feedstock cross-section and must be constant per `tool`, within relative `1e-6`. It ships as a
    warning because multi-diameter or multi-material IR is unusual but not ill-formed and no in-tree
    producer emits any; the promotion to error will be its own breaking entry.
  - `bead-volume` (error) — opt-in via `process.bead_volume_tolerance`, so it breaks nobody. It is not
    always active because `coasting` and `arc_fit` both break `volume = length × width × height × flow`
    by design.
- **`VerifyReport` gained `segments_inspected`, `rules_evaluated` and `contracts`.** This is additive to
  `spec/dry-reports-v1.schema.json` — amended in place rather than forked to v2 — and reports written by
  older engines still validate, because the new properties are not `required`. It is nonetheless
  breaking for any consumer validating against the *previous* published schema text, since
  `VerifyReport` is `additionalProperties: false`.
- **`junction-velocity` now measures the vector velocity change** `‖v_b·t̂_b − v_a·t̂_a‖` across a
  junction, using the same arc-aware tangents `optimize::adaptive_speed` uses, instead of the scalar
  feedrate difference `|v_b − v_a|`. The rule id, severity (warning) and gating contract are unchanged.
  The new measure strictly generalises the old one — with equal tangents it reduces to `|v_b − v_a|` —
  so nothing that fired before stops firing, but a constant-speed 90° corner now fires where it
  previously could not. Reported magnitudes change: the kinematics conformance report moves from
  `Δv 25.0` to `55.9` with an unchanged finding set.

### Fixed
- **TPMS: no surface emits a coincident extruding move any more (H1.4 T3).** The path dedupe compared
  points against a 1e-7 threshold while emission rounds coordinates to the 1e-6 grid, so points up to
  five times further apart than the threshold still collapsed onto a single emitted coordinate — an
  extruding move from a point to itself. Measured at *default* options: `schwarz-d` at
  `samplesPerCell 8` produced 4, `fischer-koch-y` 3 across two resolutions, `neovius` 1. Deduplication
  now happens against the grid the points are actually emitted on, so the property holds by
  construction, and a path whose points all land on one coordinate is dropped rather than emitted as a
  travel followed by an extruder-on with no move. `conformance/gallery/gyroid_infill.json` is
  unchanged: the defect is surface- and resolution-dependent and gyroid at the fixture's options never
  had a collapsing pair.
- **TPMS: the adaptive field-sample budget no longer over-estimates by 15×.** It charged every base
  interval as if it refined all the way to `adaptiveMinLayerHeight`, ignoring `adaptiveMaxDepth` — the
  actual limiter at the defaults. Measured 2001 layers estimated against 133 actual, so turning
  `adaptive` on could refuse a job that was legal without it. No previously-accepted job is now
  refused by this change; it only stops false refusals.
- **TPMS: a vacuity refusal no longer blames an option the caller never set.** `minPathLength`
  defaults to a value derived from the sample spacing, and when contours were traced but fell below
  it, the error told the user to lower that derived number — on 7 of 10 surfaces, always in the narrow
  band at the edge of the field range, which is exactly where someone tuning `isoLevel` lands.
  Following the advice did not help. The message now names `isoLevel` when the filter value was
  derived, and keeps naming `minPathLength` when the caller set it. Likewise a `perimeterInset`
  refusal says when the value was defaulted from `beadWidth`.
### Added
- **CNC pocket/profile → RS-274 (P5.3):** a contour-parallel pocket/profile generator for rectangles
  and circles (`dry_core::pocket_design`/`try_pocket_design`, `dry generate pocket --cut-mode
  pocket|profile`, distinct from the `--profile <machine.json>` flag on the same subcommand) and a
  profile-driven RS-274 program frame — `machine.cnc` (`wcs`, `tool`, `spindle_rpm`, `coolant`) brackets
  the motion with `G21 G17 G90` / `G5x` / `T<n> M6` / `S<rpm> M3` / `M8` and `M9` / `M5` / `M30`.
  The frame is opt-in and RS-274-only: without `machine.cnc`, and for every other flavor, emitted
  g-code is byte-identical to before. Acceptance is `crates/core/tests/cnc_pocket_e2e.rs` with the
  drift-gated golden `conformance/reports/cnc/pocket-rect-rs274.ngc`; the program has not been run on
  a physical controller. Generator inputs are bounded at 100,000 total passes (depth passes × rings
  for a pocket, depth passes for a profile), and degenerate fits — a tool filling the pocket in both
  directions, or leaving a cutting region below emission resolution — are rejected instead of
  emitting a program with no cutting moves; a tool filling exactly one direction is a slot and still
  cuts.

### Changed
- **The browser gallery's TPMS generator now delegates to the Rust engine over wasm instead of
  reimplementing it in JavaScript (H1.7).** `web/tpms.js` — a 638-line browser-only marching-squares
  generator with zero engine calls — is gone; `web/designs.js` and `web/index.html` now build TPMS
  ops through `web/tpms-engine.js`, which calls the same `tpms_ops_json` wasm export
  `sdk/ts/src/generators/tpms.ts` already delegates to. This makes the browser demo byte-identical to
  the native CLI, the Python SDK, and the TS SDK, and it stops shipping the JS generator's own
  defects (an isoLevel finite-only check, a perimeter-inset clamp, a single up-front
  `{op:'geometry', height: beadHeight}` op, no vacuity check, and unconditional `height`-sized top
  layers) — all superseded by the engine's own fixes. The browser-only `pathMode` (`linear` /
  `safe-arcs`) and `arcFit*` options are dropped, not ported: the engine's TPMS generator only ever
  emits `move` ops, so there is no G2/G3 arc-fitting equivalent to delegate to. The `tpmsPathMode` UI
  control is removed along with them; TPMS contours generated in the browser are now always linear
  G1 moves. `web/blocks-regression.mjs` is re-baselined: its former dynamic execution of the deleted
  JS generator is replaced with static checks that the delegation wiring is in place, since TPMS
  Op-generation output now requires the wasm build (`web/pkg/`), which this file runs before in CI.
  No behavioural coverage of the browser's TPMS output existed before this fix (`web/smoke.cjs` never
  called `tpms_ops_json`, and no conformance vectors cover TPMS) — `web/smoke.cjs` now asserts
  `tpms_ops_json` output shape (non-empty, `geometry`-first, `move`-only, unknown-surface rejection)
  for a few surfaces against the built `web/pkg-node/`, and `web/designs-import-check.mjs` imports
  `web/designs.js` under Node (against the browser-target `web/pkg/`, when built locally) to catch
  regressions in the lazy TPMS-gallery pattern. `web/pkg/` itself stays git-ignored and must be
  rebuilt locally with `bash web/build.sh` — it is not currently produced by any CI job, so the
  second half of `designs-import-check.mjs` only runs where that build exists.
  Fixed alongside this: `materializeDesign` in `web/designs.js` used to call the (now engine-backed)
  `build` eagerly at module-evaluation time, which threw before wasm `init()` ran and blanked the
  whole gallery page; `docs/site/scripts/stage-gallery.mjs`'s allow-list still named the deleted
  `web/tpms.js` instead of `web/tpms-engine.js`, breaking the docs-site product build.
- **BREAKING (wasm, Python, the TS SDK and the Rust API): every TPMS layer declares the bead height
  it occupies — the clamped top layer included, on the non-adaptive path too.** The per-layer
  declaration added for adaptive slicing (below) was gated on `adaptive`, so a plain job's top layer
  still declared the full `layerHeight` while occupying only the Z that remained after clamping. At
  the generator defaults (`2×2×2`, `cellSize 12`, `layerHeight 0.28`) the top layer spans **0.2 mm and
  declared 0.28 — 1.40× over-extrusion on the top layer of every default gyroid**, and the residual
  was not bounded at 40%: `DEGENERATE_TOP_LAYER_FRACTION` merges only sub-1% remainders, so the worst
  surviving case was **99.95×**, measured at `layerHeight 0.5997` (a 0.006 mm layer declaring 0.5997).
  As with the adaptive case, no verifier could catch it — the IR records the wrong bead
  self-consistently. This changes the default op stream on every published surface
  (`resolve_tpms_gcode` / `tpms_ops_json` on wasm, `resolve_tpms_gcode` on PyO3, `tpms()` / `tpmsOps()`
  in `sdk/ts`, and `tpms_ops` / `tpms_design` / `try_tpms_ops` / `try_tpms_design` in Rust): a clamped
  job now emits one additional `Op::Geometry`, and the G-code it produces deposits less material on
  its top layer.

  `beadHeight` is now preserved as a **ratio** (`gap × beadHeight / layerHeight`) rather than
  discarded. Declaring the raw gap overrode the option from the second layer onward: measured
  `layerHeight 0.4, beadHeight 0.5, adaptive: true, adaptiveMinLayerHeight 0.2` emitted declared
  heights `[0.5, 0.2]`, silently turning a deliberate 1.25× squish into 0.5× on every layer after the
  first. A nominal-height layer therefore declares exactly the configured `beadHeight`, which is also
  what the first layer gets — no longer a special case, just the same rule applied to its nominal gap.
  `beadHeight` defaults to `layerHeight`, so a job that does not set it is unaffected by this part.

  **A layer that follows one or more layers which traced no contour declares the nominal bead, not
  the Z it spans.** A layer with no contours emits nothing and is not the layer the next bead rests
  on, so the next depositing layer's gap spans every skipped layer — under `adaptive` that was
  already the case, and ungating the declaration made it the default-path behaviour. Measured at
  `neovius, isoLevel 1.0, layerHeight 0.2`, the layer at `z = 7.8` follows fifteen empty ones, spans
  3.2 mm and declared 3.2: a **16× bead**, worth +20% of filament through `resolve_checked`
  (47.2735 → 56.7330 mm). `neovius, isoLevel ±0.8, layerHeight 0.28` alternated 0.28/0.56 (2×) and
  `schwarz-p, isoLevel 1.0, layerHeight 0.28` declared up to 1.4 (5×). The gap is only a bead height
  when it is material actually stacked beneath: across the skipped layers nothing was deposited, so
  the nozzle lays one ordinary bead onto whatever is below rather than a 3.2 mm column into air.
  Declaring the span would be a self-consistent lie of exactly the kind this change removes, and a
  larger one than the defect it fixes. No verifier catches either version — no rule relates deposited
  material to geometry (H1.3).

  `DEGENERATE_TOP_LAYER_FRACTION` is **kept** and now pinned from both sides by a boundary test (a
  0.9% remainder merges, a 1.1% remainder stacks). An honest declaration does not make a degenerate
  layer emittable: the remainder has no lower bound, and when it is smaller than half the coordinate
  *step* (`1e-6 mm`, so 5e-7 mm) the appended top Z rounds onto the Z of the layer below, the gap
  becomes `0.0`, and the declaration is a zero height that `resolve` refuses outright. Reaching that
  needs a block height off the layer grid: with the constant deleted, `cellSize 12.0000003,
  layerHeight 2.0` emits `[2.0, 0.0]` and is refused. A *layer height* with a sub-step remainder is
  not a trigger — `layerHeight 1.99999995` on a 12 mm block emits `[1.99999995, 2.0]` and resolves,
  because every intermediate Z is rounded as it is pushed, so `zs.last()` already equals the height
  and the clamp branch never runs. Redistributing the remainder across all layers — which would make
  the constant unnecessary — is a slicing-policy change, not a declaration fix, and was declined for
  this slice.

  **Op-stream delta, measured across 28 option sets by diffing the serialized op list against the
  previous commit:** with a **grid-aligned** `layerHeight`, every clamped case is **exactly +1 op**
  (default 21634 → 21635; `small_cell`, `perimeter`, a flow/phase variant, a `layerHeight 0.37`
  block, the `0.5997` worst case, and all ten surfaces likewise +1). A `layerHeight` that is *not* a
  multiple of the 1e-6 mm coordinate step is not +1: the quantized gap alternates, so the
  declaration re-emits on most layers — `layerHeight = 1/3` on a 12 mm block goes **2463 → 2488 ops
  (+25)**, 26 declarations across 37 layers alternating `0.333333`/`0.333334`. That is honest (the
  emitted Z really does step by that amount) and is left unsuppressed: a tolerance would reintroduce
  the declared-vs-actual divergence this change removes. The adaptive fixture is **byte-identical**,
  as are a job whose height is an exact multiple of its layer height and a non-adaptive `beadHeight`
  job. The `beadHeight`-under-adaptive fixture keeps its op count and changes only the declared value
  (`0.2` → `0.25`). The four skipped-layer fixtures end at **+1 or +0** against the pre-change stream
  (e.g. `neovius, isoLevel 1.0, layerHeight 0.2` is 2852 → 2852 and byte-identical). With
  `Op::Geometry` filtered out, **all 28 streams are byte-identical before and after** — no move, path
  ordering, or layer set moved. `conformance/`, `proofs/`, `spec/` and `formal/` are untouched: there
  is no TPMS generator fixture in the conformance corpus (`gallery/gyroid_infill.json` is
  FullControl-oracle output, not generator output).

  **Known extreme-value gaps, recorded not closed** (outside physical ranges, all three the generator
  emitting IR its own `resolve` refuses — the class H1.1/H1.2 closed — while `tpms_ops_json` returns
  the stream with no refusal at all): the ratio can round a declared height to `0.0`
  (`layerHeight 0.5997, beadHeight 0.00004` → `[4e-5, 0.0]`, refused downstream; it validated OK
  before this change); `DEGENERATE_TOP_LAYER_FRACTION` does not close that hole for
  `layerHeight < ~5e-5` (`cellSize 0.1000003, layerHeight 1e-5` → refused); and `round()` overflows
  to `inf` for `beadHeight 1e307`. Tracked under H1.6 in `docs/04-tasks.md` for a later slice to
  close in the `reject_vacuous` idiom (ADR 0002 §4).
- **Adaptive TPMS layers now declare the bead height they actually occupy.** `Op::Geometry` was
  pushed once, before any slice, carrying `beadHeight` (which defaults to `layerHeight`), and was
  never updated. `resolve` reads that height as deposited volume (`length × width × height × flow`),
  so with `layerHeight: 0.4, adaptive: true, adaptiveMinLayerHeight: 0.05` a single
  `Op::Geometry { height: 0.4 }` stood against measured layer gaps of `[0.05, 0.1, 0.2, 0.4]` — **8×
  over-extrusion on the thinnest layer**, and the defect that adaptive slicing exists to avoid. No
  verifier could catch it: the IR faithfully records the wrong bead, so `bead`, `max-flow` and every
  other rule saw a self-consistent lie. The generator now emits a fresh `Op::Geometry` whenever the
  gap to the layer below changes. The first layer keeps the configured `beadHeight`: it has no layer
  beneath it to measure a gap against, and `resolve` has no first-layer convention of its own to
  match. A layer that traces no contour deposits nothing, so the next layer's gap is measured from
  the last layer that did.

  Separately, `base_layer_zs` unconditionally appended a final layer at exactly the block height.
  With `cellSize: 12, cellsZ: 1, layerHeight: 1.9999` that put two layers **0.6 µm apart**, each
  extruding a full 1.9999 mm bead; the sliver is now merged into the layer below, which keeps the
  block's full height without inventing a second bead for it.

  **Output changes for adaptive jobs and for that degenerate top-layer case only.** Verified by
  diffing the op stream against the previous commit across 15 option sets: the default options, all
  ten surfaces, `perimeter`, and a flow/phase/`z0`/`beadHeight` variant are **byte-identical**. The
  adaptive fixture gains 29 `Op::Geometry` ops (6539 → 6568) and **no move changes at all**; the
  sliver fixture drops from 518 to 454 ops, and the top layer keeps the same 55-point geometry
  (only the nearest-neighbour path ordering shifts, since its cursor now comes from the layer
  below). `conformance/gallery/gyroid_infill.json` is unaffected and byte-identical — it is a
  FullControl-oracle fixture with uniform 0.3 mm layers and no `adaptive`, and it is not produced by
  this generator.
- **BREAKING (wasm, Python, the TS SDK and the Rust API): the TPMS generator refuses option sets
  that would deposit no material.** TPMS is the only generator exposed on all three published surfaces
  (`resolve_tpms_gcode` / `tpms_ops_json` on wasm, `resolve_tpms_gcode` on PyO3, `tpms()` in
  `sdk/ts`), so this narrows acceptance on each of them. The panicking Rust wrappers `tpms_ops` /
  `tpms_design` (re-exported from `lib.rs`) now panic for these classes where they previously
  returned a four-op program; `try_tpms_ops` / `try_tpms_design` return the error. Following ADR 0002 §4 — refuse, do not
  clamp, and do not silently emit nothing — three input classes that previously returned `Ok` now
  return a `TpmsError`:
  - **`isoLevel` outside the surface's field range.** It was checked only for finiteness. The gyroid
    saturates at ±1.5, Schwarz-P at ±3, and the others differ; the valid range is surface-dependent
    and was documented nowhere. `{"isoLevel": 1.5}` produced a **4-op program with zero moves** that
    resolved, verified with **zero findings**, and simulated to **zero extruded volume** — a file
    that heats the nozzle and prints nothing. No verifier could catch it: the IR faithfully records
    an empty program. The check is computed from the sliced result rather than a per-surface table,
    so it holds for every surface and every phase/cell combination.
  - **`minPathLength` above the contour scale**, which filtered away every stitched contour. The
    rejection distinguishes this from the `isoLevel` case by the *pre-filter* contour count, and
    names the option that actually emptied the program plus the longest contour that was dropped —
    a message that blamed the wrong option would be worse than none.
  - **`perimeterInset` at or beyond half the block width or depth**, which was silently **clamped**
    to `width/2 - 1e-9`, producing a rectangle spanning 2e-9 mm and 44 zero-length extrusions
    presented as a perimeter wall. The clamp is removed; the inset is only read when `perimeter` is
    on, so it is only gated there. **This narrows the class rather than closing it:** the gate is an
    exact `2·inset >= width` boundary, so an inset one epsilon below it still yields a rectangle
    below the 1e-6 emission quantum — measured at `perimeterInset: 5.9999999` on a 12 mm block, 176
    coincident extruding moves with zero extruded length that `verify` reports clean. The vacuity
    check cannot catch it either, because a `perimeter: true` job is exempt from that check
    unconditionally. Requiring the rectangle to survive emission rounding is tracked as a follow-up.

  A program whose only material is its perimeter is still accepted — a perimeter wall is real
  material. Emitted output for every option set that already deposited material is **unchanged**,
  and `conformance/gallery/gyroid_infill.json` is byte-identical (it is a FullControl-oracle
  fixture and is not produced by this generator).
- **BREAKING (wasm, Python, the `dry` CLI, `containers/verify-runner` and `crates/cloud`): the
  ingress paths now refuse the values `emit` refuses.** H1.1 made the emitter the last gate before a
  machine; it was also the *only* one. Five paths fed non-finite or nonsensical quantities into the
  IR behind it, and each is now closed where the number enters. `verify-runner` and `cloud` are named
  explicitly because both call `import_gcode_reader_with_map`, so both gain the G-code rejections:
  - **Binary codec.** `Reader::f64` was a bare `from_le_bytes` with no validation, so a `.dryc`
    carrying `00 00 00 00 00 00 F8 7F` in any column decoded to `Length(NaN)` — `DecodeLimits`
    bounds sizes, never values, though hostile input is explicitly in the decoder's threat model.
    Both binary forms (`DRY0` and `DRY1`, which share the reader) now fail with the new
    `CodecError::NonFinite`. The JSON codec was never exposed: `serde_json` rejects the bare
    `NaN`/`Infinity` literals, so JSON round-trips were always safe. Note the encode/decode
    asymmetry this creates: `encode` is not tightened in this release, but an existing `.dryc`
    written by an older version from a toolpath holding non-finite values is now **undecodable** —
    the only producer of such a file was a defect, so the archive was never faithfully readable, but
    the failure now surfaces at decode instead of downstream.
  - **G-code import.** The word scanner deliberately admits exponent notation, so `M221 S1e400`
    parsed to `inf`; `flow_ratio_from_percent` detected the non-finite value and returned it
    anyway, and one `0.0 * inf = NaN` later the following move emitted `E NaN`. `G1 Xnan` reached
    the IR by the same route without any exponent. Every numeric word (X/Y/Z/E/F/S/P/I/J/K) is now
    checked finite at the scanner (`GcodeParseError`), and both ratio helpers are total. A negative
    `F` is refused when the motion is lifted (`GcodeImportError`); motion *before* the first `F` is
    still accepted, since a program that inherits the machine's modal feedrate is valid. Checking the
    *parsed word* is not sufficient on its own, so the arithmetic between the scanner and the IR is
    checked too: `G1 X1e308 Y1e308` overflows in `point_dist`, `G20` scales every coordinate,
    feedrate, extrusion and `G92` origin by 25.4, and an `M221` flow ratio scales the deposit before
    it meets the filament cross-section. Each of those built a quantity from a computed value and
    put `Length(inf)` into the IR of a release build (and panicked `Length::mm`'s `debug_assert` in a
    debug one) from a file of a few dozen bytes. Every such value now goes through `Length::try_mm`
    or an explicit finiteness check and reports a `GcodeImportError` naming the source line. A
    `filament_diameter` that is finite but too large to square into a finite cross-section is
    likewise refused (`GcodeImportParams` is caller JSON on wasm and PyO3).
  - **`ResolveParams`.** `retraction_distance` / `retraction_speed` were never validated, although
    the per-op `Retract`/`Unretract` fields they stand in for were checked positive — the guard was
    bypassed by omitting the op field. `retraction_distance: Some(-2.0)` made
    `filament: Length::mm(-dist)` *positive*, so `verify` classified the retract as an unretract and
    `max_retraction_distance` never applied, while `simulate` subtracted a negative duration. Both
    are now required finite and positive. This is live on the bindings: wasm and PyO3 deserialize
    `ResolveParams` from raw caller JSON, so `resolve_*` now errors where it used to return a
    mis-modelled toolpath. Validating the *inputs* is again not sufficient on its own, so
    `resolve_checked` now also checks the toolpath it produced: `validate_design` bounds coordinates
    only by `is_finite`, and two ops 1e200 apart square to `Area(inf)` inside `dist`, which
    `Area::sqrt` returns as `Some(Length(inf))` because `inf >= 0.0` — schema-valid JSON put a
    non-finite length straight into the IR. `resolve_checked` refuses any lowered toolpath carrying
    a non-finite quantity, naming the segment and field; the check is a postcondition, so it holds
    however the lowering computes and for ops added later. Separately, `dia` must now give a finite
    non-zero bead cross-section: `π·(dia/2)²` underflows to zero below `dia ≈ 4e-162`, and every
    extruding op divides by it — `Op::Deposit` yielded `Length(inf)` filament and a travel's
    `0.0 / 0.0` yielded `Length(NaN)`, which `simulate` then read back through `Length::mm`.
  - **3MF import.** A present-but-non-finite attribute (`x="nan"`, `feedrate="inf"`) went straight
    into the IR, a negative `feedrate` was accepted, and a *missing* one made a moving segment
    zero-speed — invisible to every metric. All three are now `ThreeMfError`s, as is a segment
    length that overflows to `inf` after parsing (`x="1e308"` against an origin of zero — the
    attribute is finite, the squared delta is not). **`export_3mf_xml` changes with it:** it now
    writes `feedrate="0.0"` for a moving zero-speed segment, where it previously wrote the attribute
    only when `speed > 0`. That combination is not hypothetical and dry itself produces it — a
    G-code program with motion before its first `F` imports to exactly such a segment — so without
    this the importer's new rejection would have refused dry's own export. The guard mirrors the
    importer's own running-position delta rather than `Segment.length`, because the two disagree on
    the first segment of a G-code import: its start is undefined, so its IR length is zero even
    though it moves. Round-trip is restored, including for a program whose first motion is away from
    the origin.
  - **Negative feedrate in `simulate`.** It passed the `speed == ZERO` check entirely and produced a
    negative duration that was subtracted from `total_time_s`; such a move is now un-timeable
    instead. A **non-finite** speed is un-timeable for the same reason, and that is a second
    accounting change worth stating plainly: `inf` previously divided to a zero duration and showed
    up as `max_flow_rate = inf`, which `verify`'s max-flow rule failed, while `NaN` poisoned
    `total_time_s`; such a segment is now invisible to the metrics and no longer trips that rule.
    Nothing in ingress can produce one any more, so reaching that arm means hand-built IR.
    Zero-speed accounting is deliberately **unchanged**: it is the branch
    `Dry.Semantics.SimulateMetrics.segmentMotionTime` models, pinned by
    `proofs/fixtures/simulate-metrics-refinement-v0.json`.

  Emitted g-code for valid input is byte-identical; every golden and conformance vector is
  unchanged. Only inputs that were already invalid change outcome.
- **BREAKING (library): `Area::sqrt` returns `Option<Length>`.** It returned `Length(NaN)` for a
  negative area — trivially constructible, and a non-finite quantity manufactured inside the unit
  system itself. `Length::mm` now carries `debug_assert!(value.is_finite())` and `Length::try_mm` is
  the checked constructor for boundary code; deliberate construction of hostile IR (as in the emit
  rejection tests) uses the raw `Length(..)` tuple constructor.
- **BREAKING (wasm and Python bindings): `emit` now refuses IR it cannot faithfully represent.**
  `dry emit` never runs the verifier, so the emitter is the last gate before a machine, and it
  validated nothing: a non-finite quantity left as a syntactically well-formed word (`G1 FNaN Xinf
  YNaN Z0.2`), an un-normalised five-axis toolframe moved the linear axes to the wrong point, a
  `CncFrame` reaching the `pub`/`Deserialize` path unvalidated emitted a bare `G0` where `G54`
  belongs or `S0 M3` before a cutting move, and an arc with no explicit endpoint emitted a full 360°
  circle on RS-274. All four are refused. On the bindings this is a **behaviour change on a published
  surface**: `dry.resolve_gcode` / `resolve_tpms_gcode` (wasm, TS SDK) and `resolve_gcode` /
  `resolve_tpms_gcode` (Python) now raise instead of returning an empty array/list. The empty return
  was reachable from ordinary JSON — `validate_design` checks only finiteness for an arc op, so
  `[geometry, extruder on, {"op":"arc","cx":10,"cy":0}]` resolved cleanly and emitted nothing — and
  in-tree consumers (`web/viewer.js`, `sdk/ts/src/engine.ts`) read that empty result as a successful
  zero-line program. `dry emit` on refused IR exits 2 and now leaves **no new** output file: the
  program is streamed to a temporary path and renamed into place only once it is complete, so a
  mid-program refusal no longer *truncates* a `.gcode` at `--out` (and, under RS-274, one missing its
  `M9`/`M5`/`M30` postamble) — a stale file already at that path is left as it was, since the rename
  never happens. `dry emit --step-nc` gates the sidecar the same way and stages it to a temp path
  before the g-code emits, committing both only once the program is known to be emittable, so the
  `.stpnc` gets the same atomicity guarantee; `dry_core::emit_step_nc` returns `Result<String,
  CodecError>` accordingly.
- **RS-274 / GRBL / KRL span rewrites no longer carry filament-axis modals.** `dry rewrite-gcode`
  targeting a non-FFF flavor previously spliced `M83` / `M82` / `G92 E0` / `M221 S100` into every
  rewritten motion span — words addressing an axis a CNC, laser or robot controller does not have,
  and an unknown M-code aborts the program on LinuxCNC/Fanuc. Output for those three flavors changes;
  Marlin/Klipper/Duet output is byte-identical. Source lines *outside* a motion span are still echoed
  through verbatim by contract, so a Marlin source's own `M104`/`M106`/`M221` still reaches the
  rewrite — filtering that echo is a separate decision.

### Deprecated
- `dry_core::emit` — use `emit_stream`, which reports the refusals above. `emit` keeps its infallible
  signature and its refuse-the-whole-program behaviour (debug builds panic on the `debug_assert`,
  release builds return no lines) as a transitional guard for in-tree callers that build their own
  IR; any caller handling IR it did not construct must migrate.

## [0.4.0] - 2026-07-28

Post-0.3.0 work — additive features plus the licensing/distribution change below.

### Changed
- Re-license Dry-authored work prospectively as proprietary, keep source and installable artifacts on
  authenticated private distribution surfaces, remove public PyPI/npm publishing, and require a private
  repository at release time.
- Keep product documentation public through a sanitized build that excludes SDK implementation code,
  WebAssembly, interactive gallery assets, packages, and release downloads.
- Raise the supported Rust toolchain to 1.88 so the pinned wasm-bindgen CLI can build in hosted CI.

### Added
- **Production safety hardening:** binary decode resource budgets (`DecodeLimits`) reject hostile
  lengths before allocation/decompression; imported firmware commands outside Dry's semantic model
  produce source-located `unmodeled-gcode` warnings; Moonraker auto-print requires a profile and a clean
  gate unless explicitly forced, treats negative API responses as failures, validates multipart
  filenames and enforces network timeouts. CI now runs untrusted PRs on isolated hosted runners, audits
  dependencies, tests all feature combinations, and release publication is locked, smoke-tested and
  sequenced behind a complete GitHub Release.
- **Machine-model v2 (kinematics, end-to-end):** a `peak-acceleration` verifier rule (arc centripetal,
  Error) + `junction-velocity` rule (Δv, Warning) gated on a profile's `machine.kinematics`; a new
  `dry import-printer-cfg` that derives a profile from a Klipper printer.cfg; and `machine.kinematics`
  exposed on the wasm/PyO3/TS SDKs (`resolve_balanced_ir` + `kinematics_json` on `resolve_verify`).
  PA / input-shaper modeling remains deferred.
- `dry explain --llm --model <id>`: online path that calls the Claude Messages API and closes the loop
  — applies executable recommendations (rewrite modes / contract overrides), re-traces + re-verifies, and
  reports measured before/after with a gate verdict; advisory (re-slice-only) suggestions are marked
  unverified. New feature-gated `dry-llm` crate is the only network code; `dry-core` stays pure.
- **Native typed contracts (Python, TypeScript & wasm)** — the `verify()` boundary now accepts
  structured `bounds` (`[[x0,x1],[y0,y1],[z0,z1]]`), `speed_range` and first-layer ranges as native typed
  values instead of CSV strings, and exposes the previously-hidden retraction / first-layer contract
  fields. The PyO3 binding takes typed Python lists; the wasm/TypeScript path crosses them as flat
  `Float64Array`s (the TS SDK keeps a fully typed surface). Legacy CSV strings are still accepted
  (normalized in the SDK layer); no wire or behavioral change for existing callers.
- **TPMS infill generator in the Rust engine core** — a new `generate` tier ports the
  triply-periodic-minimal-surface generators from the TypeScript SDK into `dry-core` (marching-squares
  slicing, contour stitching, nearest-neighbour ordering, adaptive-Z, sample-budget guard), exposed
  through a new `resolve_tpms_gcode` wasm entry point. All ten surfaces are supported (gyroid, schwarz-p,
  schwarz-d, iwp, neovius, fischer-koch-s/y, frd, lidinoid, split-p) via a typed `Surface` selector;
  unknown names are a clean deserialize error. Reachable from every front-end — `dry.tpms_gcode()`
  (Python) and the `resolve_tpms_gcode` / `tpms_ops_json` wasm entries — and the TypeScript SDK now
  **delegates** its TPMS generation to the compiled Rust engine, so all SDKs are byte-identical
  (cross-SDK identity, P4). The engine uses `libm` for native/wasm determinism.
- **Profile-gated optimization modes (`safe`/`balanced`/`max`)** — `rewrite-gcode --mode` runs an
  escalating pass set — `safe` (collinear merge + arc fit), `balanced` (+ adaptive junction/curvature
  speed shaping), `max` (+ coasting, travel reorder, z-hop) — and accepts the rewrite per motion span
  only when it introduces no new error-severity verifier finding under the active profile's contracts;
  rejected spans pass through verbatim (e.g. `max` is rejected under a `monotonic_z` contract because
  z-hop lowers Z). Adds a schema-validated `--json` `RewriteReport` envelope. The legacy `--optimize` /
  `--reorder-travel` behavior is unchanged.
- **Kinematic-limits machine model** — profiles gain an optional `machine.kinematics` block
  (`max_acceleration_mm_s2`, `max_junction_velocity_mm_s`). `balanced` mode reads it to shape cornering
  speed to the printer's real dynamics — the arc/junction limiter uses the profile's acceleration and an
  absolute square-corner-velocity cap instead of hardcoded defaults. Deterministic and firmware-neutral
  (read straight from a Klipper `printer.cfg`); pressure-advance / input-shaper remain deferred.
- **`dry explain` — offline LLM-explanation bundle (Direction 4 v1)** — a new command assembles the
  deterministic `trace` + `forensics` + `verify` reports plus a curated prompt into one self-contained
  briefing (Markdown by default; `--json` → a schema-validated `ExplainBundle`) to hand to an LLM. The
  engine never calls a model, so the bundle is reproducible and golden-tested; the prompt's hard rule is
  that any suggested change is a hypothesis that must be re-verified with `dry verify` / `review-gcode`
  before it's trusted. Recommends Claude Opus 4.8. The online `dry explain --llm` path is deferred.
- `dry compare <a> <b>`: deterministic two-file forensic delta (slicer/settings, time/flow, findings
  added/removed), drift-gated; optional `--llm --model <id>` adds a model narrative ("what changed, why,
  which is better") reusing the `dry-llm` client. New pure `dry-core::compare`; no new crate.
- `dry upload <file> --moonraker <url>`: verify gate (accept/warn/reject) then upload the (optionally
  `--rewrite`-cleaned) g-code to a Moonraker host, with optional `--print`; `--force` overrides the gate.
  New feature-gated `dry-moonraker` crate is the only network code; `dry-core` stays pure.
- **Product workflows & pilot guides** — a run-verified CLI cookbook (`docs/15`), three pilot guides
  (`docs/pilots/`: authoring, post-slicer review, SDK integration) and runnable `examples/`. IR commands
  handed raw g-code now fail with an actionable hint (use `import-gcode`/`review-gcode`).
- **Governance & support** — `CONTRIBUTING.md`, `SECURITY.md`, a support matrix (`docs/16`) and an
  auditable provenance + dependency-license ledger (`docs/17`).
- **Typed contract input** — the Python/TypeScript SDK `verify()` now accepts structured `bounds` /
  `speed_range` (lists) in addition to the legacy comma-strings.
- **G-code forensics** — `dry forensics-gcode` and a `ForensicsReport`: slicer detection, feature
  attribution from `;TYPE:`/`;FEATURE:` markers, layer model, line-width / declared-settings / infill-angle
  / infill-spacing / extrusion-multiplier estimates, and seam- and travel-strategy hints. Every derived
  fact carries a confidence tag (`from-comment` / `measured` / `inferred`); marker-less files degrade
  gracefully. Drift-gated goldens, independently schema-validated.
- **Supported firmware/printer profile matrix** — six curated clean-room profiles (Marlin/Klipper/Duet ×
  PLA/PETG/ABS) under `conformance/profile-matrix/`, each schema-valid and drift-gated through the review
  pipeline.

## [0.3.0] - 2026-06-29

The production-transition program: the public IR contract, the safety/profile contract, a release
pipeline, scale gates, and a known-limitations page. Includes a behavioral change to `verify` (hence the
minor bump).

### Added
- **Dry IR v0 public contract** — a normative spec (`docs/10-dry-ir-v0-spec.md`) for the JSON wire form
  and the `DRY0`/`DRY1` binary encodings, a draft-2020-12 JSON Schema, a curated public vector set under
  `conformance/vectors/`, and an independent (no `dry-core`) Python validator. Conformance is semantic,
  not cross-language byte-identity.
- **Profile + report contracts** — `docs/11-profiles-and-reports.md`, JSON Schemas for the profile input
  and the verify/review/trace report outputs, a closed verification **rule catalog** (stable kebab-case
  ids + per-rule severities), example profiles, and drift-gated golden reports.
- **Release pipeline** — a tag-triggered `release.yml` producing CLI binaries (macOS/Linux/Windows) with
  checksums, Python wheels (maturin) and an npm package, attached to the GitHub Release; PyPI/npm
  publishing activates when registry secrets are present (`docs/12-releasing.md`).
- **Performance & scale gates** — a deterministic bounded-memory gate proving `DRY1` streaming stays
  bounded while JSON/`DRY0` materialize, criterion benchmarks over the codecs and passes, and a CI
  bench-compile gate (`docs/13-performance-and-scale.md`).
- **Known-limitations page** — an honest account of current scope boundaries and sharp edges
  (`docs/14-known-limitations.md`).

### Changed
- **Verification severities** — `travel-without-retraction`, `first-layer-height` and `first-layer-speed`
  are now **warnings** rather than errors. A toolpath whose only findings are these no longer fails
  `verify`/`review-gcode` on those alone (`error_count` excludes warnings; the exit code is `0`). All
  other rules remain errors.
- The `review-gcode` and `trace-gcode` JSON outputs are now backed by typed report envelopes; the wire
  shape is unchanged.

## [0.2.0] - 2026-06-22

### Added
- **Core Engine & IR (`dry-core`)**:
  - Implemented first-layer speed and height safety verification contracts.
  - Implemented retraction safety verification contracts checking excessive speed/distance and travel runs without retraction.
  - Preserved negative extrusion E-axis changes (retractions/unretractions) in the toolpath IR for contract validation.
- **G-code Parsing & Round-Trip Gate**:
  - Implemented modal parser state initialization (like relative/absolute extrusion mode) to support flavor-aware parsing.
  - Implemented a G-code parser round-trip validation gate ensuring re-emitted G-code is byte-for-byte identical to the original across Marlin, Klipper, and Duet flavors.
  - Added support for G1 E0 travel format and dialect-specific dwell commands round-tripping.
- **Conformance Suite (`conformance/`)**:
  - Created a repeatable Python exporter pipeline (`conformance/export.py`) that exports golden, base G-code, simulate, profiles, and round-trip fixtures from the sibling `fullcontrol` repository.
- **Python & TypeScript Gallery Conformance (Phase 2.5)**:
  - Reimplemented all 26 gallery authoring designs in both the Python SDK (`py/`) and TypeScript SDK (`sdk/ts/`) to serve as the authoring conformance suite.
  - Extended the Python fluent `Design` builder and TypeScript `Design`/`ops` builders with `retract()`, `unretract()`, and `deposit()` methods.
  - Added support for stationary deposition (`Op::Deposit`) and retraction (`Op::Retract`, `Op::Unretract`) in the Rust core, Resolve pass, and binary columnar codec.
  - Resolved floating-point inequality and axis coordinate emission format requirements to match oracle G-code output.

## [0.1.0] - 2026-06-22

### Added
- **Core Engine & IR (`dry-core`)**:
  - Implemented core intermediate representation (Dry IR) for toolpath serialization.
  - Added physical simulation metric calculations (`time`, `distance`, `material`, `peak-flow`).
  - Added G-code emitter with Marlin, Klipper, and Duet dialect support.
  - Implemented firmware flavor-specific dwell semantics (e.g. `G4 P` for Klipper, `G4 S` for Marlin/Duet).
  - Versioned profile model supporting build volume limits, speed ranges, nozzle temperature, and flow limit validations.
  - Custom safety verification contracts (out of bounds, excessive flow, cold extrusion, orientation correctness, monotonic-Z checks).
- **L2 Optimizations**:
  - Added collinear segment merging optimization pass to clean redundant vertices.
  - Added native arc fitting (`G2`/`G3`) optimization pass to compress line sequences.
  - Added travel path reordering optimization pass to reduce travel time and length.
  - Added adaptive speed scaling pass based on junction angle and curvature.
  - Added coasting optimization pass to reduce extrusion before travel moves.
  - Added Z-hop optimization pass to lift nozzle over travel runs.
- **Language Bindings & SDKs**:
  - Created PyO3 native Python extension module and a pythonic SDK (`dry-py`).
  - Created WebAssembly adapter (`dry-wasm`) enabling compiler execution inside browsers.
  - Created TypeScript authoring SDK (`@dry/sdk`) mirroring the Python design APIs.
- **Visual Authoring & Playground**:
  - Interactive HTML5 authoring surface with Blockly-based compiler pipeline.
  - Live 3D path visualization, sync'd G-code syntax highlighting, and simulation reports.

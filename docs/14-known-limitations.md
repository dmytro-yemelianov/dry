# Known limitations

An honest, current account of what Dry does **not** do, and the sharp edges in what it does. This is the
counterpart to `docs/08-production-transition.md` §"What not to claim yet" — it exists so users and pilots
size Dry correctly. Capabilities are tracked in the per-area docs (`10`–`13`); this page is the
**caveats**.

## Scope — what Dry is not (yet)

Do not rely on Dry for any of these today:

- **A slicer replacement.** Dry has no mesh import/repair, slicing, supports, infill, placement or cooling
  generation. It compiles *authored* or *imported* toolpaths; it does not turn a mesh into one.
- **Turnkey industrial certification.** No process qualification, traceability or certification claims.
- **Production CNC / laser / robot output.** The multi-target architecture exists, but only **FFF g-code**
  is a supported emission target. CNC/laser/robot dialects, process models and machine profiles are not
  production-ready.
- **A complete non-planar / 5-axis workflow.** The toolframe orientation is a first-class IR property and
  5-axis rotary emission exists, but there is no kinematics validation, collision/singularity handling, or
  real-machine gating. Treat 5-axis as experimental.
- **A frozen external IR standard.** Dry IR v0 is published and conformance-gated
  (`docs/10-dry-ir-v0-spec.md`), but it is **v0** — expect evolution under the documented compatibility
  policy, not an immutable standard.
- **Safety guarantees beyond the documented rules.** `verify` enforces exactly the rule catalog in
  `docs/11-profiles-and-reports.md` — nothing more. A clean report means "none of these rules fired", not
  "safe to print unattended".

## Sharp edges in what Dry does do

- **Only `DRY1` streaming is bounded-memory.** Reading JSON or `DRY0`, or calling the non-streaming
  `simulate`/`verify`/`emit` on a materialized `Toolpath`, is O(N) in segments. Bounded memory requires the
  `DRY1` archive read through the `*_stream` passes (`docs/13-performance-and-scale.md`). JSON
  materialization must not be presented as streaming.
- **Cross-implementation byte-identity is not guaranteed.** Conformance is **semantic** (exact f64
  bit-equality, structural equality), not identical bytes — DEFLATE output and float formatting are
  implementation-defined (`docs/10-dry-ir-v0-spec.md` §9). The reference encoder is byte-stable against its
  own goldens; a second implementation need not produce the same bytes.
- **`ManualGcode` is encoded three ways.** JSON `manualgcode`, `DRY0` dictionary `manual_gcode`, `DRY1` tag
  `7`. This asymmetry is frozen and documented for v0 (`docs/10` §10); an external reader must map all three
  to one kind. Unifying it is a future major bump.
- **Verification is FFF-centered and contract-gated.** Most rules only fire when a contract/profile supplies
  the relevant limit; with no profile, only the structural rules run. The rule set targets FFF concerns
  (bounds, flow, temperature, retraction, first layer). `travel-without-retraction`, `first-layer-height`
  and `first-layer-speed` are **warnings**, not errors — a report with only these passes `verify`.
- **Profiles are intentionally small.** The v1 profile schema (`docs/11`) carries the limits the verifier
  can enforce plus import defaults — not a full slicer/material database. `firmware.flavor` is recognized
  for `marlin` / `klipper` / `duet`; others fall back to Marlin-style behavior.
- **Kinematic limits shape, they do not verify.** `machine.kinematics` (max acceleration / junction
  velocity) feeds the `balanced` optimisation pass to keep cornering speed within a deterministic,
  firmware-agnostic machine envelope — but there is **no verifier rule** that checks a toolpath against
  these limits, so `verify` will not flag a corner that exceeds them. The model is also deliberately
  simple: no pressure-advance, input-shaper or firmware-specific calibration (deferred).
- **Imported G-code recovers only what the text encodes.** Bead width/height, layer height and flow are
  recovered from import defaults/profile, not measured; reviewing G-code without those defaults yields
  `bead`/flow findings driven by the missing geometry, not the print.

## Support boundary

Supported today: **FFF-centered** authoring (Python/TypeScript/Rust), `DRY0`/`DRY1`/JSON IR round-trips,
`simulate`/`verify`/`optimize`/`emit`, and post-slicer `review`/`trace`/`rewrite` for Marlin/Klipper/Duet
g-code. Everything outside that — mesh slicing, non-FFF targets, certified/unattended safety,
production 5-axis — is out of scope for the current releases.

See also: `docs/08-production-transition.md` (the path to widening this boundary) and
`docs/09-customer-readiness.md` (which segments are ready now).

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
- **Production CNC / laser / robot output.** Production readiness is still **FFF-first**: CNC/laser/robot
  targets are not yet production-ready, although RS-274 output and prototype STEP-NC intent export are now
  available via `--format rs274` and `--step-nc`.
  - **`--format krl` has never been run on a KUKA controller or simulator.** It emits a real
    `DEF`/`END` module and `tools/krl_check.sh` parses **the goldens** with an external KRL grammar
    (CI job `krl`), but that is a syntax check over a fixed corpus, not an execution and not a check
    of the program you just emitted: nothing here establishes that a KRC will load it, that the poses
    are reachable, or that the motion is what was intended. No free KRL execution environment exists
    to test against. `PTP` velocity (`$VEL_AXIS[]`) is not set, no `INI`/`BAS(#INITMOV,0)` is written
    so acceleration and orientation-interpolation mode are whatever the last program left behind,
    extrusion is not carried at all, and the default identity `$TOOL` puts the TCP at the flange, so
    the emitted coordinates ignore tool length until a real `$TOOL` is supplied. Full boundary:
    [`22-krl-emit.md`](22-krl-emit.md).
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
  materialization must not be presented as streaming. Binary decoders enforce default resource limits
  before allocation/decompression; applications with a different trusted-file budget can supply explicit
  `DecodeLimits`.
- **Cross-implementation byte-identity is not guaranteed.** Conformance is **semantic** (exact f64
  bit-equality, structural equality), not identical bytes — DEFLATE output and float formatting are
  implementation-defined (`docs/10-dry-ir-v0-spec.md` §9). The reference encoder is byte-stable against its
  own goldens; a second implementation need not produce the same bytes.
- **`ManualGcode` is encoded three ways.** JSON `manualgcode`, `DRY0` dictionary `manual_gcode`, `DRY1` tag
  `7`. This asymmetry is frozen and documented for v0 (`docs/10` §10); an external reader must map all three
  to one kind. Unifying it is a future major bump.
- **Verification is FFF-centered and contract-gated.** Most rules only fire when a contract/profile supplies
  the relevant limit; with no profile, only the structural rules run. The rule set targets FFF concerns
  (bounds, flow, temperature, retraction, first layer). `travel-extrudes`, `travel-without-retraction`,
  `first-layer-height` and `first-layer-speed` are **warnings**, not errors — a report with only these
  passes `verify`.
- **A depositing travel is reported, not refused.** `travel-extrudes` is always-on but a **warning**, so
  IR whose `travel` flag disagrees with its own `volume` still passes `verify` and still exits `0`. The
  finding is located and counted; it just does not gate. The reason is that `travel` is a *classification*
  and, for imported G-code, an *inferred* one — "`G0`, or no `E` word" — while Marlin, Klipper and
  RepRapFirmware all execute `G0` as an ordinary move that honours an `E` word, which is exactly how
  OrcaSlicer's stock start G-code writes purge/prime lines (Bambu X1C and Prusa MK4 profiles alike). As an
  error the rule failed `dry review-gcode` on every stock file tried while nothing unsafe was commanded. What is genuinely lost is
  narrower than it looks: no Dry producer can emit a depositing travel (`resolve` and `optimize` set
  `volume = 0` on every travel), so error severity only ever gated imported and hand-authored IR. The cost
  is that travel-derived accounting silently mis-attributes such a move — `simulate`'s travel time and
  distance, and `travel-without-retraction` — and that the rewrite gate (`docs/11` §3.4) can no longer
  reject a span on it. Severity is **not** scoped to provenance, though the IR header does record
  `imported-from-gcode`: `verify_stream` cannot see the header, so the same bytes would verify differently
  through `dry verify` than through `dry review-gcode`; `Report` echoes `contracts` but not `meta`, so the
  difference would be invisible in the report; and a producer-declared field must not pick the severity
  the verifier assigns it.
- **`simulate`'s `max_flow_rate` metric still scores an E-only retract/prime move as a deposition rate.**
  The `max-flow` *rule* no longer does — it requires a path to deposit along, which is what removed
  1,722 of 1,810 corpus findings and all 813 on the first real user file (`docs/11` §2, "A pure filament
  move is not a deposition"). The descriptive metric was deliberately left alone, so
  `dry review-gcode` on a healthy Ender-3 file reports `max_flow_rate: 96.21` mm³/s (a `G1 E1 F2400`
  prime) beside zero `max-flow` findings. Two reasons it is a separate slice: the metric is timed
  correctly — a prime really does take 1/40 s and dropping it would understate every program that primes
  — so the fix is to report the peak *deposition* rate under a new name rather than to narrow this one;
  and the metric's segment domain is what `formal/Dry/Semantics/SimulateMetrics.lean` and
  `proofs/fixtures/simulate-metrics-refinement-v0.json` model, so narrowing it is a formal-artifact
  change. Until then, treat `max_flow_rate` on imported G-code as "peak filament throughput", not
  "peak deposition rate".
- **Neither retraction rule can see a retraction that happens while the tool moves.** Both require a
  stationary filament move, because that is the only case in which the commanded feedrate *is* the
  filament's speed. Two real patterns therefore go unjudged. (a) A **wipe-while-retracting**
  (`G1 X90.672 Y98.376 E-.11401` under a modal `F3000`): OrcaSlicer's `retract_before_wipe = 70%` splits
  one 1 mm retraction into a stationary part and a wipe remainder, so `retraction-distance` measures 0.7 mm
  of a 1 mm retraction, and a slicer configured to retract entirely inside a wipe would never be
  distance-checked at all. (b) A **retract combined with a Z lift** (`G1 Z0.6 E-1 F3600`), where the true
  filament speed is not `F` but `F·|ΔE|/|ΔXYZ|` and can be several times higher — so the honest rule is
  one that computes the filament's own speed from the E-to-motion ratio, not a reinterpretation of these
  two. The pilot file contains 684 of pattern (a) and none of (b); both are recorded rather than
  guessed at.
- **An imported de-retraction is not speed-judged.** `retraction-speed` covers a pure *unretract* only
  when it deposits nothing, and `gcode::lift` recovers a volume from any positive `E` — which makes an
  imported `G1 E1 F6000` byte-identical in IR shape to a legitimate stationary L1 `deposit` (material laid
  in place). The rule is an **error**, so the ambiguity is resolved toward not gating a legal program.
  Closing it needs the importer to record which act the `E` word performed; pinned as
  `retraction_speed_cannot_see_an_imported_unretract` in `crates/core/tests/verify_rule_scope.rs`.
- **`junction-velocity` is high-volume on real slicer output at any defensible square-corner velocity,
  and that is a property of the print, not of the rule.** The rule now measures the right quantity — the
  direction change at the junction, turned into an allowed corner velocity by the junction-deviation
  relation, calibrated so a 90° corner is allowed exactly `scv` (`docs/11` §1). On the first real user
  file (108,533 segments, Ender-3 S1, `scv = 8`, the vendor's own `machine_max_jerk_x = 8`) that took the
  count from 41,112 to 33,539 warnings. It does **not** make it small, and raising `scv` will not either:
  measured on the same file, `scv = 4` → 42,119, `scv = 8` → 33,539, `scv = 10` → 29,719,
  `scv = 20` → 18,830. The reason is in the geometry: 33,236 of that file's 90,897 printing junctions turn
  by more than 20°, and at the commanded 40–60 mm/s every one of them is above its cornering limit. Every
  firmware planner slows down at each, which is why the print is healthy. So a per-junction finding is a
  **plan-fidelity advisory** (the same character as `rotary-feed`: the controller obeys, the plan was
  optimistic), it is a Warning and never gates, and its useful form is aggregate — "this program commands
  N corners above the machine's cornering limit, costing an estimated T seconds" — which is a reporting
  change, not a rule change. Do not read a large `junction-velocity` count as a defect in the g-code.
- **No rule asks whether a velocity change along a straight line is achievable.** `junction-velocity`
  used to fire on a collinear speed step (its old measure was a scalar Δv), which read as coverage but was
  a cornering limit reporting an acceleration question. It no longer does. `peak-acceleration` evaluates
  arcs only, so a collinear 10 → 100 mm/s step in a distance too short to accelerate through is currently
  unchecked. Closing it needs a trapezoidal-planning predicate over consecutive segments, which is a new
  rule rather than a widening of either existing one.
- **No rule flags a beam lit during a travel.** `travel-extrudes` states the material analogue (a travel
  that deposits), but nothing says the equivalent about `power`: a travel carrying `power: 600` verifies
  clean — not even as a warning, which is the remaining asymmetry now that the material rule is one. It is
  deliberately not in the always-on structural set, because that set may only hold properties
  every producer satisfies and `resolve` does not: the channel is sticky like `temperature`, so a design
  that says `power 600` and then repositions resolves to a lit rapid, and an always-on error rule would
  refuse IR Dry itself produced from a legal design. The frozen corpora cannot settle it either — one of
  50 fixtures carries the channel at all, and it has no travel segments. Closing this needs a decision
  about whether `resolve` should force travels dark, not just a rule. Until then the property is held by
  the optimiser's own tests (`crates/core/tests/channels.rs`), which is why it has had to be fixed twice.
- **Profiles are intentionally small.** The v1 profile schema (`docs/11`) carries the limits the verifier
  can enforce plus import defaults — not a full slicer/material database. `firmware.flavor` is recognized
  for `marlin` / `klipper` / `duet` / `rs274` / `linuxcnc`; others fall back to Marlin-style behavior.
- **Kinematic verification is deliberately approximate.** `machine.kinematics` feeds the `balanced`
  optimisation pass and the `peak-acceleration` / `junction-velocity` rules. These are deterministic,
  firmware-neutral envelope checks, not a reproduction of firmware motion planning. There is no
  pressure-advance, input-shaper or firmware-specific calibration model.
- **Imported G-code recovers only what the text encodes.** Bead width/height, layer height and flow are
  recovered from import defaults/profile, not measured; reviewing G-code without those defaults yields
  `bead`/flow findings driven by the missing geometry, not the print.
- **The importer does not semantically model every firmware command.** Unsupported `G`/`M`/`T` commands
  are preserved byte-for-byte and reported as `unmodeled-gcode` warnings; they are never silently
  reinterpreted as modal motion. That covers an unmodeled command's *parameters* as well as its code:
  they need not be `LETTER value` G-code words at all, so Bambu's `M1002 <name> : <value>` macros,
  Prusa's quoted `M862.3 P "MK4"` firmware checks, version strings and base64 payloads are preserved
  lines rather than a refused file. What still fails loudly, by design: a **modeled motion** line whose
  words are missing or unreadable (`G1 X`, `G1 Xnope`), a modeled command carrying an unreadable number
  (`M221 Snope`), and a word value that is not a finite number, on any command. Review the preserved
  lines manually. Auto-print fails closed on these warnings unless `--force` is explicit. See
  [`docs/25-slicer-corpus-baseline.md`](25-slicer-corpus-baseline.md) for what this looks like at
  real-world volume against genuine OrcaSlicer output (`unmodeled-gcode` and `travel-extrudes` counts
  across the committed `conformance/slicer-corpus/` files).
- **`Q`, `L` and `W` are ambiguous with the legacy KRL import dialect.** They are the `PTP`/`LIN`/`WAIT`
  markers of the g-code-shaped KRL Dry wrote before #181, and also real RS-274 word letters
  (`crates/core/src/gcode.rs`). A line that states its own `G`/`M`/`T` command is read as that command,
  so Bambu's `M1006 A0 B10 L100 C37 …` is the macro it is and not a `LIN` move with a rotary pose; but a
  bare `L100` line with nothing else on it is still read as a KRL `LIN`.

## Support boundary

Supported today: **FFF-centered** authoring (Python/TypeScript/Rust), `DRY0`/`DRY1`/JSON IR round-trips,
`simulate`/`verify`/`optimize`/`emit`, and post-slicer `review`/`trace`/`rewrite` for Marlin/Klipper/Duet
`g-code`, plus experimental RS-274 output and STEP-NC intent sidecars. Everything outside that — mesh slicing,
non-FFF targets, certified/unattended safety, production 5-axis — is out of scope for the current releases.

See also: `docs/08-production-transition.md` (the path to widening this boundary) and
`docs/09-customer-readiness.md` (which segments are ready now).

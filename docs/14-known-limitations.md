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
  - **The Phase 8 industrial dialects have had no controller contact at all.** Siemens Sinumerik
    840D/ONE, Haas NextGen, Heidenhain TNC and ABB RAPID emit structurally plausible programs and are
    covered by unit tests over their own output — which is weaker evidence than KRL has, since KRL is
    at least parsed by an external grammar. No independent interpreter checks any of the four, and
    none has been loaded on a control. Only RS-274 is gated by a genuine external interpreter
    (LinuxCNC `rs274`, CI job `linuxcnc`).
- **A production robot kinematics solver.** `Robot6AxisModel::solve_ik` is a **five**-degree-of-freedom
  solve returned in a six-joint shape: `J6` is never determined and always reads `0.0`, because a TCP
  point plus a tool *direction* does not fix the roll about the tool axis and the signature takes no
  roll reference. It produces only the elbow-up branch, so it cannot follow a path requiring
  reconfiguration, and it checks reach but neither joint travel limits nor self-collision — an
  accepted solve is not a claim that the pose is attainable.
- **The digital-twin physics simulator is analytic, not validated.** Cutting force, tool deflection,
  shear-zone temperature, Taylor tool life and chatter boundaries are closed-form estimates from
  textbook models with published coefficients. Nothing in this repo compares them against a
  dynamometer, a thermocouple or a real cut. Treat the numbers as indicative, not as a process
  guarantee.
- **A complete non-planar / 5-axis workflow.** The toolframe orientation is a first-class IR property and
  5-axis rotary emission exists, but there is no kinematics validation, collision/singularity handling, or
  real-machine gating. Treat 5-axis as experimental.
- **A frozen external IR standard.** Dry IR v0 is published and conformance-gated
  (`docs/10-dry-ir-v0-spec.md`), but it is **v0** — expect evolution under the documented compatibility
  policy, not an immutable standard.
- **Safety guarantees beyond the documented rules.** `verify` enforces exactly the rule catalog in
  `docs/11-profiles-and-reports.md` — nothing more. A clean report means "none of these rules fired", not
  "safe to print unattended".

## Cross-target parity is not uniform

Dry's SDKs do **not** all expose the same engine. The Python, wasm and TypeScript bindings carry the
FFF authoring surface, the generators, B-Rep slicing and dexel simulation — but the Phase 7/8
additions are reachable only from `dry-core` and, in part, the CLI:

| Capability | `dry-core` | CLI | Python | wasm | TS |
|---|---|---|---|---|---|
| TPMS lattice infill | yes | yes | yes | yes | yes |
| Pocket / profile milling | yes | yes | yes | yes | yes |
| STEP single-solid slicing | yes | yes | yes | yes | yes |
| Mesh heightfield 5-axis drape | yes | yes | yes | **no** | **no** |
| Lathe turning / facing | yes | yes | yes | yes | yes |
| Siemens / Haas / Heidenhain / ABB RAPID emit | yes | `--format` | yes | yes | yes |
| CNC machine preamble (`cnc_frame`) | yes | profile | yes | yes | yes |
| 5-axis jerk-limited lookahead | yes | **no** | yes | yes | yes |
| Machining physics | yes | **no** | yes | yes | yes |
| STEP multi-solid assembly + CSG | yes | **no** | yes | yes | yes |
| Dexel stock simulation | yes | **no** | yes | yes | yes |
| Tool holder collision | yes | **no** | yes | yes | yes |

**This table is no longer hand-maintained.** It mirrors
[`conformance/capability-parity.toml`](../conformance/capability-parity.toml), which
`tools/check_capability_parity.py` checks against the source **in both directions** on every CI run:
a cell recorded reachable whose symbol is gone fails, and a surface that *gains* a capability recorded
absent fails too. Every absent cell carries a note saying why it is a reviewed gap rather than an
oversight. That gate exists because this table was previously a snapshot and was wrong in two cells
the first time it was written — and the manifest's first run caught three more.

**The binding gap is closed.** The lookahead optimiser, the physics simulator and all four Phase 8
dialects reach Python, wasm and TypeScript, with the `cnc_frame` they need to emit a machine preamble
rather than bare motion. Python and TypeScript are cross-checked against *each other*:
`py/tests/test_physics_and_lookahead.py` and `sdk/ts/test/physics_and_lookahead.test.ts` assert the
same physics numbers through two different FFI paths (native PyO3 and wasm), and they agree bit for
bit.

**Every generator now has a CLI surface.** `dry generate` carries `pocket`, `tpms`, `brep`, `drape`,
`lathe-facing` and `lathe-turning`. TPMS was the notable gap — H1.4 called it "the most-exposed
generator (wasm + PyO3 + TS)" while it was absent from the product that actually ships. Facing and
turning are separate subcommands because their parameters genuinely differ (turning is specified by
raw/target diameter, cut length and depth of cut; facing by a target Z and pass count), so a merged
flag set would describe neither engine call.

Still absent from the CLI, and these are analyses rather than generators: the lookahead optimiser,
the physics simulator, dexel stock simulation and tool-holder collision. STEP **multi-solid** assembly
slicing with CSG void subtraction is also CLI-absent — `dry generate brep` slices a single solid, and
expressing an assembly needs a way to name several STEP files with additive/subtractive roles that no
flag set here covers yet. Mesh drape remains the one capability missing from wasm and TypeScript,
because it reads a mesh from disk.

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
- **`max-flow` scores an E-only retract/prime move as if it deposited at that rate.** The rule computes
  flow as (filament cross-section area) x (segment speed), which is correct for an ordinary extruding
  move but not for a retract/de-retract line that carries an `E` word and no `X`/`Y` (`G1 E.8 F1800`,
  `G1 E-0.8 F1800`): the filament is only moving through the feed path, not being deposited on the part,
  yet the rule reports it at `pi * (diameter/2)^2 * (F / 60)` mm3/s regardless — 72.16 mm3/s for a
  1.75 mm filament at `F1800`, 60.13 mm3/s at `F1500`, both far above any of these machines' real
  print-flow ceilings (the X1C's published max flow is ~32 mm3/s). Across the committed
  `conformance/slicer-corpus/` files, this pattern accounts for the large majority of `max-flow`
  findings against the example profiles (1,722 of 1,810, all at the profile's own filament diameter) —
  see [`docs/25-slicer-corpus-baseline.md`](25-slicer-corpus-baseline.md) for the measurement. This is an
  **expected-import-artifact of the verifier's flow formula**, not evidence the profile's ceiling is
  wrong (a profile-mismatch) and not a real overflow event; a materially better max-flow rule would
  exclude segments with no XY displacement, but nothing in the current rule catalog does.
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

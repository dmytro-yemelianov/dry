# Changelog

All notable changes to the Dry compiler project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Dry IR and the
profile/report contracts version independently (see `docs/10-dry-ir-v0-spec.md` and
`docs/11-profiles-and-reports.md`).

## [Unreleased]

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
- **BREAKING (wasm, Python, the `dry` CLI, `containers/verify-runner` and `crates/cloud`): the
  ingress paths now refuse the values `emit` refuses.** H1.1 made the emitter the last gate before a
  machine; it was also the *only* one. Five paths fed non-finite or nonsensical quantities into the
  IR behind it, and each is now closed where the number enters. `verify-runner` and `cloud` are named
  explicitly because both call `import_gcode_reader_with_map` and `decode`, so both gain the new
  rejections:
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
    mis-modelled toolpath.
  - **3MF import.** A present-but-non-finite attribute (`x="nan"`, `feedrate="inf"`) went straight
    into the IR, a negative `feedrate` was accepted, and a *missing* one made a moving segment
    zero-speed — invisible to every metric. All three are now `ThreeMfError`s, as is a segment
    length that overflows to `inf` after parsing (`x="1e308"` against an origin of zero — the
    attribute is finite, the squared delta is not). **`export_3mf_xml` changes with it:** it now
    writes `feedrate="0.0"` for a moving zero-speed segment, where it previously wrote the attribute
    only when `speed > 0`. That combination is not hypothetical and dry itself produces it — a
    G-code program with motion before its first `F` imports to exactly such a segment — so without
    this the importer's new rejection would have refused dry's own export. Round-trip is restored.
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

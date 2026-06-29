# Changelog

All notable changes to the Dry compiler project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Dry IR and the
profile/report contracts version independently (see `docs/10-dry-ir-v0-spec.md` and
`docs/11-profiles-and-reports.md`).

## [Unreleased]

### Added
- **Native typed contracts (Python)** — the PyO3 `verify()` boundary now accepts structured `bounds`
  (`[[x0,x1],[y0,y1],[z0,z1]]`), `speed_range` and first-layer ranges as native typed values instead of
  CSV strings, and exposes the previously-hidden retraction / first-layer contract fields as kwargs.
  Legacy CSV strings are still accepted (normalized in the SDK layer); no wire or behavioral change for
  existing callers.
- **Gyroid TPMS generator in the Rust engine core** — a new `generate` tier ports the gyroid
  triply-periodic-minimal-surface generator from the TypeScript SDK into `dry-core` (marching-squares
  slicing, contour stitching, nearest-neighbour ordering, adaptive-Z, sample-budget guard), exposed
  through a new `resolve_tpms_gcode` wasm entry point. Uses `libm` for native/wasm determinism — output
  differs sub-micron from the JS generator, so correctness is validated by geometric invariants, not
  byte-identity (the other nine surfaces, PyO3 exposure and TS-SDK delegation are deferred).
- **Profile-gated `safe` optimization mode** — `rewrite-gcode --mode safe` runs the geometry-local
  passes (collinear merge + arc fit) and accepts the rewrite per motion span only when it introduces no
  new error-severity verifier finding under the active profile's contracts; rejected spans pass through
  verbatim. Adds a schema-validated `--json` `RewriteReport` envelope. `balanced`/`max` are reserved; the
  legacy `--optimize` / `--reorder-travel` behavior is unchanged.

## [0.4.0] - 2026-06-29

Post-0.3.0 work — all additive (no behavioral changes from 0.3.0).

### Added
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

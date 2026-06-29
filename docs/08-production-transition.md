# Dry production transition plan

This document translates the current v0 foundation into a production program. The goal is not to call
Dry "done"; it is to define the gates that make Dry safe to depend on, package, pilot, and support.

## Current position

Dry has a credible technical core:

- one dependency-light Rust engine;
- thin CLI, Python, TypeScript and wasm/browser adapters;
- typed units and L2 motion IR;
- deterministic resolve, simulate, verify, optimize, emit and import flows;
- JSON plus `DRY0`/`DRY1` binary codecs;
- conformance fixtures and byte-identity checks;
- browser gallery and Blockly-style authoring surfaces;
- review, trace and rewrite commands for slicer G-code.

That is enough for controlled internal use, research workflows, and technical pilot customers. It is not
yet enough for a broad production product that non-expert users can trust without validation.

## Production definition

Dry should be considered production-ready only when these conditions are true:

1. The public contract is stable.
   Dry IR has a published schema/spec, versioning rules, compatibility policy, and conformance vectors.

2. Releases are repeatable.
   CLI binaries, Python wheels, npm packages, wasm bundles and checksums are built from tagged releases.

3. Safety behavior is explainable.
   Verification findings are stable, documented, source-located, and tied to machine/material profiles.

4. Scale behavior is measured.
   Large files have memory and runtime budgets with regression tests, especially for streaming paths.

5. User workflows are product-shaped.
   First-run docs, examples, errors, profile loading, report output and browser flows are clear enough for
   a user who did not write the engine.

6. Support boundaries are explicit.
   The project states which machines, firmware flavors, file formats, and workflows are supported.

## Transition workstreams

### 1. IR standard and compatibility

> **Delivered (v0):** [`docs/10-dry-ir-v0-spec.md`](10-dry-ir-v0-spec.md),
> [`spec/dry-ir-v0.schema.json`](../spec/dry-ir-v0.schema.json), the public
> [`conformance/vectors/`](../conformance/vectors) (JSON + `DRY0`/`DRY1` + metrics + g-code), a Rust
> drift gate (`crates/core/tests/spec_vectors.rs`) and an independent, `dry-core`-free Python validator
> (`tools/validate_vectors.py`). All three acceptance criteria below are met for v0.

Deliverables:

- `Dry IR v0` schema document covering JSON, `DRY0`, `DRY1`, units, segment kinds and metadata.
- Semver and compatibility rules for readers, writers and adapters.
- Public conformance vectors with expected JSON, binary, metrics and g-code outputs.
- A small independent reader/writer smoke implementation or external round-trip harness.

Acceptance:

- A new implementation can read and write at least one vector without using `dry-core`.
- Old valid v0 files keep decoding after new releases unless explicitly migrated.
- Unknown enum/kind/version failures are documented.

### 2. Release engineering

> **Delivered (initial):** a tag-triggered [`release.yml`](../.github/workflows/release.yml) builds CLI
> binaries (macOS/Linux/Windows) with `SHA256SUMS`, Python wheels (maturin) and an npm package, attaches
> them to the GitHub Release, and publishes to PyPI/npm when secrets are present; a version/tag guard
> (`scripts/check-version.sh`) keeps the manifests in lockstep. Process + install-without-source:
> [`docs/12-releasing.md`](12-releasing.md). Remaining: exercise the pipeline on a real tag and add more
> binary targets where useful.

Deliverables:

- Tagged release process.
- CLI artifacts for macOS, Linux and Windows where practical.
- Python wheels through maturin.
- npm package for the TypeScript SDK and wasm payload.
- Checksums, changelog entries and release notes.

Acceptance:

- A clean machine can install and run CLI, Python and TypeScript examples without building from source.
- CI can reproduce release artifacts from a tag.
- Release notes state compatibility and migration risks.

### 3. Safety and profile workflow

> **Delivered (v1):** [`docs/11-profiles-and-reports.md`](11-profiles-and-reports.md) with the
> [`profile`](../spec/dry-profile-v1.schema.json) and [`reports`](../spec/dry-reports-v1.schema.json)
> schemas, a closed verification rule catalog with stable ids + severities, example profiles, and
> drift-gated golden reports re-validated independently by `tools/validate_reports.py`. Remaining: a
> golden firmware/printer matrix and downstream automation report formats.

Deliverables:

- Profile schema reference with examples for common printer/material cases.
- Stable verification rule IDs and severity definitions.
- Machine/material/process profile validation.
- Report formats for CLI JSON, browser UI and downstream automation.
- Golden profiles for a small supported firmware/printer matrix.

Acceptance:

- A user can load a profile, review a G-code file, understand findings, and export a report.
- The same report is stable across CLI and wasm for the same input.
- Unsafe rewrites are either rejected or documented with an explicit warning.

### 4. Performance and scale

> **Delivered (initial):** [`docs/13-performance-and-scale.md`](13-performance-and-scale.md) documents the
> memory model (only the `DRY1` streaming path is bounded-memory; JSON/`DRY0` materialize), criterion
> benchmarks (`crates/core/benches/engine_codec.rs`), and a **deterministic** bounded-memory scale gate
> (`crates/core/tests/memory_scale.rs`, via a counting allocator) plus a CI bench-compile gate. Remaining:
> large representative corpora and tracked wall-clock thresholds.

Deliverables:

- Benchmarks for JSON input, `DRY0`, `DRY1`, emit, verify, simulate, trace and rewrite.
- Memory ceilings for large-print use cases.
- Regression thresholds in CI for representative corpora.
- Documentation of which operations stream and which materialize the full toolpath.

Acceptance:

- A large print can be simulated, verified and emitted through `DRY1` without unbounded memory growth.
- JSON materialization is documented and not accidentally presented as bounded-memory streaming.
- Performance regressions fail before release.

### 5. Product workflows

Deliverables:

- CLI recipes for: import, verify, trace, rewrite, optimize, pack and unpack.
- Browser workflow for gallery, authoring, verification and export.
- Python and TypeScript tutorials for algorithmic toolpath generation.
- Error messages with user-actionable fixes.
- Example inputs for each supported customer segment.

Acceptance:

- A new technical user can complete a validated "generate -> verify -> emit" workflow in under 15 minutes.
- A post-slicer user can complete a "review -> trace -> rewrite" workflow without reading Rust docs.
- The browser demo makes it clear what is experimental and what is release-supported.

### 6. Integrations and extensibility

Deliverables:

- Rust authoring SDK or documented native API pattern.
- CAD/workbench integration spike for one host.
- Plugin/export hooks for future 3MF, CNC, laser and robot targets.
- Typed contract input object to replace comma-string boundaries in public APIs.

Acceptance:

- One external app can call Dry through a stable API and reproduce a conformance vector.
- Integration docs name what Dry owns versus what upstream CAD/slicer code owns.

### 7. Governance, support and legal hygiene

Deliverables:

- Clean-room provenance checklist for every generated corpus and profile.
- Contributor guide.
- Security/contact policy.
- Support matrix.
- License review before public commercial positioning.

Acceptance:

- A release can be audited for provenance and dependency licensing.
- Users know where to report bugs and what support level to expect.

## Milestones

### Production alpha

Audience: internal users, advanced makers, researchers, trusted technical pilots.

Required:

- current CI green;
- documented install-from-source path;
- pilot profiles;
- known limitations page;
- no silent codec truncation;
- source-located verification reports.

Exit:

- 3 to 5 pilot workflows complete without manual code patches.
- All pilot failures are classified as product gap, profile gap, engine bug or unsupported scope.

### Production beta

Audience: external technical users who can validate machine output.

Required:

- tagged releases;
- Python/npm artifacts;
- CLI binaries or documented build installers;
- published IR schema draft;
- profile schema reference;
- large-file benchmark report;
- pilot feedback incorporated.

Exit:

- New users can install without repository checkout for at least one frontend.
- No known high-severity correctness bugs in supported workflows.
- Compatibility and migration policy is documented.

### Production v1

Audience: customers relying on Dry in repeatable workflows.

Required:

- stable Dry IR v1 contract;
- conformance vector publication;
- release automation;
- support matrix;
- backwards-compatibility tests;
- user-facing docs and examples;
- profile validation and reporting workflow.

Exit:

- A supported workflow can be upgraded across versions without data loss or unexplained output drift.
- Support boundaries are narrow but enforceable.

## Near-term sequencing

1. Publish the IR/spec and conformance vectors.
2. Build the release packaging path.
3. Add benchmark gates for large files and streaming behavior.
4. Formalize profile/report workflows.
5. Run technical pilots against the best-fit customer segments.
6. Only then expand into broader CAD, mesh, CNC, laser or robot promises.

## What not to claim yet

Do not claim:

- general slicer replacement;
- turnkey industrial certification;
- broad CNC/laser/robot production support;
- complete non-planar/5-axis workflow;
- stable external IR standard;
- bounded-memory JSON streaming;
- safety guarantees beyond documented verification rules.

The correct current claim is narrower and stronger: Dry is a tested, deterministic toolpath compiler
foundation with working FFF-centered workflows and a clear path to production hardening.

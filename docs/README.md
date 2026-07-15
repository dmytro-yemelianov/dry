# Dry — the toolpath compiler

Specification, roadmap and task backlog for untying from FullControl and building **toolpath compiler
infrastructure** ("LLVM/MLIR for machine motion") — a typed, units-aware, multi-level IR (**Dry IR**) with
a Rust engine and thin multi-language authoring SDKs, built **clean-room** (Apache-2.0, independent of
GPLv3 FullControl) with FullControl used only as design inspiration and a dev/CI behavioural **oracle**
(see `CLEANROOM.md`).

> Status: **working v0 foundation**. The engine, CLI, Python/TypeScript SDKs, wasm/browser surfaces,
> codecs, verifier, optimizer and conformance checks are implemented. These docs now track the path from
> technical foundation to production product.

| Doc | What it covers |
|---|---|
| [`00-vision-and-scope.md`](00-vision-and-scope.md) | The thesis (compiler infrastructure, not a library), in/out of scope, success criteria, relationship to the fork, the honest risk. |
| [`01-architecture.md`](01-architecture.md) | The multi-level IR (L0 design → L1 path → L2 motion → L3 target), the toolframe model, typed units & channels, the pure-functional pass framework, columnar/streaming storage, the engine API, the SDKs, targets/interop, and the behavioural-reference map (clean-room: FC as oracle/inspiration). |
| [`02-roadmap.md`](02-roadmap.md) | Phases P0–P6 with goals, deliverables and hard exit gates; the risk register; sequencing/critical path. |
| [`03-conformance.md`](03-conformance.md) | How correctness is bootstrapped from the fork (5 conformance corpora), the per-phase parity gates + tolerances, the float/determinism discipline, the lessons-as-tests, and the CI shape. |
| [`04-tasks.md`](04-tasks.md) | The actionable backlog per phase (sized, with deps + acceptance) and the immediate next 5. |
| [`05-product-directions.md`](05-product-directions.md) | Expanded product directions: slicer vs CAD workbench, post-slicer Klipper review/optimization, G-code forensics, time-series analysis and LLM-assisted explanations. |
| [`06-lattice-research-codegen.md`](06-lattice-research-codegen.md) | How the star-polygon lattice research PDF maps into the Dry `M1`..`M4` code generator, with decisions and limits. |
| [`07-tpms-codegen.md`](07-tpms-codegen.md) | TPMS implicit-field contour code generator: gyroid, Schwarz P/D, I-WP, Neovius, Fischer-Koch, F-RD and related surfaces. |
| [`08-production-transition.md`](08-production-transition.md) | Production transition plan: readiness definition, workstreams, release gates, milestones and what not to claim yet. |
| [`09-customer-readiness.md`](09-customer-readiness.md) | Customer readiness matrix: best-fit segments, pilot design, product packages and segment-specific gates. |
| [`10-dry-ir-v0-spec.md`](10-dry-ir-v0-spec.md) | Normative Dry IR v0 specification: the JSON wire form, the `DRY0` (columnar) and `DRY1` (chunked streaming) binary encodings, the three version axes, the SemVer/compatibility policy, and the semantic conformance model. Paired with [`../spec/dry-ir-v0.schema.json`](../spec/dry-ir-v0.schema.json) and the public, independently-validated [`../conformance/vectors/`](../conformance/vectors). |
| [`11-profiles-and-reports.md`](11-profiles-and-reports.md) | The machine/material profile schema, the verification rule catalog (stable kebab-case ids + per-rule severities), and the verify/review/trace report schemas. Paired with [`../spec/dry-profile-v1.schema.json`](../spec/dry-profile-v1.schema.json), [`../spec/dry-reports-v1.schema.json`](../spec/dry-reports-v1.schema.json), the example profiles, and the drift-gated [`../conformance/reports/`](../conformance/reports). |
| [`12-releasing.md`](12-releasing.md) | The tagged-release process (`.github/workflows/release.yml`): version/tag guard, CLI binaries + `SHA256SUMS`, Python wheels (maturin), npm package artifact, private GitHub Release distribution, and install-without-source instructions. |
| [`13-performance-and-scale.md`](13-performance-and-scale.md) | The memory model (the `DRY1` streaming path is bounded-memory; JSON/`DRY0` materialize), the criterion benchmarks, and the deterministic bounded-memory scale gate (`tests/memory_scale.rs`). |
| [`14-known-limitations.md`](14-known-limitations.md) | An honest account of current limitations: no slicing, FFF-only emission, experimental 5-axis, v0 IR, semantic (not byte) conformance, the `manualgcode` asymmetry, and the support boundary. |
| [`15-cli-cookbook.md`](15-cli-cookbook.md) | Copy-pasteable, run-verified recipes for every CLI command (inspect/simulate/emit/optimize/verify/pack/unpack and import/review/trace/rewrite-gcode). |
| [`pilots/`](pilots/) | Three pilot guides — [authoring](pilots/authoring.md) (generate→verify→emit), [post-slicer review](pilots/post-slicer-review.md) (review→trace→rewrite), [SDK integration](pilots/sdk-integration.md) (reproduce a vector) — backed by runnable [`../examples/`](../examples). |
| [`16-support-matrix.md`](16-support-matrix.md) | What is Supported / Experimental / Out-of-scope across firmware flavors, file formats, targets, release platforms and workflows. |
| [`17-provenance-and-licensing.md`](17-provenance-and-licensing.md) | The auditable corpus-provenance ledger (oracle-generated vs authored clean-room) and the runtime dependency-license audit (all permissive; the GPL oracle is dev/CI-only). |
| [`site/reference/fullcontrol-sources.md`](site/reference/fullcontrol-sources.md) | Source-to-fixture audit for fullcontrol.xyz, upstream model notebooks/tutorials, the oracle fork gallery, and author gists. |
| [`CLEANROOM.md`](CLEANROOM.md) | The clean-room provenance & licensing discipline (Apache-2.0; FullControl as inspiration + dev/CI oracle only, never code). |

**Read in order.** The one-paragraph summary: don't rewrite the library — promote the IR + Rust engine
to *the product*, generalise it (toolframe, units-as-types, dialects, splines, streaming), grow Python /
TypeScript / Rust front-ends onto the one IR, and gate every step on conformance generated from the FullControl oracle's
~906 tests, golden g-code, ~695 device profiles and 26 exported gallery fixtures from a 27-design
registry — then cut the FC API last. The missing registry fixture is tracked in the
[FullControl source audit](site/reference/fullcontrol-sources.md).

The core thesis was reached in conversation; the supporting argument (why not a blind rewrite, why the
IR is the durable asset) lives in the FullControl fork's `docs/ir_prior_art.md` (standards survey) and `docs/ir_spec.md`
(the fork's hardened IR — a behavioural reference for Dry IR, reimplemented not copied).

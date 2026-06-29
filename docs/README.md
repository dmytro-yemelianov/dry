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
| [`CLEANROOM.md`](CLEANROOM.md) | The clean-room provenance & licensing discipline (Apache-2.0; FullControl as inspiration + dev/CI oracle only, never code). |

**Read in order.** The one-paragraph summary: don't rewrite the library — promote the IR + Rust engine
to *the product*, generalise it (toolframe, units-as-types, dialects, splines, streaming), grow Python /
TypeScript / Rust front-ends onto the one IR, and gate every step on conformance generated from the FullControl oracle's
~906 tests, golden g-code, ~695 device profiles and 27-design gallery — then cut the FC API last.

The core thesis was reached in conversation; the supporting argument (why not a blind rewrite, why the
IR is the durable asset) lives in the FullControl fork's `docs/ir_prior_art.md` (standards survey) and `docs/ir_spec.md`
(the fork's hardened IR — a behavioural reference for Dry IR, reimplemented not copied).

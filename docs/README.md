# Dry — the toolpath compiler (planning)

Specification, roadmap and task backlog for untying from FullControl and building **toolpath compiler
infrastructure** ("LLVM/MLIR for machine motion") — a typed, units-aware, multi-level IR (**Dry IR**) with
a Rust engine and thin multi-language authoring SDKs, reached via a strangler-fig route that bootstraps
from the FullControl fork's hard-won correctness.

> Status: **planning**. Project **Dry**. This is design intent, not yet built.

| Doc | What it covers |
|---|---|
| [`00-vision-and-scope.md`](00-vision-and-scope.md) | The thesis (compiler infrastructure, not a library), in/out of scope, success criteria, relationship to the fork, the honest risk. |
| [`01-architecture.md`](01-architecture.md) | The multi-level IR (L0 design → L1 path → L2 motion → L3 target), the toolframe model, typed units & channels, the pure-functional pass framework, columnar/streaming storage, the engine API, the SDKs, targets/interop, and the reuse map (the ~60–70% taken from the FullControl fork). |
| [`02-roadmap.md`](02-roadmap.md) | Phases P0–P6 with goals, deliverables and hard exit gates; the risk register; sequencing/critical path. |
| [`03-conformance.md`](03-conformance.md) | How correctness is bootstrapped from the fork (5 conformance corpora), the per-phase parity gates + tolerances, the float/determinism discipline, the lessons-as-tests, and the CI shape. |
| [`04-tasks.md`](04-tasks.md) | The actionable backlog per phase (sized, with deps + acceptance) and the immediate next 5. |

**Read in order.** The one-paragraph summary: don't rewrite the library — promote the IR + Rust engine
to *the product*, generalise it (toolframe, units-as-types, dialects, splines, streaming), grow Python /
TypeScript / Rust front-ends onto the one IR, and gate every step on conformance ported from the FullControl fork's
~906 tests, golden g-code, ~695 device profiles and 27-design gallery — then cut the FC API last.

The core thesis was reached in conversation; the supporting argument (why not a blind rewrite, why the
IR is the durable asset) lives in `docs/ir_prior_art.md` (the standards survey) and `docs/ir_spec.md`
(the fork's already-hardened IR, which is Dry IR's prototype).

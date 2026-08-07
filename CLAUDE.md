# dry

Parametric design/CAM DSL: a Rust engine (`crates/core`) that resolves feature-based designs to IR, G-code, and reports, with a CLI (`crates/cli`). Bindings — `crates/wasm` (wasm-bindgen), `crates/cloud` (workers-rs), `py/` (PyO3), `containers/verify-runner` (axum) — are excluded from the Cargo workspace and build standalone with their own locks; all four now have dedicated CI jobs (`wasm`, `python-sdk`, `cloud`, `verify-runner`) — still build and test them locally too before claiming done, since each has its own lock and can drift between CI runs. `sdk/ts` is a separate npm package built from the wasm engine. Formal artifacts live in `proofs/` (numeric contracts and mutation claims), `formal/` (Lean 4), `spec/` (JSON schemas), and `conformance/`.

## Commands

- `cargo test -p dry-core` — engine tests
- `cargo test -p dry-cli` — CLI tests
- Excluded crates (`crates/wasm`, `crates/cloud`, `py/`, `containers/verify-runner`) build and test from their own directories; all four have CI jobs now, but still verify locally too before claiming done.
- `python tools/validate_vectors.py conformance/vectors` — conformance vectors
- `formal/` is a Lean 4 project (lake); see the `formal-assurance` CI job.

## Model routing & Orchestration

Work is orchestrated and verified by **Gemini 3.6 Flash (High)**, offloading hard computational and proof tasks to specialized **NVIDIA API subagents**:

| Work | Route / Subagent | Model & Engine |
|---|---|---|
| **Orchestration, Verification & Routing** | Primary Agent / `reviewer` / `scout` | **Gemini 3.6 Flash (High)** (high-level routing, test suite validation, rule-catalog verification, sitemap synthesis) |
| **Kernel Engineering & Geometry** | `kernel-engineer` | **NVIDIA API (Llama 3.3 70B / DeepSeek V4)** via `scripts/nvidia_subagent.py --profile kernel` (`crates/core` numerics, kinematics, lowering, emitters) |
| **Formal Proofs & Assurance** | `proof-engineer` | **NVIDIA API (DeepSeek-R1)** via `scripts/nvidia_subagent.py --profile proof` (`formal/` Lean 4, `proofs/` error budgets, numeric boundaries) |
| **Heavy Code Generation & Tooling** | `routine-dev` | **NVIDIA API (Llama 3.3 70B / Qwen 2.5 72B)** via `scripts/nvidia_subagent.py --profile heavy-dev` or `scripts/run_aider_nvidia.sh` (bindings, CLI, services, SDKs) |
| **Safety & Contract Audits** | `auditor` | **NVIDIA API (DeepSeek-R1)** via `scripts/nvidia_subagent.py --profile audit` (NaN/inf leaks, unrepresented states, boundary checks) |

Gemini 3.6 Flash (High) handles all orchestration, plan generation, test execution, verification checks, and final sign-offs. Heavy computation, theorem synthesis, and complex kernel code generation are dispatched to NVIDIA API agents (`scripts/nvidia_subagent.py` or `scripts/run_aider_nvidia.sh`).

**Commit attribution.** A subagent signs `Co-Authored-By` with the model it actually ran on — not the dispatching session's. Do not paste your own trailer into a subagent brief; that attributes one model's work to another. If a brief asks you to sign a name that isn't yours, sign yours and say so in your report.

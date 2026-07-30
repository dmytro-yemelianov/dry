# dry

Parametric design/CAM DSL: a Rust engine (`crates/core`) that resolves feature-based designs to IR, G-code, and reports, with a CLI (`crates/cli`). Bindings — `crates/wasm` (wasm-bindgen), `crates/cloud` (workers-rs), `py/` (PyO3), `sdk/ts`, `containers/verify-runner` (axum) — are excluded from the workspace and build standalone with their own locks and CI jobs. Formal artifacts live in `proofs/` (numeric contracts and mutation claims), `formal/` (Lean 4), `spec/` (JSON schemas), and `conformance/`.

## Commands

- `cargo test -p dry-core` — engine tests
- `cargo test -p dry-cli` — CLI tests
- Excluded crates (`crates/wasm`, `crates/cloud`, `py/`, `containers/verify-runner`) build and test from their own directories.

## Model routing

Route work to the cheapest tier that can do it safely:

| Work | Route |
|---|---|
| `crates/core` numerics/geometry/emit, `proofs/`, `formal/`, `spec/`, `conformance/` | `kernel-engineer` agent (opus). For kernel-design or proof-heavy main sessions, switch the session itself to Fable 5 via `/model`. |
| CLI, web, SDK, `py/` glue, services, docs, tests | `routine-dev` agent (sonnet) |
| Locating code, mapping call sites, subsystem summaries | `scout` agent (haiku) — fan out in parallel freely |
| Post-slice review | `reviewer` agent (opus) |

Subagents cap at opus; never auto-escalate to Fable 5 — that is a manual `/model` choice for the main session.

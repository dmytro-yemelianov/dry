# dry

Parametric design/CAM DSL: a Rust engine that resolves feature-based designs to IR, G-code, and reports, layered as `kmet-contracts` (`crates/contracts`, shared vocabulary) → `kmet-kernel` (`crates/kernel`, resolve/emit) → `kmet-verify` (`crates/verify`, rules) → `kmet-trace` (`crates/trace`, reports and analytics), re-exported as one surface by the `dry-core` facade (`crates/core`), with a CLI (`crates/cli`). Bindings — `crates/wasm` (wasm-bindgen), `crates/cloud` (workers-rs), `py/` (PyO3), `containers/verify-runner` (axum) — are excluded from the Cargo workspace and build standalone with their own locks; all four now have dedicated CI jobs (`wasm`, `python-sdk`, `cloud`, `verify-runner`) — still build and test them locally too before claiming done, since each has its own lock and can drift between CI runs. `sdk/ts` is a separate npm package built from the wasm engine. Formal artifacts live in `proofs/` (numeric contracts and mutation claims), `formal/` (Lean 4), `spec/` (JSON schemas), and `conformance/`.

## Commands

- `cargo test -p kmet-kernel` — engine tests (resolve, ir, features, emit, gcode, codec, profile, optimize, generate, units, frame, clothoid, engine, sdk)
- `cargo test -p kmet-verify` — verification-rule tests
- `cargo test -p kmet-trace` — trace/report/forensics/compare/explain/recommend/reverse tests
- `cargo test -p kmet-contracts` — shared-vocabulary tests (`RuleId`, `Severity`, `Contracts*`, `Kinematics`, tolerance constants)
- `cargo test -p dry-core` — `dry-core` is now a pure re-export facade over the four crates above, plus its own cross-layer integration tests; it runs none of the engine's own unit tests
- `cargo test -p dry-cli` — CLI tests
- Excluded crates (`crates/wasm`, `crates/cloud`, `py/`, `containers/verify-runner`) build and test from their own directories; all four have CI jobs now, but still verify locally too before claiming done.
- `python tools/validate_vectors.py conformance/vectors` — conformance vectors
- `formal/` is a Lean 4 project (lake); see the `formal-assurance` CI job.

## Model routing

Route work to the cheapest tier that can do it safely:

| Work | Route |
|---|---|
| `crates/kernel` (numerics/geometry/emit), `crates/verify` (rules), `crates/trace` (analytics), `crates/contracts` (shared vocabulary), `proofs/`, `formal/`, `spec/`, `conformance/` | `kernel-engineer` agent — Opus 5, effort `xhigh` |
| Everything outside those four crates — `crates/core` (now a pure re-export facade plus cross-layer integration tests, no implementation of its own), `crates/cli`, other workspace crates (`llm`, `moonraker`, `license`), bindings (`crates/wasm`, `crates/cloud`, `py/`, `sdk/ts`, `containers/verify-runner`), `web/`, `services/` (TypeScript workers — distinct from the Rust `crates/cloud`), docs, tests | `routine-dev` agent — Sonnet, effort `medium` |
| Locating code, mapping call sites, subsystem summaries | `scout` agent — Haiku; fan out in parallel freely |
| Post-slice review | `reviewer` agent — Opus 5, effort `xhigh` |

Opus 5 at `xhigh` is the ceiling: it is the recommended effort for coding and agentic work, and it is where the correctness-critical slices belong. Both Opus agents pin `claude-opus-5` rather than the `opus` alias, so a model bump is a deliberate edit. Do not escalate a subagent past this tier.

Effort defaults to inheriting the session, so `routine-dev` sets `medium` explicitly rather than inheriting a high session setting for mechanical work. `scout` leaves `effort` unset deliberately — Haiku's effort support is version-dependent and recon does not need the knob. Main-session effort comes from `effortLevel` in `~/.claude/settings.json`; use `/model` to put the session itself on Opus 5 for kernel-design or proof-heavy work.

**Commit attribution.** A subagent signs `Co-Authored-By` with the model it actually ran on — not the dispatching session's. Do not paste your own trailer into a subagent brief; that attributes one model's work to another. If a brief asks you to sign a name that isn't yours, sign yours and say so in your report.

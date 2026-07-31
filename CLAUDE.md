# dry

Parametric design/CAM DSL: a Rust engine (`crates/core`) that resolves feature-based designs to IR, G-code, and reports, with a CLI (`crates/cli`). Bindings — `crates/wasm` (wasm-bindgen), `crates/cloud` (workers-rs), `py/` (PyO3), `containers/verify-runner` (axum) — are excluded from the Cargo workspace and build standalone with their own locks; all four now have dedicated CI jobs (`wasm`, `python-sdk`, `cloud`, `verify-runner`) — still build and test them locally too before claiming done, since each has its own lock and can drift between CI runs. `sdk/ts` is a separate npm package built from the wasm engine. Formal artifacts live in `proofs/` (numeric contracts and mutation claims), `formal/` (Lean 4), `spec/` (JSON schemas), and `conformance/`.

## Commands

- `cargo test -p dry-core` — engine tests
- `cargo test -p dry-cli` — CLI tests
- Excluded crates (`crates/wasm`, `crates/cloud`, `py/`, `containers/verify-runner`) build and test from their own directories; all four have CI jobs now, but still verify locally too before claiming done.
- `python tools/validate_vectors.py conformance/vectors` — conformance vectors
- `formal/` is a Lean 4 project (lake); see the `formal-assurance` CI job.

## Model routing

Route work to the cheapest tier that can do it safely:

| Work | Route |
|---|---|
| `crates/core` numerics/geometry/emit, `proofs/`, `formal/`, `spec/`, `conformance/` | `kernel-engineer` agent — Opus 5, effort `xhigh` |
| Everything outside `crates/core` — CLI, other workspace crates (`llm`, `moonraker`, `license`), bindings (`crates/wasm`, `crates/cloud`, `py/`, `sdk/ts`, `containers/verify-runner`), `web/`, `services/` (TypeScript workers — distinct from the Rust `crates/cloud`), docs, tests | `routine-dev` agent — Sonnet, effort `medium` |
| Locating code, mapping call sites, subsystem summaries | `scout` agent — Haiku; fan out in parallel freely |
| Post-slice review | `reviewer` agent — Opus 5, effort `xhigh` |

Opus 5 at `xhigh` is the ceiling: it is the recommended effort for coding and agentic work, and it is where the correctness-critical slices belong. Both Opus agents pin `claude-opus-5` rather than the `opus` alias, so a model bump is a deliberate edit. Do not escalate a subagent past this tier.

Effort defaults to inheriting the session, so `routine-dev` sets `medium` explicitly rather than inheriting a high session setting for mechanical work. `scout` leaves `effort` unset deliberately — Haiku's effort support is version-dependent and recon does not need the knob. Main-session effort comes from `effortLevel` in `~/.claude/settings.json`; use `/model` to put the session itself on Opus 5 for kernel-design or proof-heavy work.

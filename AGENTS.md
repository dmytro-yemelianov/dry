# dry — Agent Guidelines & Routing (AGY)

Parametric design/CAM DSL: a Rust engine (`crates/core`) that resolves feature-based designs to IR, G-code, and reports, with a CLI (`crates/cli`). Bindings — `crates/wasm` (wasm-bindgen), `crates/cloud` (workers-rs), `py/` (PyO3), `containers/verify-runner` (axum) — are excluded from the Cargo workspace and build standalone with their own locks; all four have dedicated CI jobs (`wasm`, `python-sdk`, `cloud`, `verify-runner`) — still build and test them locally too before claiming done, since each has its own lock and can drift between CI runs. `sdk/ts` is a separate npm package built from the wasm engine. Formal artifacts live in `proofs/` (numeric contracts and mutation claims), `formal/` (Lean 4), `spec/` (JSON schemas), and `conformance/`.

## Commands

- `cargo test -p dry-core` — engine tests
- `cargo test -p dry-cli` — CLI tests
- Excluded crates (`crates/wasm`, `crates/cloud`, `py/`, `containers/verify-runner`) build and test from their own directories.
- `python tools/validate_vectors.py conformance/vectors` — conformance vectors
- `formal/` is a Lean 4 project (`lake`); see the `formal-assurance` CI job.

## Antigravity (AGY) Subagent Routing

Route work to the specialized agent role:

| Agent | Model | Primary Scope & Responsibilities |
|---|---|---|
| `architect` | `pro` / `inherit` | Architecture governance, dialect lowering invariants (L0→L1→L2→L3), ADRs in `docs/adr/`, NFRs (bitwise determinism, bounded memory, compile-time units), cross-language FFI boundary design. |
| `product-owner` | `pro` / `inherit` | Whole-portfolio oversight, roadmap/milestones (`docs/02-roadmap.md`), task breakdown (`docs/04-tasks.md`), cross-target parity enforcement, release readiness. |
| `delivery-lead` | `flash` / `inherit` | Release engineering, milestone gates, dependency sequencing, risk tracking, lockstep version verification (`scripts/check-version.sh`). |
| `qa-assurance` | `pro` / `inherit` | Lean 4 formal verification (`formal/`), conformance oracle vectors (`conformance/`), numeric boundary audits & error budgets (`proofs/`), parser fuzzing. |
| `kernel-engineer` | `pro` / `inherit` | Correctness-critical engine implementation in `crates/core` (resolve, emit, engine, gcode, units, ir, codec, verify), `proofs/`, `formal/`, `spec/`, `conformance/`. |
| `routine-dev` | `flash` / `inherit` | Non-kernel implementation: `crates/cli`, `crates/llm`, `crates/moonraker`, `crates/license`, `web/`, `sdk/`, `py/` glue, `services/`, docs, and tests. |
| `scout` | `flash_lite` / `flash` | Read-only reconnaissance, fast call site mapping, symbol location (`file:line`), subsystem summaries. Fan out in parallel. |
| `reviewer` | `pro` / `inherit` | Post-slice code review, numeric contract verification (`proofs/`), schema parity (`spec/`), cross-target drift audit. |
| `ralph-loop` | `pro` / `inherit` | Autonomous iterative task execution loop: assess state -> plan -> implement -> run verification suites -> iterate until all tests & gates pass. |

## Core Rules & Non-Negotiables

1. **Verify Before Completion**: Run target test suites (`cargo test -p dry-core`, `cargo test -p dry-cli`, or vector suites) before marking any task done.
2. **Contract Preservation**: Changes to resolve/emit semantics must respect `proofs/` claims and `spec/` schemas.
3. **Cross-Target Parity**: Never let Rust core changes drift from bindings (`crates/wasm`, `crates/cloud`, `py/`, `sdk/ts`) without tracking.
4. **Markdown Link Formatting**: Always use clean, workspace-relative markdown links (e.g., `[path/to/file](path/to/file)`). Never use `file:///` URIs.

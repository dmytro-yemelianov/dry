# dry — Agent Guidelines & Routing (AGY)

Parametric design/CAM DSL: a Rust engine (`crates/core`) that resolves feature-based designs to IR, G-code, and reports, with a CLI (`crates/cli`). Bindings — `crates/wasm` (wasm-bindgen), `crates/cloud` (workers-rs), `py/` (PyO3), `containers/verify-runner` (axum) — are excluded from the Cargo workspace and build standalone with their own locks. CI has dedicated `wasm`, `python-sdk`, `cloud`, and `verify-runner` jobs; the cloud crate is compile-gated rather than unit-tested. Run the touched target's CI-equivalent gates locally before claiming completion because root workspace commands do not validate these roots. `sdk/ts` is a separate npm package built from the wasm engine. Formal artifacts live in `proofs/` (numeric contracts and mutation claims), `formal/` (Lean 4), `spec/` (JSON schemas), and `conformance/`.

## Codebase Knowledge Graph

Use codebase-memory-mcp for code discovery in this order:

1. `search_graph` — locate symbols and public surfaces.
2. `trace_path` — map callers, callees, data flow, and cross-service impact.
3. `get_code_snippet` — read an exact symbol after locating it.
4. `query_graph` — analyze complex multi-hop or hotspot questions.
5. `get_architecture` — establish the high-level structure.

Before relying on graph results, verify that the indexed branch, HEAD, and repository root match the frozen review baseline. Use a separate project name for a worktree instead of overwriting an unrelated checkout's index. Fall back to grep/glob only for literals, errors, configuration, non-code files, generated artifacts, or when the graph is insufficient; record the fallback in the evidence.

## Commands

- `cargo test -p dry-core` — engine tests
- `cargo test -p dry-cli` — CLI tests
- Excluded crates (`crates/wasm`, `crates/cloud`, `py/`, `containers/verify-runner`) run their target-specific CI-equivalent gates from their own directories; cloud currently has a compile gate but no unit-test step.
- `python3 tools/validate_vectors.py conformance/vectors` — conformance vectors
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
| `routine-dev` | `flash` / `inherit` | Non-kernel implementation: application crates, `crates/wasm`, `crates/cloud`, `py/` glue, `containers/verify-runner`, `web/`, `sdk/`, `services/`, `tools/license-issuer`, docs, and tests. |
| `scout` | `flash_lite` / `flash` | Read-only reconnaissance, fast call site mapping, symbol location (`file:line`), subsystem summaries. Fan out in parallel. |
| `reviewer` | `pro` / `inherit` | Post-slice code review, numeric contract verification (`proofs/`), schema parity (`spec/`), cross-target drift audit. |
| `ralph-loop` | `pro` / `inherit` | Bounded controller: freeze state, map impact, route specialists, collect independent review and gate evidence, iterate until accepted or explicitly blocked. |

The `Model` column expresses a capability tier, not a provider-specific model identifier. Runtime-supported model names in the local agent frontmatter are authoritative; inherit the current model when the requested tier is unavailable.

## Shared Agent Contract

Every dispatched packet must state the operating mode (`review-only` or `remediation`), objective, acceptance criteria, frozen base/HEAD and dirty-state baseline, authorized paths/systems, graph project, affected targets, required gates, and exit condition.

Source precedence is:

1. The user task and acceptance criteria.
2. The nearest `AGENTS.md` instructions.
3. Accepted ADRs and normative specifications/schemas.
4. Current code, tests, manifests, locks, and CI workflows.
5. Roadmap/task documents for status only.
6. Agent files for role procedure; never use them as authority for volatile repository facts.

Review-only agents do not edit. Remediation agents may edit only their authorized scope and may not weaken ADRs, schemas, proof claims, mutation manifests, conformance goldens, or release gates merely to obtain a pass. Never regenerate a golden or vector during diagnosis; an update requires an independently established contract change and separate review.

Every finding must record severity, confidence, `path:line`, violated contract or invariant, expected and actual behavior, reproducer/evidence, affected targets, owner, and closure command. A material patch is complete only after an independent `reviewer` accepts it; the fixer and `ralph-loop` controller cannot self-certify.

After three materially different attempts with no progress on the same cause, stop the loop and escalate with exact evidence and the smallest decision required. Do not repeat an unchanged failing command indefinitely.

## Core Rules & Non-Negotiables

1. **Verify Before Completion**: Run target test suites (`cargo test -p dry-core`, `cargo test -p dry-cli`, or vector suites) before marking any task done.
2. **Contract Preservation**: Changes to resolve/emit semantics must respect `proofs/` claims and `spec/` schemas.
3. **Cross-Target Parity**: Never let Rust core changes drift from bindings and consumers (`crates/wasm`, `crates/cloud`, `py/`, `sdk/ts`, `containers/verify-runner`) without an explicit owner and gate.
4. **Markdown Link Formatting**: Always use clean, workspace-relative markdown links (e.g., `[path/to/file](path/to/file)`). Never use `file:///` URIs.

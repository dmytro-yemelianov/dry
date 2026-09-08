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

Each role is defined once, in `.claude/agents/<id>.md`. That file is the authoritative role prompt, tool allowlist, and read-only/read-write boundary for the role, whichever surface dispatches it.

| Agent | Task tool | OpenClaw fleet | Primary Scope & Responsibilities |
|---|---|---|---|
| `architect` | `opus` | `nvidia/nemotron-3.5-lightning-30b-a3b` | Architecture governance, dialect lowering invariants (L0→L1→L2→L3), ADRs in `docs/adr/`, NFRs (bitwise determinism, bounded memory, compile-time units), cross-language FFI boundary design. |
| `product-owner` | `opus` | `ollama/gemma4:12b-mlx` | Whole-portfolio oversight, roadmap/milestones (`docs/02-roadmap.md`), task breakdown (`docs/04-tasks.md`), cross-target parity enforcement, release readiness. |
| `delivery-lead` | `sonnet` | `ollama/glm-5.3-flash:cloud` | Release engineering, milestone gates, dependency sequencing, risk tracking, lockstep version verification (`scripts/check-version.sh`). |
| `qa-assurance` | `opus` | `nvidia/deepseek-ai/deepseek-v4-flash` | Lean 4 formal verification (`formal/`), conformance oracle vectors (`conformance/`), numeric boundary audits & error budgets (`proofs/`), parser fuzzing. |
| `kernel-engineer` | `opus` | `nvidia/nemotron-3-ultra-550b-a55b` | Correctness-critical engine implementation in `crates/core` (resolve, emit, engine, gcode, units, ir, codec, verify), `proofs/`, `formal/`, `spec/`, `conformance/`. |
| `routine-dev` | `sonnet` | `ollama/qwen3:14b` | Non-kernel implementation: application crates, `crates/wasm`, `crates/cloud`, `py/` glue, `containers/verify-runner`, `web/`, `sdk/`, `services/`, `tools/license-issuer`, docs, and tests. |
| `scout` | `haiku` | `ollama/llama3.2:3b` | Read-only reconnaissance, fast call site mapping, symbol location (`file:line`), subsystem summaries. Fan out in parallel. |
| `reviewer` | `opus` | `nvidia/nemotron-3-ultra-550b-a55b` | Post-slice code review, numeric contract verification (`proofs/`), schema parity (`spec/`), cross-target drift audit. |
| `ralph-loop` | `opus` | `nvidia/nemotron-3-ultra-550b-a55b` | Bounded controller: freeze state, map impact, route specialists, collect independent review and gate evidence, iterate until accepted or explicitly blocked. |

There are two dispatch surfaces and they are not interchangeable:

- **Task tool** (Claude Code subagents). The `model:` field in `.claude/agents/<id>.md` accepts only Claude aliases (`opus`, `sonnet`, `haiku`, `fable`); a `provider/model` ref there does not load. The frontmatter is authoritative for this surface. Do not add an `effort:` key — an unrecognized frontmatter key silently drops the agent from the registry.
- **OpenClaw fleet** (`sessions_spawn(agentId=…)`). This is the only surface that can run a role on nvidia or local ollama. Each fleet entry pins `cwd` to the repo root and takes its role prompt by reference from `.claude/agents/<id>.md`, so the role is never duplicated.

Verify a model id against `openclaw models list` before putting it in this table. Every id above was checked; note that a doubled provider prefix (`nvidia/nvidia/...`) is accepted as a session pin but is not a valid config model ref.

**Project model infrastructure:**
- **nvidia**: `nvidia/nemotron-3-ultra-550b-a55b` (ultra), `nvidia/nemotron-3-super-120b-a12b` (super), `nvidia/nemotron-3.5-lightning-30b-a3b` (lightning), `nvidia/deepseek-ai/deepseek-v4-flash` (flash) — for correctness-critical, architecture, and numeric audit work
- **ollama** (local, localhost:11434): `ollama/gemma4:12b-mlx`, `ollama/qwen3:14b`, `ollama/llama3.2:3b`, `ollama/qwen2.5:14b-instruct` — for application development, routine tasks, and reconnaissance
- Fallback chain: nvidia → ollama local → Claude

## When to Route Instead of Doing It Inline

Do not implement a big or important change in the main session. Route it, so the work carries a role contract and an independent reviewer.

Route when any of these holds:

- The change touches `crates/core` resolve/emit semantics, `proofs/`, `formal/`, `spec/`, or `conformance/` — always `kernel-engineer`, always with a `reviewer` pass.
- The change spans more than one target, or crosses the Rust core into `crates/wasm`, `crates/cloud`, `py/`, `sdk/ts`, or `containers/verify-runner`.
- The change touches a release gate, CI workflow, or deployment path.
- The task needs more than roughly three files changed, or its acceptance criteria are not yet decided.
- Anything a `reviewer` would have to accept before merge.

Handle inline: single-file edits, a one-line fix with a named failing test, reading and answering a question, or running an existing gate.

For a multi-slice objective, dispatch `ralph-loop` as the controller rather than driving the specialists by hand, and give it the full packet required by the Shared Agent Contract. Reconnaissance fans out: dispatch several `scout` agents in parallel for independent questions before committing to a design.

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

## GitHub Workflow: Projects, Issues, and PR Lifecycle

All agents (especially `product-owner`, `delivery-lead`, and `ralph-loop`) must follow the standardized GitHub issue and project tracking lifecycle across both `dmytro-yemelianov/dry` and the `opentoolpath` organization:

1. **Issue-Driven Development:**
   - Every non-trivial feature, RFC, bug fix, or roadmap task must correspond to a tracked GitHub Issue before or during development.
   - Issues must be assigned to the relevant Milestone (e.g. `v1.0`, `v1.1`, `v1.2`, `v1.3`) and linked to the active GitHub Project board (e.g. `https://github.com/orgs/opentoolpath/projects/1`).
   - The issue description must clearly define: Objective, Acceptance Criteria, Affected Targets, and Required Verification Gates.

2. **Project Board State Tracking:**
   - Work items on the project board must accurately reflect execution state:
     - `Todo`: Backlog and roadmap items ready for development.
     - `In Progress`: Actively assigned to an agent or branch.
     - `Done`: Implemented, verified by local/CI gates, reviewed by an independent `reviewer`, and merged.

3. **Branching & PR Linking:**
   - Develop on descriptive branches: `feat/<name>`, `fix/<name>`, `rfc/<name>`, or `chore/<name>`.
   - Pull Requests must link the tracked issue using GitHub keywords (`Fixes #<id>`, `Closes #<id>`, or `Relates to #<id>`).
   - PR descriptions must include verifiable test command outputs for all affected targets.

4. **Cross-Repository Alignment:**
   - When an engine change in `dry` implements or modifies an OpenToolpath standard feature (e.g. native `.otp` packing/emitting), cross-link the `dry` PR to the corresponding `opentoolpath/spec` or `opentoolpath/conformance` issue.

## Core Rules & Non-Negotiables

1. **Verify Before Completion**: Run target test suites (`cargo test -p dry-core`, `cargo test -p dry-cli`, or vector suites) before marking any task done.
2. **Contract Preservation**: Changes to resolve/emit semantics must respect `proofs/` claims and `spec/` schemas.
3. **Cross-Target Parity**: Never let Rust core changes drift from bindings and consumers (`crates/wasm`, `crates/cloud`, `py/`, `sdk/ts`, `containers/verify-runner`) without an explicit owner and gate.
4. **Markdown Link Formatting**: Always use clean, workspace-relative markdown links (e.g., `[path/to/file](path/to/file)`). Never use `file:///` URIs.
5. **Issue & Project Board Tracking**: Track substantive roadmap milestones, feature additions, and bug remediations via GitHub Issues, linked PRs, and Project boards. Never leave active work unreferenced in the tracking system.

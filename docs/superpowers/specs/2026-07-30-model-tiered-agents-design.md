# Model-Tiered Subagents for dry — Design

Date: 2026-07-30
Status: approved in discussion; implementation pending

## Purpose

Encode a model-routing policy for Claude Code work on this repo so that each kind of task runs on the cheapest model tier that can do it safely. The repo's cost/correctness profile drives the split: `crates/core` (~24k lines of numerics, geometry, IR, G-code emit) is backed by formal artifacts (`proofs/`, `formal/`, `spec/`, `conformance/`) and five binding targets that must stay consistent (native, wasm, cloud, py, TS SDK) — that work justifies Opus-tier models. Everything else (CLI, web, SDK glue, docs, tests) does not.

## Components

Four subagent definitions in `.claude/agents/` plus a new root `CLAUDE.md`.

### 1. `kernel-engineer` — model: opus, all tools

Implementation agent for the correctness-critical surface: `crates/core`, `proofs/`, `formal/`, `spec/`, `conformance/`.

Prompt requirements:
- Run the targeted tests (`cargo test -p dry-core`, plus any touched integration/conformance suites) before reporting done; report failures verbatim.
- Changes to numerics or resolve/emit semantics must be checked against the contracts in `proofs/` (claims TOMLs, numeric-boundary profiles) and the schemas in `spec/`.
- Flag any change whose behavior surfaces through the bindings (wasm/cloud/py/TS) so parity can be re-verified — the bindings build outside the workspace and are easy to silently break.

### 2. `routine-dev` — model: sonnet, all tools

Implementation agent for everything outside the kernel: `crates/cli`, `crates/llm`, `crates/moonraker`, `crates/license`, `web/`, `sdk/`, `py/` glue, `services/`, docs, and test scaffolding. Same run-the-tests discipline (`cargo test -p dry-cli`, or the touched project's own test command), lighter prompt.

### 3. `scout` — model: haiku, tools: Glob, Grep, Read (no Bash — enforced read-only)

Reconnaissance agent: locate code, map call sites, summarize a subsystem before a change. Returns `file:line` references and concise structural maps, not prose essays. Cheap to fan out in parallel.

### 4. `reviewer` — model: opus, tools: Glob, Grep, Read, Bash (can run tests/clippy; no edit tools)

Post-slice review agent with dry-specific checks on top of general bug-finding:
- Coverage-first reporting: report every finding with a confidence level and estimated severity; do not self-filter for importance (filtering happens downstream).
- Check touched code against the numeric contracts in `proofs/` and schemas in `spec/`.
- Check cross-target parity: does the change alter behavior that `crates/wasm`, `crates/cloud`, `py/`, or `sdk/ts` re-expose, and are those surfaces updated?
- Check test/conformance coverage: does the slice add or update tests commensurate with the change?

### 5. `CLAUDE.md` (new, root)

Short and focused:
- One-paragraph project description (parametric design/CAM DSL; engine + CLI in-workspace; `crates/wasm`, `crates/cloud`, `py/`, `containers/verify-runner` excluded and built standalone).
- Essential commands: `cargo test -p dry-core`, `cargo test -p dry-cli`; note the excluded crates have their own locks/CI jobs.
- Model-routing rules:
  - Core numerics / proofs / spec work → `kernel-engineer` agent (opus); Fable 5 via `/model` for kernel-design or proof-heavy main sessions.
  - Routine slices (CLI, web, SDK, docs, tests) → `routine-dev` agent (sonnet).
  - Repo reconnaissance → `scout` agent (haiku), fan out freely.
  - Post-slice review → `reviewer` agent (opus).

## Out of scope

- No hooks, no saved Workflow scripts, no settings.json changes.
- No automatic escalation to Fable 5 ($10/$50) — that remains a manual `/model` choice for the main session; subagents cap at opus.

## Validation

- Agent files use valid frontmatter (`name`, `description`, `model`, `tools`) and appear as available agent types in a new session.
- One smoke delegation to `scout` to confirm routing and read-only behavior.

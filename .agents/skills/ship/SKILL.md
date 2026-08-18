---
name: ship
description: Plan, implement in an isolated worktree branch, verify across engine, CLI, proofs, and binding gates, and independently review before merging in dry. Uses AGY's active model backend (inherit) for all subagents.
---

# Ship Workflow for dry

Execute a complete feature, fix, or mathematical assurance slice for `dry` through the subagent harness:

## Phase 1: Planning & Reconnaissance
1. If the task requires deep code discovery, dispatch a `scout` subagent (`Model: 'inherit'`, `TypeName: 'scout'`) to map `crates/core`, bindings, or `proofs/`.
2. Author an implementation plan artifact detailing the slice scope, mathematical invariants, test coverage, and binding parity impacts.

## Phase 2: Worktree Implementation
1. Invoke the specialized implementer subagent (`kernel-engineer`, `proof-engineer`, or `routine-dev`) in an isolated worktree branch:
   - `Workspace: 'branch'`
   - `Model: 'inherit'`
2. The implementer writes TDD tests and implements changes following `docs/AGENTS.md` and `.agents/rules/coding-quality.md`.

## Phase 3: Verification & Gate Checks
The implementer validates:
- `cargo test -p dry-core`
- `cargo test -p dry-cli`
- `python tools/validate_vectors.py conformance/vectors` (if conformance vectors touched)
- `lake build` in `formal/` (if Lean proofs touched)

## Phase 4: Independent Review
1. Invoke a read-only `reviewer` subagent (`Model: 'inherit'`, `TypeName: 'reviewer'`) to audit the diff in the worktree branch.
2. For numeric/kernel modifications, invoke an `auditor` subagent (`Model: 'inherit'`, `TypeName: 'auditor'`) to verify numerical stability and contract invariance.
3. If blockers are found, route back to the implementer in the worktree.

## Phase 5: Merge & Exit Report
1. Merge the worktree branch into `main`.
2. Run the full verification suite on `main`.
3. Author an operations exit report in `docs/ops/YYYY-MM-DD-<topic>-exit.md` and update `docs/ops/YYYY-MM-DD-session-handover.md`.

# Agent Harness & Multi-Model Balancing

Six named agent roles operating across `.claude/agents/` and `.agents/` (AGY harness) tailored for **dry** (Parametric Design/CAM DSL, Core Geometry/Numerics Engine, Formal Proofs, and Multi-Target Bindings).

**Active Provider Selection:** The orchestrating agent's model setting determines the provider family. All subagents default to `Model: 'inherit'` (using AGY's active backend without external API keys or `-p` flags). When switching the active model in AGY (e.g. Claude Opus ↔ Gemini 3.7 Flash ↔ GPT-4o), all `inherit` subagents automatically follow the selected workhorse.

---

## Multi-Model Role Matrix

| Role | AGY `invoke_subagent` Model | Gemini Workhorse | Claude Code Tier | OpenAI / Reasoning Tier | Primary Responsibility in `dry` |
|---|---|---|---|---|---|
| **orchestrator** | `inherit` | 2.5 / 3.7 Pro | Opus 4.6 / Fable | o3 | Session-opening, roadmap ownership (`docs/02-roadmap.md`, `docs/04-tasks.md`), task sequencing, verification gate sign-offs. |
| **scout** | `inherit` | Flash Lite | Haiku 3.5 | gpt-4o-mini | Read-only reconnaissance: rapid codebase mapping, call-site tracing, dataflow discovery, and subsystem summaries with exact `file:line` references. |
| **kernel-engineer** | `inherit` | 2.5 / 3.7 Flash | Sonnet 4 / Haiku 3.5 | gpt-4.5 | High-integrity TDD in `crates/core` (resolve, emit, engine, gcode, units, ir, codec, verify, tpms, clothoid, kinematics), `spec/`, and `conformance/`. Runs in worktree branches (`Workspace: 'branch'`). |
| **proof-engineer** | `inherit` | 2.5 / 3.7 Pro | Sonnet 4 | o1 / o3-mini | Formal proofs & mathematical assurance: `formal/` (Lean 4 lake builds), `proofs/` (numeric boundary budgets, mutation claims, contract validation). Runs in `Workspace: 'branch'`. |
| **routine-dev** | `inherit` | 2.5 / 3.7 Flash | Sonnet 4 / Haiku 3.5 | gpt-4.5 | Implementation across non-kernel surfaces: `crates/cli`, `crates/llm`, `crates/moonraker`, `crates/license`, `web/`, `sdk/ts`, `py/` bindings, `containers/verify-runner`, `services/`, docs. |
| **reviewer** | `inherit` | 2.5 / 3.7 Pro | Sonnet 4 | gpt-4o | Read-only gatekeeper: audits diffs against numeric contracts in `proofs/`, JSON schemas in `spec/`, cross-target binding parity, and test/conformance vector coverage. |
| **auditor** | `inherit` | 2.5 Pro (Deep Think) | Opus 4.6 / Fable | o1 / o3-mini | Read-only mathematical & safety red-team: numeric stability, NaN/Inf leak prevention, floating-point precision bounds, division-by-zero fences, and AST soundness. |

---

## The Operating Loop

```
user goal / roadmap (docs/02-roadmap.md)
        │
   orchestrator (inherit / Pro / Opus) ─────────────────────────┐
        │ per slice/task                                        │ per milestone exit
        ▼                                                       ▼
      scout (read-only mapping)                           auditor / reviewer (invariant audit)
        │                                                       │ surviving attacks
        ▼                                                       ▼
  kernel-engineer / proof-engineer / routine-dev ◄─ fix loop ─┐  orchestrator routes:
        │ (isolated worktree: Workspace: 'branch')            │  merge / next task
        ▼                                                     │
     reviewer (read-only audit: contracts, parity) ── findings ─┘
        │ approved
        ▼
   git merge to main ──► test gates verified across core & bindings
```

---

## Non-Negotiable Engineering Discipline

1. **Strict Sandboxing**: All code-generating subagents (`kernel-engineer`, `proof-engineer`, `routine-dev`) execute in isolated git worktree branches (`Workspace: 'branch'`). Reviewers and auditors are strictly read-only (`enable_write_tools: false`).
2. **Numeric Contracts & Proof Soundness**: Changes to numerics or resolve/emit semantics must satisfy claims in `proofs/` (`claims.toml`, boundary and mutation TOMLs) and schemas in `spec/`.
3. **Cross-Target Binding Parity**: `crates/wasm`, `crates/cloud`, `py/`, `sdk/ts`, and `containers/verify-runner` re-expose core behavior with independent locks. Any change altering core API/AST must flag binding parity or update bindings.
4. **Tests Before Done**: Every slice must pass its respective test suite (`cargo test -p dry-core`, `cargo test -p dry-cli`, `python tools/validate_vectors.py conformance/vectors`, lake builds) before declaring completion.
5. **Durable Session Handover**: Cross-session progress is recorded in `docs/ops/YYYY-MM-DD-session-handover.md`.

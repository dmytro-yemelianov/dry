# `dry explain` — offline LLM-explanation bundle (Direction 4 v1)

**Date:** 2026-06-29
**Status:** Approved design, ready for implementation
**Branch:** `feat/explain-bundle`

## Problem

Dry already produces deterministic structured analyses of a sliced toolpath — `trace-gcode`
(per-window time-series), `forensics-gcode` (confidence-tagged slicer/settings inference) and
`review-gcode`/`verify` (safety findings). These are rich but dense. Product Direction 4
(`docs/05-product-directions.md` §4) calls for an LLM layer that turns those facts into a plain-language
*"what is this print, why is it slow, what's risky"* readout plus concrete, re-verifiable change
suggestions — **without** the LLM ever touching the math or the safety gate.

## Decisions (resolved during brainstorming)

1. **Offline bundle.** The engine never calls an LLM. `dry explain` deterministically assembles the facts
   plus a curated prompt into a bundle the user pastes into Claude — or that Claude Code / an agent / an
   MCP consumes. This preserves determinism, adds zero network/API-key/HTTP dependency to the engine, and
   composes with any LLM. (An optional online `--llm` path is explicitly deferred.)
2. **Explain + recommendations.** The bundle's prompt asks the model to (a) explain what the print is,
   (b) explain where time goes / why it's slow, (c) flag risks, and (d) propose a prioritized table of
   concrete profile/setting changes — with a **hard guardrail** that any change is a hypothesis that must
   be re-verified by re-running `dry verify` / `review-gcode`, never presented as safe without that gate.
3. **Markdown default, `--json` opt-in.** Default output is a self-contained Markdown briefing (facts as
   readable tables + the prompt block). `--json` emits a structured `{ meta, reports, prompt }` envelope
   for agents/MCP.

## CLI

```
dry explain <file.gcode> [--profile <name|path>] [--window <seconds>] [--json] [--out <path>]
```

- Imports the g-code, runs trace + forensics + verify internally (reusing the existing command
  plumbing), and renders the bundle to stdout (or `--out`).
- `--profile` feeds the verify contracts and trace context. Without it, verify uses default contracts and
  the bundle states that the safety gate ran with defaults.
- `--window` is the trace window (defaults to the `trace-gcode` default).
- Handed raw non-g-code, it fails with the same actionable hint as the other IR/g-code commands.

## Components (isolated)

1. **`crates/core/src/explain.rs`** — a pure builder over the already-computed typed reports:
   ```rust
   pub struct ExplainBundle {
       pub file: Option<String>,
       pub profile: Option<String>,
       pub reports: ExplainReports,   // { trace: TraceReport, forensics: ForensicsReport, verify: ReviewReport }
       pub prompt: String,
   }
   pub fn build_explain_bundle(reports: ExplainReports, meta: ExplainMeta) -> ExplainBundle
   pub fn render_markdown(bundle: &ExplainBundle) -> String
   ```
   No I/O, deterministic. The prompt is a `const` template with light, deterministic interpolation
   (file/profile names + a few headline numbers). `ExplainBundle`/`ExplainReports` are serde
   `Serialize`/`Deserialize` for the `--json` form.
2. **CLI `explain` command** (`crates/cli/src/main.rs`) — wires import → trace + forensics + review into
   the builder, then renders Markdown or JSON.

## The prompt (the core IP)

A curated instruction block embedded in the bundle that:

- **Role:** a 3D-printing / CNC process engineer reading a *deterministic* analysis of a sliced toolpath.
- **Hard guardrail:** the numbers are ground truth from a deterministic engine — do not recompute or
  invent metrics; treat every suggested setting change as a hypothesis that MUST be re-verified by
  re-running `dry verify` / `review-gcode` against the profile, and never present a change as safe
  without that gate.
- **Tasks:** (1) say what the print is, citing forensics with its confidence tags; (2) explain where the
  time goes and why it's slow, from the trace windows; (3) flag risks from the verify findings; (4)
  propose a prioritized, re-verifiable change table (each row: change · expected effect · re-verify note).
- **Output shape:** Summary → Time analysis → Risks → Recommendations table.
- **Model note:** works best with a frontier model — recommend Claude Opus 4.8 (`claude-opus-4-8`); use
  Claude Fable 5 (`claude-fable-5`) for the most demanding analysis. (Pulled from the `claude-api` skill.)

The prompt is a fixed template (no randomness), so bundles are deterministic and golden-testable.

## Data flow

```
file.gcode → import_gcode → IR
          → trace(IR)         → TraceReport
          → forensics(parsed) → ForensicsReport
          → verify(IR, contracts from profile) → ReviewReport
          → build_explain_bundle({trace, forensics, verify}, meta)
          → render_markdown(bundle)  |  serde_json::to_string(bundle)
          → stdout / --out
```

## Error handling

- Non-g-code input → the existing actionable hint (use `import-gcode` / `review-gcode`).
- Missing profile → bundle proceeds with `Contracts::default()` and a note that the gate used defaults.
- Low-confidence forensics → already tagged (`from-comment`/`measured`/`inferred`); surfaced as-is.

## Testing

Deterministic, so drift-gated goldens (mirroring the rewrite/report goldens):

- A fixture `.gcode` → `explain --json` produces a stable `ExplainBundle`, drift-gated under
  `conformance/reports/explain/` and validated against the published schema by the independent
  `tools/validate_reports.py`.
- A Markdown structural check (contains the prompt's guardrail sentence, the three fact sections, and a
  recommendations instruction).
- Unit tests: the bundle carries all three report sections; the prompt contains the guardrail; the
  `--json` shape round-trips.

## Docs

- `docs/15-cli-cookbook.md` — an `explain` recipe.
- `docs/11-profiles-and-reports.md` — the `ExplainBundle` `--json` schema (added to
  `spec/dry-reports-v1.schema.json` as a new `$defs` entry referencing the existing report defs).
- `docs/05-product-directions.md` — mark Direction 4 v1 shipped.

## Scope / YAGNI

v1 is the bundle only. Explicitly deferred:

- The optional online `--llm` path that calls the Claude API directly (keeps the engine pure for now).
- `compare` (two-file deltas) — a separate Direction-3/4 follow-up.

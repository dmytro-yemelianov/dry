# `dry explain --llm` (online closed loop) + `dry compare` — Direction 4 v2

**Date:** 2026-06-30
**Status:** Approved design, ready for implementation
**Branch:** `feat/explain-llm-compare`
**Supersedes-scope:** picks up the two follow-ups explicitly deferred by
`docs/superpowers/specs/2026-06-29-llm-explain-bundle-design.md` (§ Scope/YAGNI).

## Problem

`dry explain` (v1, shipped) assembles a deterministic **offline** bundle — trace + forensics + verify
plus a curated prompt — that the user pastes into an LLM. Product Direction 4 (`docs/05-product-directions.md`
§4) calls for the **online** path: dry itself calls the model, and — crucially — closes the loop so the
deterministic engine *gates* the model's suggestions rather than trusting them. dry's identity is that the
math and the safety gate are ground truth; the LLM advises, dry decides what it can actually run and
measures the result.

This spec also covers `compare` (two-file forensic deltas), the other deferred follow-up, because it shares
the new Claude client.

## Decisions (resolved during brainstorming)

1. **One spec, two phases.** Phase 1 = `explain --llm` (ships first, builds the reusable Claude client).
   Phase 2 = `compare` (offline deterministic diff + `compare --llm` reusing the client).
2. **Closed loop, hybrid action space.** The model proposes freely; dry tags each recommendation
   **executable** (a change dry can actually run — a `rewrite-gcode` mode or a verify-contract override) or
   **advisory** (re-slice-only, which dry cannot measure because it does not slice). dry *applies +
   re-traces + re-verifies* the executable ones with measured before/after numbers and a gate verdict;
   advisory ones are clearly marked **unverified hypotheses** the user applies in their slicer. This is the
   honest "engine gates the LLM" loop: dry only claims measured results for what it executed.
3. **No default model.** `--model <id>` is **required** with `--llm` (error if omitted) — there is never a
   surprise-cost run. Token usage + an estimated USD cost print to **stderr** after each call.
4. **dry-core stays pure.** The network code lives in a new feature-gated crate `dry-llm`; the engine never
   calls an LLM. The deterministic recommendation/result types and the executor live in `dry-core` and are
   fully golden-testable with no network.
5. **explain stays an analyst.** v1 measures + recommends; to *produce* the improved g-code the user runs
   `dry rewrite-gcode --mode <winner>`. An `--emit-best <path>` is explicitly deferred (keeps the
   producer/analyst separation clean).

## Crates & boundary

- **`crates/llm` (`dry-llm`)** — new, the **only** network code. A thin raw-HTTP Anthropic *Messages* client
  (`ureq`, blocking — no async runtime, fits dry's sync CLI). Depends on `dry-core` for the shared schema
  types. Rust has no official Anthropic SDK, so raw HTTP against `POST /v1/messages` is the prescribed path
  (`x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`).
- **`dry-core`** — stays pure (no HTTP, no async). Gains: the shared recommendation/result schema types and
  the deterministic **executor** (`apply_executable`).
- **`dry-cli`** — depends on `dry-llm` behind a `llm` cargo feature. `--llm` code paths are
  `#[cfg(feature = "llm")]`; the default build stays dependency-light (no TLS/HTTP stack unless built with
  `--features llm`).

## CLI surface

```
dry explain <file.gcode> --llm --model <id> [existing explain flags]
            [--max-applies N] [--json] [--out <path>]
```

- `--llm` flips `explain` from the offline bundle to the online closed loop. **Without `--llm`, behavior is
  byte-identical to v1** (the offline bundle path is untouched).
- `--model <id>` is **required when `--llm`** — enforced at runtime (clap parses `--model` as optional;
  the handler dies with an actionable message if `--llm` is set without it). Accepts any model id
  (`claude-opus-4-8`, `claude-sonnet-4-6`, `claude-haiku-4-5`, `claude-fable-5`, …).
- API key from **`ANTHROPIC_API_KEY` only** (no `--api-key` flag — env is safer and keeps keys out of shell
  history). Missing → actionable error.
- `--max-applies N` (default **4**) caps how many *executable* recommendations dry actually runs (bounds
  cost/time). If the model returns more, dry runs the highest-priority N and `log`s exactly what it skipped
  (no silent truncation).
- `--json` emits the full envelope (below). Cost/usage **always** go to stderr so stdout stays clean for
  piping.
- All existing `explain` flags (`--profile`, `--filament-diameter`, `--line-width`, `--layer-height`,
  `--max-flow`, `--bounds`, `--monotonic-z`, `--min-temp`, `--speed-range`, `--window-s`) carry through and
  feed the same import / contracts / trace as today.

## Closed loop (data flow)

```
file → import → trace + forensics + verify → ExplainBundle              (existing, pure dry-core)
     → dry-llm: POST /v1/messages
            system = bundle.prompt
            user   = the three reports as JSON
            output_config.format = RECOMMENDATIONS_SCHEMA                (new, impure dry-llm)
     → AnalysisResponse { summary, time_analysis, risks, recommendations[] }
     → classify each recommendation:  executable | advisory
     → for each executable (≤ max-applies, by priority): dry-core apply_executable
            rewrite  → run the gated mode pipeline → re-trace + re-verify → before/after + verdict
            contract → override the field → re-verify the same toolpath → compliance delta
     → render briefing + results table   |   serde_json envelope
     → cost/usage readout to stderr
```

## Structured output (the model's contract)

The request sets `output_config.format` to a flat JSON schema (`additionalProperties: false`; within
structured-output limits — no recursion, no numeric/length constraints) so the model returns
machine-actionable recommendations rather than prose-only:

```jsonc
{
  "summary": "string",          // what the print is (cites forensics)
  "time_analysis": "string",    // where time goes / why it's slow (from trace)
  "risks": "string",            // verify findings summarised
  "recommendations": [
    {
      "title": "string",
      "rationale": "string",
      "expected_effect": "string",
      "priority": 1,            // integer, 1 = highest
      "action_kind": "rewrite" | "contract" | "advisory",
      "mode":  "safe" | "balanced" | "max",   // present when action_kind == "rewrite"
      "field": "max_flow" | "speed_range" | "bounds" | "monotonic_z" | "min_temp"
             | "max_retraction_distance" | "max_retraction_speed"
             | "max_travel_without_retract" | "first_layer_height_range"
             | "first_layer_speed_range",      // present when action_kind == "contract"
      "value": "string"          // present when action_kind == "contract"; parsed by dry
    }
  ]
}
```

dry maps `action_kind` → executor. **Anything it cannot execute** — unknown `field`, unparsable `value`,
missing `mode`, or `action_kind == "advisory"` — **is demoted to advisory** and rendered as an *unverified
hypothesis* with a short note. The model can never make dry run an arbitrary action; dry only executes from
this known vocabulary. (`field` enum mirrors `dry_core::Contracts`.)

## The executor (pure, in `dry-core`)

```rust
pub enum ExecutableAction {
    Rewrite { mode: OptimizeMode },
    Contract { field: ContractField, value: ContractValue },
}

pub struct MetricSnapshot { pub total_time_s: f64, pub max_flow_mm3_s: f64,
                            pub findings: usize, pub error_count: usize }

pub enum Verdict { Improved, CleanNoGain, Regressed, Informational }

pub struct ExecutionResult {
    pub action: String,           // human label, e.g. "rewrite-gcode --mode balanced"
    pub before: MetricSnapshot,
    pub after: MetricSnapshot,
    pub verdict: Verdict,
    pub note: String,
}

pub fn apply_executable(
    action: &ExecutableAction,
    imported: &ImportedGcode,     // the already-imported toolpath + source map
    contracts: &Contracts,
    window_s: f64,
) -> ExecutionResult
```

- **rewrite**: runs the existing per-span verifier-gated mode pipeline (`safe`/`balanced`/`max`) → re-trace
  + re-verify the rewritten toolpath → `before`/`after` snapshots + `verdict`:
  `Improved` (time or flow improved, still verifies clean), `CleanNoGain` (clean, no measurable gain),
  `Regressed` (rewrite introduced findings — should not happen given the gate, but reported honestly).
- **contract**: clones `Contracts` with the one field overridden → re-verifies the **same** toolpath →
  reports the compliance shift (findings added/removed). `verdict = Informational` — tightening a gate does
  not change the print, it reveals what *now* violates ("adopting `max_flow=12` would flag N segments").

Pure and deterministic → golden + unit tested with no network.

## `dry-llm` surface

```rust
pub struct ClientConfig { pub api_key: String, pub model: String, pub max_tokens: u32 }

pub struct AnalysisResponse {
    pub summary: String, pub time_analysis: String, pub risks: String,
    pub recommendations: Vec<Recommendation>,   // shared type from dry-core
    pub usage: Usage,                            // input_tokens, output_tokens
}

pub enum LlmError { MissingKey, Http(u16, String), Refusal(String), Decode(String), Transport(String) }

pub fn analyze(cfg: &ClientConfig, bundle: &ExplainBundle) -> Result<AnalysisResponse, LlmError>;
```

Internals split into pure, testable pieces:
- `build_request(cfg, bundle) -> serde_json::Value` — body with `model`, `max_tokens`, `system`,
  `messages`, `output_config.format`. No `thinking` block (adaptive-only models 400 on `budget_tokens`;
  structured parsing is cleaner without thinking blocks). Unit-tested.
- `decode_response(json) -> Result<AnalysisResponse, LlmError>` — checks `stop_reason` (maps `"refusal"` →
  `LlmError::Refusal(category)` **before** reading content), then parses the structured text block.
  Unit-tested against sample payloads.
- `analyze` wires `ureq` between them: sets headers, POSTs, maps non-2xx → `LlmError::Http(status, body)`.
  The only line that touches the network.

## Cost readout

After a successful call, `dry-cli` computes cost from `usage` + a per-model price table and prints to
stderr: `model claude-sonnet-4-6 · in 4,210 tok / out 905 tok · ~$0.0262`.

| model id          | input $/1M | output $/1M |
|-------------------|-----------:|------------:|
| claude-opus-4-8   | 5.00       | 25.00       |
| claude-opus-4-7   | 5.00       | 25.00       |
| claude-opus-4-6   | 5.00       | 25.00       |
| claude-sonnet-4-6 | 3.00       | 15.00       |
| claude-haiku-4-5  | 1.00       | 5.00        |
| claude-fable-5    | 10.00      | 50.00       |

Unknown model → print token counts + `(pricing unknown for <model>)` rather than a wrong number.

## Output

- **Markdown (default):** the briefing (Summary → Time analysis → Risks) followed by a **Results table** —
  one row per recommendation: `Change | Expected effect | Status`. Executable rows show measured deltas
  (`time −18%`, `peak flow 15.2→12.8 mm³/s`) and a gate verdict; advisory rows read
  `unverified — apply in your slicer`.
- **`--json` envelope** (`{ meta, bundle, analysis, recommendations, results, usage, cost_usd }`) for
  agents/MCP. Documented, **not** drift-gated (model output is non-deterministic).

## Error handling

| Condition | Behaviour |
|---|---|
| `--llm` without `--model` | die: "`--llm` requires `--model <id>` (e.g. `--model claude-sonnet-4-6`)" |
| `ANTHROPIC_API_KEY` unset | die: "set ANTHROPIC_API_KEY to use `--llm`" |
| non-2xx from the API | die with the status + a body snippet |
| `stop_reason: "refusal"` | die: "model declined (category: …)"; never read content |
| structured-output parse failure | die with the offending text snippet |
| non-g-code input | the existing actionable hint (shared with the other g-code commands) |
| offline path (no `--llm`) | unchanged from v1 |

## Determinism & testing

- The **offline bundle stays the deterministic, drift-gated artifact** — unchanged
  (`conformance/reports/explain/`).
- The `--llm` path is inherently non-deterministic → **not** golden-gated. Instead the deterministic
  sub-parts are tested:
  - `dry-core` executor: golden + unit tests — apply each `ExecutableAction` kind to a fixture `.gcode`,
    assert `before`/`after` snapshots and `verdict`. No network.
  - `dry-llm`: unit-test `build_request` (body/headers/schema shape) and `decode_response` (sample success
    → recommendations; refusal → `Refusal`; HTTP-error mapping). **No live network in CI.**
  - Classification (`Recommendation` → executable vs advisory demotion) unit-tested in `dry-core`.
- No live-network test runs in CI; an optional `#[ignore]`d smoke test may exercise the real endpoint
  locally when `ANTHROPIC_API_KEY` is present.

## Phase 2 — `compare`

```
dry compare <fileA> <fileB> [--profile …] [other import/contract flags]
            [--llm --model <id>] [--json] [--out <path>]
```

- **Offline (deterministic, drift-gated):** `crates/core/src/compare.rs` builds a `CompareDelta`:
  - *forensics delta* — changed slicer / inferred settings (layer model, line width, infill, seam, …),
  - *trace delta* — total/print/travel time and peak-flow deltas,
  - *verify delta* — findings added / removed between the two files.
  Rendered as a side-by-side delta table; `--json` emits the `CompareDelta`. Golden-gated under
  `conformance/reports/compare/`.
- **`--llm`:** sends both analyses + the computed `CompareDelta` + a compare-specific prompt → the model
  narrates "what changed, why it matters, which file is better and why," reusing `dry-llm` (no new network
  code). Same cost readout, same non-deterministic-so-not-gated treatment.
- **Components:** `dry-core compare.rs` (pure delta builder + render), `dry-cli compare` command, `dry-llm`
  reused for the `--llm` narrative.

## Docs

- `docs/15-cli-cookbook.md` — `explain --llm` and `compare` recipes (incl. `ANTHROPIC_API_KEY` + cost note).
- `docs/11-profiles-and-reports.md` — the `--llm --json` envelope and the `CompareDelta` schema.
- `docs/05-product-directions.md` — mark Direction 4 online path / compare progress.
- `CHANGELOG.md` `[Unreleased]`.

## Scope / YAGNI

- v1 ships **`explain --llm` closed loop** first; **`compare`** second (same spec).
- **Deferred:** streaming token output; adaptive thinking / prompt caching; `--emit-best <path>` (explain
  stays an *analyst* — to produce g-code, run `rewrite-gcode`); multi-model fan-out / judge panels; an
  online `compare --llm` "pick the winner and auto-apply" loop; live-network CI tests.

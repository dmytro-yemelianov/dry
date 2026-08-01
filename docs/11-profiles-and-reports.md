# Dry profiles and reports — reference

This is the **normative** reference for Dry's safety workflow contract: the machine/material **profile**
schema, the **verification rule catalog** (stable ids + severities), and the **report output** shapes
for `verify`, `review-gcode` and `trace-gcode`. The machine-readable schemas are
[`../spec/dry-profile-v1.schema.json`](../spec/dry-profile-v1.schema.json) and
[`../spec/dry-reports-v1.schema.json`](../spec/dry-reports-v1.schema.json); worked examples are under
[`../spec/examples/profiles/`](../spec/examples/profiles) and the golden reports under
[`../conformance/reports/`](../conformance/reports).

The rule ids and severities here are generated from one source of truth — the `RuleId` registry in the
engine (`crates/core/src/verify.rs`) — and the report shapes from the typed envelopes in
`crates/core/src/report.rs`. Goldens are drift-gated against the engine and re-validated against the
schemas by an independent Python validator (`tools/validate_reports.py`), the same discipline as the IR
vectors in [`10-dry-ir-v0-spec.md`](10-dry-ir-v0-spec.md).

---

<!-- docs-gen:start profiles-reports-core -->
## 1. Profile schema (v1)

A profile is a small JSON document of the **factual limits** a toolpath is checked against plus the
import defaults needed to lift slicer G-code. `version` MUST be `1`. Unknown keys are ignored
(forward-compatibility). Several fields accept deserialization **aliases** (kept for ergonomics); the
schema validates the canonical names.

| Section | Field (canonical) | Type | Aliases | Notes |
|---|---|---|---|---|
| — | `version` | `1` | — | profile schema version |
| — | `name` | string | — | human-readable, for reports |
| `firmware` | `flavor` | string | — | `marlin` / `klipper` / `duet` / `rs274` / `grbl` / `robot-krl` (`krl`) |
| `machine` | `build_volume` | `[[x_lo,x_hi],[y_lo,y_hi],[z_lo,z_hi]]` mm | `bounds` | |
| `machine` | `feedrate_range` | `[min,max]` mm/min | `speed_range` | extruding moves |
| `machine` | `kinematics.max_acceleration_mm_s2` | mm/s² > 0 | — | drives the `balanced` arc centripetal limit |
| `machine` | `kinematics.max_junction_velocity_mm_s` | mm/s > 0 | — | caps the `balanced` per-junction feedrate |
| `material` | `filament_diameter` | mm > 0 | — | |
| `material` | `max_volumetric_flow_mm3_s` | mm³/s > 0 | `max_flow`, `max_volumetric_flow` | |
| `material` | `min_nozzle_temperature_c` | °C > 0 | `min_temp`, `min_nozzle_temp` | cold-extrusion guard |
| `process` | `line_width` | mm > 0 | — | import/review default |
| `process` | `layer_height` | mm > 0 | — | import/review default |
| `process` | `monotonic_z` | bool | — | require non-decreasing Z |
| `process` | `max_retraction_distance` | mm > 0 | — | |
| `process` | `max_retraction_speed` | mm/min > 0 | — | |
| `process` | `max_travel_without_retraction` | mm > 0 | — | |
| `process` | `first_layer_height_range` | `[min,max]` mm ≥ 0 | — | |
| `process` | `first_layer_speed_range` | `[min,max]` mm/min ≥ 0 | — | |
| — | `start_gcode` / `end_gcode` | string \| `[string]` | — | a multi-line string or a list of command lines |

Validation rules (enforced by `Profile::validate`): `version == 1`; every range has `lo ≤ hi`; positive
fields are finite and `> 0`; range fields are finite with a non-negative lower bound; when present, both
`machine.kinematics` fields are finite and `> 0`. A profile maps to verifier **contracts** (§2), import
params, resolve params and emit params.

**`machine.kinematics`** is a small, optional, **firmware-agnostic** motion model: a max acceleration and a
max junction / square-corner velocity. It has two optional numeric fields:

- `max_acceleration_mm_s2` — the toolhead's maximum acceleration in mm/s². When set, the `peak-acceleration`
  verifier rule (§2) flags arcs whose centripetal acceleration `v²/r` exceeds this value, and the `balanced`
  rewrite mode (§3.4) caps arc speed to respect it.
- `max_junction_velocity_mm_s` — the maximum velocity change allowed at a sharp junction (a per-junction
  feedrate cap), in mm/s. When set, the `junction-velocity` verifier rule (§2, **warning** severity)
  fires on contiguous printing segments with a velocity change Δv exceeding this value, and `balanced`
  mode (§3.4) limits per-junction speed to respect it. This rule is an *advisory* approximation of
  cornering — a flat Δv-vs-limit check, not a reproduction of firmware junction kinematics (Klipper's
  square-corner-velocity feeds a junction-deviation formula that permits larger Δv on shallow corners),
  so on real slicer g-code it may warn on corners the printer handles fine. It is a Warning, never a gate.
  (The `peak-acceleration` rule, by contrast, evaluates every arc — including travel arcs — against the
  acceleration limit.)

Kinematic limits feed the `balanced` optimisation pipeline but are *not* enforced by the core verifier
(they are gated in the rewrite process). Pressure-advance and input-shaper models are deliberately out of
scope for v1.

**Versioning:** profile `version` is independent of the IR schema version. Additive optional fields are a
minor change; removing/renaming/retyping a field or changing a default is a major change (a new
`version`).

## 2. Verification rule catalog

`verify` returns a `Report` of located **findings**. Each finding carries a stable kebab-case **rule id**
and a **severity**. A rule with an "enabling contract" only fires when that contract is supplied (via a
profile or CLI flag); structural rules always run.

A rule is an **error** (a machine-safety, geometric-validity or contract violation) unless it is a
**process/quality advisory** — `Report::ok()` is true and the CLI exit code is `0` when the only findings
are warnings.

| Rule id | Severity | Triggers when… | Enabled by |
|---|---|---|---|
| `finite` | error | a quantity is NaN or infinite | always |
| `travel-extrudes` | error | a travel (non-printing) move deposits material | always |
| `bead` | error | an extruding move has a non-positive bead width or height | always |
| `orientation-not-unit` | error | the toolframe orientation vector is not unit length | always |
| `arc-radius` | error | an arc's endpoint radius disagrees with its start radius | always |
| `bounds` | error | a move leaves the build volume | `machine.build_volume` |
| `max-flow` | error | volumetric flow exceeds the ceiling | `material.max_volumetric_flow_mm3_s` |
| `speed` | error | an extruding feedrate is outside the allowed range | `machine.feedrate_range` |
| `monotonic-z` | error | Z decreases where it must be non-decreasing | `process.monotonic_z` |
| `cold-extrusion` | error | extruding below the minimum nozzle temperature | `material.min_nozzle_temperature_c` |
| `retraction-distance` | error | a retraction distance exceeds the limit | `process.max_retraction_distance` |
| `retraction-speed` | error | a retraction/unretraction speed exceeds the limit | `process.max_retraction_speed` |
| `travel-without-retraction` | **warning** | a travel run exceeds the allowed distance without a retraction | `process.max_travel_without_retraction` |
| `first-layer-height` | **warning** | the first-layer height is outside the allowed range | `process.first_layer_height_range` |
| `first-layer-speed` | **warning** | the first-layer speed is outside the allowed range | `process.first_layer_speed_range` |
| `peak-acceleration` | error | an arc's centripetal acceleration exceeds the machine's max acceleration | `machine.kinematics.max_acceleration_mm_s2` |
| `junction-velocity` | **warning** | a junction's velocity change exceeds the machine's square-corner velocity | `machine.kinematics.max_junction_velocity_mm_s` |
| `unmodeled-gcode` | **warning** | imported/manual G-code is preserved but is outside the verifier's semantic model | always active when present |

The rule set is **closed**: a reader MAY treat an unknown rule id as a forward-compatible addition, but
the engine only emits ids from this table. Adding a rule is a minor change; removing or re-typing one, or
changing a rule's default severity, is a notable change called out in release notes.

## 3. Report outputs

All three outputs are stable JSON contracts (`spec/dry-reports-v1.schema.json`).

### 3.1 `verify --json` → `VerifyReport`

```json
{ "findings": [ { "rule": "bounds", "severity": "error", "segment": 3, "message": "…" } ] }
```

`segment` is the offending segment index or `null` for a whole-toolpath finding.

### 3.2 `review-gcode --json` → `ReviewReport`

```json
{
  "file": "part.gcode",
  "profile": "voron24-abs",
  "segments": 1234,
  "metrics": { "total_time_s": …, "segment_count": 1234, "max_flow_rate": … },
  "findings": [ { "rule": "max-flow", "severity": "error", "segment": 42, "source_line": 991, "message": "…" } ],
  "error_count": 1
}
```

`findings` here are **located** — each adds `source_line`, the original G-code line (or `null`).
`error_count` counts only `error`-severity findings. `file` / `profile` are `null` when not supplied.

### 3.3 `trace-gcode` → `TraceReport`

```json
{ "file": "part.gcode", "profile": null, "trace": { "window_s": 5.0, "segment_count": …, "windows": [ … ] } }
```

`trace` is a `TraceSummary`: totals plus fixed-duration `windows`, each carrying its segment range and —
for imported G-code — its source-line range (`source_line_start`/`source_line_end`, omitted when absent).

### 3.4 `rewrite-gcode --json` → `RewriteReport`

`rewrite-gcode` re-emits imported motion while preserving the non-motion source lines in place. The
`--mode safe|balanced|max` flag turns on a **gated** optimisation pass and `--json` reports its outcome
(`mode` echoes the chosen mode):

```json
{
  "file": "part.gcode",
  "profile": "voron24-abs",
  "mode": "safe",
  "spans_total": 12,
  "spans_accepted": 11,
  "spans_rejected": 1,
  "segment_count_before": 4096,
  "segment_count_after": 3120,
  "metrics_before": { "total_time_s": …, "segment_count": 4096, … },
  "metrics_after": { "total_time_s": …, "segment_count": 3120, … },
  "spans": [
    { "span_index": 7, "accepted": false, "segment_count_before": 240, "segment_count_after": 240, "new_error_rules": ["bounds"] }
  ]
}
```

`metrics_before`/`metrics_after` are whole-file `Metrics` (§3.2) simulated before and after the rewrite.
Each entry of `spans` is the per-span ledger: its `span_index` (source order), whether the rewrite was
`accepted`, the segment count before/after, and `new_error_rules` — the error rule ids the rewrite would
have **introduced** (empty when accepted). `file`/`profile` are `null` when not supplied.

When `--json` is set the `RewriteReport` goes to stdout and the rewritten G-code goes to the (required)
`--out` file. Without `--json`, the rewritten G-code is emitted as before and a one-line accept/reject
summary plus each rejected span's new rules is printed to stderr.

#### The optimisation gate

`--mode` selects how aggressive the per-span rewrite is; all three modes share the **same gate** and
differ only in the IR→IR pipeline they run:

| mode | pipeline | what it does |
| --- | --- | --- |
| `safe` | `merge_collinear` → `arc_fit` | geometry canonicalisation only (no metric or order change beyond arc fitting) |
| `balanced` | `safe` + `adaptive_speed` | also shapes feedrate at sharp junctions / tight arcs; no reordering. Consumes the profile's `machine.kinematics` (§1) when present — its max acceleration sets the arc centripetal limit and its max junction velocity caps the per-junction feedrate; absent, it uses the built-in 500 mm/s² default with no junction cap |
| `max` | `balanced` + `coasting` + `travel_reorder` + `z_hop` | the full order-changing pipeline: trims ooze, reorders independent extrusion runs to cut travel, and lifts the nozzle on travels |

The rewrite is applied **per source motion span** and is *gated* against the verifier (§2): a span's
rewrite is accepted only when it introduces **no new error rule** relative to the same span's input under
the active contracts. Pre-existing input errors do not block the rewrite, and new *warning*-only findings
do not block it; a rejected span passes through verbatim while its neighbours are still rewritten.
Because `balanced`/`max` change feedrates, travel order and Z, the gate is what makes them safe to expose:
e.g. `balanced` is rejected on a span where `adaptive_speed` would scale a junction feedrate below a
`speed_range` minimum (a new `speed` error), and `max` is rejected on a span where `z_hop`'s lowering
move would violate a `monotonic_z` contract (a new `monotonic-z` error). Contracts come from the
`--profile` (a profile maps to contracts via §2); with no profile the gate has no machine contracts and
only the always-on structural invariants (`finite`, `bead`, `arc-radius`, `travel-extrudes`,
`orientation-not-unit`) can reject a span, so a stderr warning is printed. The legacy ungated `--optimize`
/ `--optimize --reorder-travel` flags are unchanged and run the geometry / aggressive pipelines without
the gate.

### 3.5 `explain --json` → `ExplainBundle`

`dry explain` assembles an **offline LLM-explanation bundle**: it runs `trace`, `forensics` and `verify`
internally and emits `{ file, profile, profiled, reports: { trace, forensics, verify }, prompt }`. The
three `reports` are the existing `TraceReport` / `ForensicsReport` / `ReviewReport` verbatim (so each
still validates against its own `$def`); `prompt` is a deterministic, curated instruction block. The
engine never calls an LLM — the bundle is the input you (or an agent/MCP) hand to one. The prompt's hard
rule: every suggested change is a hypothesis that must be re-verified with `dry verify` / `review-gcode`
before it is trusted. Markdown is the default output; `--json` emits this envelope. The schema `$def` is
`ExplainBundle`; the golden lives at `conformance/reports/explain/explain.json`.

### 3.6 `explain --llm --json` → online analysis response

`dry explain --llm --model <id>` calls Claude directly and closes the loop: it classifies the model's
recommendations as executable (re-check under verifier contracts) or advisory (human decision / slicer
rework), applies the executable ones, re-simulates and re-verifies, and reports measured before/after
results. The `--json` envelope is:

```json
{
  "meta": { "file": "part.gcode", "model": "claude-sonnet-4-6", "profiled": true },
  "analysis": {
    "summary": "…",
    "time_analysis": "…",
    "risks": "…"
  },
  "recommendations": [
    {
      "title": "…",
      "rationale": "…",
      "expected_effect": "…",
      "priority": 1,
      "action_kind": "rewrite",
      "mode": "balanced",
      "field": null,
      "value": null
    }
  ],
  "results": [
    {
      "title": "…",
      "result": {
        "action": "rewrite-gcode --mode balanced",
        "before": { "total_time_s": 120.5, "max_flow_mm3_s": 8.2, "findings": 0, "error_count": 0 },
        "after":  { "total_time_s": 110.3, "max_flow_mm3_s": 7.9, "findings": 0, "error_count": 0 },
        "verdict": "improved",
        "note": "time 120.5s -> 110.3s, peak flow 8.20 -> 7.90 mm3/s"
      }
    }
  ],
  "usage": {
    "input_tokens": 1234,
    "output_tokens": 567
  },
  "cost_usd": 0.0042
}
```

**Key difference from `ExplainBundle`:** this envelope is **not drift-gated** — model output is
non-deterministic, so results are advisory and must be reviewed. The deterministic sub-parts (`trace`,
`forensics`, `verify` run inside `--llm`) stay gated; the verifier is the gate for any applied rewrite.
The cost readout (optional, per-model pricing tables in `dry-llm`) is informational only and prints to
stderr alongside token counts.

### 3.7 `compare --json` → `CompareDelta`

`dry compare` runs `trace`, `forensics` and `verify` on two G-code files and reports the forensic
delta: side-by-side metrics, changed settings, and added/removed safety findings. The output is
deterministic, reproducible and golden-tested (like `explain` offline). The schema is:

```json
{
  "slicer": { "before": "Cura", "after": "PrusaSlicer" },
  "time": {
    "total": { "before": 120.5, "after": 110.3, "abs": -10.2, "pct": -8.46 },
    "print": { "before": 110.2, "after": 100.0, "abs": -10.2, "pct": -9.26 },
    "travel": { "before": 10.3, "after": 10.3, "abs": 0.0, "pct": null }
  },
  "peak_flow_mm3_s": { "before": 8.2, "after": 7.9, "abs": -0.3, "pct": -3.66 },
  "layer_count": { "before": 100.0, "after": 100.0, "abs": 0.0, "pct": null },
  "travel_distance_mm": { "before": 2100.0, "after": 2050.0, "abs": -50.0, "pct": -2.38 },
  "retractions": { "before": 45.0, "after": 42.0, "abs": -3.0, "pct": -6.67 },
  "settings": [
    { "field": "declared.layer_height_mm", "before": "0.2", "after": "0.15" },
    { "field": "seam.strategy", "before": "aligned", "after": "scattered" }
  ],
  "findings": {
    "before_count": 2,
    "after_count": 1,
    "added": ["max-flow@2450"],
    "removed": ["speed@1020"]
  }
}
```

`slicer` is `null` when the slicer is unchanged. Each numeric metric is a `ScalarDelta` with
`before`, `after`, `abs` (absolute change), and `pct` (percent change, `null` when `before == 0`).
`settings` lists only the fields that changed (declared and inferred forensics settings). `findings`
keyed as `"<rule>@<source_line>"` so a finding that moved lines reads as removed+added rather than
silently unchanged. The delta itself is **drift-gated** — golden-tested against the engine.

### 3.8 `compare --llm --json` → narrative over the delta

`dry compare --llm --model <id>` calls Claude directly to analyze the forensic delta and produce a
narrative. The `--json` envelope is:

```json
{
  "delta": { … },
  "narrative": {
    "summary": "Layer height reduced, flow optimized, retractions decreased.",
    "what_changed": "Layer height from 0.2mm to 0.15mm. Flow peak capped by retraction discipline. …",
    "why_it_matters": "Finer layers improve surface finish and geometric accuracy. Lower flow reduces nozzle …",
    "better": "b",
    "better_rationale": "B prints 8.5% faster with tighter geometry and lower risk of stringing."
  },
  "usage": {
    "input_tokens": 2048,
    "output_tokens": 512
  },
  "cost_usd": 0.0091
}
```

`delta` is the full `CompareDelta` from §3.7 (deterministic, drift-gated). `narrative` holds the
model's five-field analysis: a summary, what changed, why it matters, which file is better
(`better` is one of `"a"`, `"b"`, or `"either"`), and the
rationale for that judgment. Token usage and cost (optional, `null` for unknown-pricing models) are
informational. **This envelope is NOT drift-gated** — model output is non-deterministic, so it is
advisory only. Use it to understand the forensic delta qualitatively; apply any measured improvements
only after manual review.

<!-- docs-gen:end profiles-reports-core -->
## 4. Stability & conformance

- The profile schema, the rule catalog, and the report schemas are the public contract.
- Golden reports under `conformance/reports/` are generated from engine seeds that exercise **every** rule
  id and drift-gated by `crates/core/tests/report_goldens.rs`.
- `tools/validate_reports.py` independently re-validates every golden report and example profile against
  the published schemas (no `dry-core`), in the `spec-vectors` CI job.

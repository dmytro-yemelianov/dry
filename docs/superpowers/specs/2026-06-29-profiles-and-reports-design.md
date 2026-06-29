# Design: Profile + report schema (Slice D)

**Date:** 2026-06-29
**Status:** Approved — implementation tracked in GitHub issues
**Branch:** `feat/profiles-and-reports` (stacked on `feat/ir-v0-spec`)
**Source docs:** `docs/08-production-transition.md` (§WS3), `docs/09-customer-readiness.md` (tasks #3/#4, print-lab gate); advances issue #27.

## Goal

Make the **safety workflow contract-stable** for the print-lab / post-slicer-QA pilot segment: a published
machine/material **profile** schema with examples, a **stable verification rule catalog** with defined
severities, and **JSON report schemas** for the verify / review / trace outputs — all proven against
generated goldens, the same rigor as the IR vectors in slice A.

## Decisions (from brainstorming)

1. **Lift into core.** A closed `RuleId` registry in `verify.rs`, and typed report-envelope structs for
   the review/trace wrappers (today inline `json!` in the CLI). The report becomes a real, reusable
   contract.
2. **Introduce a warning set now.** Per-rule default severity; reclassify the three process/quality rules
   below to `Warning` (a deliberate behavior change, see §B).
3. **Golden reports + drift gate.** Extend slice A's bless-generator/drift-gate pattern to the report
   outputs, validated against the report schemas by an independent Python validator.

## A. Core changes (`dry-core`)

### A.1 `verify.rs` — closed `RuleId` registry

- `enum RuleId` over the existing ~15 kebab-case rules, with `as_str()` and a `catalog()` returning
  `Rule { id: RuleId, default_severity: Severity, summary: &'static str }`.
- `verify_stream` pushes findings via the registry so each finding's `rule` string and `severity` come
  from one source. The rule set is compile-time-closed (like `SegmentKind`).
- The current full rule set: `finite`, `travel-extrudes`, `bead`, `orientation-not-unit`, `arc-radius`,
  `bounds`, `max-flow`, `speed`, `monotonic-z`, `cold-extrusion`, `retraction-distance`,
  `retraction-speed`, `travel-without-retraction`, `first-layer-height`, `first-layer-speed`.

### A.2 Typed report envelopes (`report.rs`)

- `LocatedFinding { rule, severity, segment, source_line, message }` — a `Finding` plus the resolved
  source line.
- `ReviewReport { file, profile, segments, metrics, findings: Vec<LocatedFinding>, error_count }`.
- `TraceReport { file, profile, trace: TraceSummary }`.
- The CLI `review-gcode --json` and `trace-gcode` build these typed structs (replacing the inline
  `json!`), preserving the current wire shape exactly.

## B. Severity policy (behavior change)

Default severity is a property of each rule:

- **Warning** (process/quality, not machine-unsafe): `travel-without-retraction`, `first-layer-height`,
  `first-layer-speed`.
- **Error** (safety / geometric validity / contract violation): all others.

Consequence: a toolpath whose *only* findings are in the warning set now satisfies `Report::ok()` and
exits `0`. Three core tests (`travel_without_retraction_is_flagged`, `first_layer_height_out_of_range_is_flagged`,
`first_layer_speed_out_of_range_is_flagged`) currently assert `!report.ok()` and will be updated to assert
`report.ok()` **and** `severity == Warning`. Called out in the PR + a `CHANGELOG`/release note.

## C. Docs & schemas

- `docs/11-profiles-and-reports.md` — normative: the profile schema (fields, units, aliases, `version`
  policy), the **rule catalog** (each rule: id · default severity · trigger · enabling contract), the
  severity policy, and the verify / review / trace report shapes. Cross-links the schemas + examples.
- `spec/dry-profile-v1.schema.json` — draft 2020-12 schema for the profile input.
- `spec/dry-reports-v1.schema.json` — `$defs`: `Severity`, `Finding`, `LocatedFinding`, `Metrics`,
  `VerifyReport`, `ReviewReport`, `TraceWindow`, `TraceSummary`, `TraceReport`.

## D. Example profiles

`spec/examples/profiles/` (authored clean-room, kept separate from the oracle-derived
`conformance/profiles/` corpus): Ender3-PLA-Marlin, Voron-ABS-Klipper, Prusa-PETG-Marlin, Bambu-PLA. Each
parses with `Profile::from_json` (so it passes `Profile::validate()`) and validates against the profile
schema.

## E. Golden reports + drift gate

- `conformance/reports/<case>/` bundles — a seed (toolpath + `Contracts` + trace window), and golden
  `verify.json`, `trace.json`, `review.json`. Seeds are designed so that **every** `RuleId` appears in at
  least one golden (incl. the warnings), making the goldens a completeness check on the catalog.
- `crates/core/tests/report_goldens.rs` — bless-generator (`UPDATE_REPORTS=1`) + drift gate: committed
  goldens must match freshly generated reports; a `rule_catalog_is_covered` assertion checks every
  `RuleId` is exercised.

## F. Independent validation + CI

- `tools/validate_reports.py` (stdlib + `jsonschema`, no `dry-core`): validates every golden report under
  `conformance/reports/` against `spec/dry-reports-v1.schema.json`, and every example profile under
  `spec/examples/profiles/` against `spec/dry-profile-v1.schema.json`.
- Added as a step in the existing `spec-vectors` CI job.

## G. Wiring

README + `docs/README` index gain doc 11; cross-links from 08·WS3 and the 09 print-lab/post-slicer gate;
issue #27 advanced/closed.

## Acceptance → 08·WS3 / 09 #3,#4

- ✅ Profile schema reference + 3–5 examples.
- ✅ Stable verification rule IDs + severity definitions (closed registry + catalog doc).
- ✅ JSON report schema for verify + trace (+ review) outputs.
- ✅ Reports stable across runs (drift gate) and schema-valid (independent Python validator).

## Work breakdown (GitHub issues)

- Epic: Slice D — Profile + report schema.
- D1 RuleId registry + severities (`verify.rs`); D2 typed report envelopes + CLI; D3 `docs/11`;
  D4 schemas; D5 example profiles; D6 golden reports + drift gate; D7 Python report validator + CI;
  D8 docs index + #27.

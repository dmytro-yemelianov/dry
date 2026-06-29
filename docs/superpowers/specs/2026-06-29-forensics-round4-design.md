# Design: G-code forensics — round 4 (travel strategy)

**Date:** 2026-06-29
**Status:** Approved — tracked in GitHub issues
**Branch:** `feat/forensics-round4` (off main)
**Source:** issue #29 — the remaining "travel strategy hints".

## Goal

Infer the **travel strategy** from the toolpath, confidence-tagged:

- **Z-hop usage** — count travel moves that lift Z (a Z increase across the travel). `measured`.
- **Retraction discipline** — `retraction_ratio = retractions / travel_moves`. `measured`.
- **Strategy hint** (`inferred`) — classify from the retraction ratio + z-hop:
  - `retract-on-travel` when most travels retract (ratio ≥ 0.7),
  - `combing-likely` when few do (ratio ≤ 0.2 — combing avoids retraction within a region),
  - else `mixed`; append `+ z-hop` when z-hops are present; `none` when there are no travels.

## Report addition

```
ForensicsReport {
  …,
  travel_strategy: TravelStrategy { z_hops: usize, retraction_ratio: f64, hint: String, source: Confidence },
}
```

## Method

Walk the segments once: a **z-hop** is a `travel` segment with both Z defined and `end.z > start.z + ε`.
`retraction_ratio` reuses the existing retraction + travel counts. The hint is a pure function of the
ratio and z-hop count. No new geometry beyond a per-segment Z compare.

## Artifacts

| Path | What |
|---|---|
| `crates/core/src/forensics.rs` | `TravelStrategy`, z-hop/ratio/hint computation, report field |
| `spec/dry-reports-v1.schema.json` | extend `ForensicsReport` (+ `TravelStrategy`) |
| `conformance/reports/forensics*/` | re-blessed goldens |
| `crates/core/tests` | a z-hop + retract-heavy sample (`retract-on-travel + z-hop`); a combing-style sample |
| `docs/15` | note the field |

## Acceptance

- Z-hops counted on a sample with Z-lifting travels; 0 otherwise.
- `retract-on-travel` when every travel retracts; `combing-likely` with few retractions; `+ z-hop` appended when present.
- Re-blessed goldens validate against the extended schema (independent Python check).

## Work breakdown (issues)

- Epic: Forensics round 4.
- N1 travel-strategy computation + report field; N2 schema + re-blessed goldens + tests + docs.

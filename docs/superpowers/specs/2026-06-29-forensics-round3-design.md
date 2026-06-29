# Design: G-code forensics — round 3 (spacing + seam)

**Date:** 2026-06-29
**Status:** Approved — tracked in GitHub issues
**Branch:** `feat/forensics-round3` (off main)
**Source:** issue #29 — "infill spacing" + "seam and travel strategy hints".

## Goal

Two more inferred facts, each confidence-tagged:

1. **Infill spacing / periodicity** — the perpendicular distance between parallel infill lines, using the
   dominant infill angle from round 2. Reported with a regularity note (coefficient of variation of the
   gaps). `inferred`.
2. **Seam-strategy hint** — cluster the start points of outer-wall loops; classify as `aligned`,
   `clustered`, or `scattered` (with the loop count). `inferred`.

## Report additions

```
ForensicsReport {
  …,
  infill_spacing_mm: Estimate,            // inferred; None when < 2 parallel infill lines
  seam: SeamHint { strategy: String, loops: usize, source: Confidence },
}
```

## Method

**Infill spacing.** Take the dominant infill angle θ; project each infill-segment midpoint onto the
perpendicular unit vector (θ+90°); sort the offsets; collapse near-equal offsets (same line, tol 0.05 mm);
the spacing is the **median** adjacent gap. The note carries the gap **CV** as a regularity signal. `None`
with a note when fewer than two distinct parallel lines exist.

**Seam.** Walk the segments; a loop *start* is an outer-wall extruding segment whose predecessor was not
outer-wall-extruding (i.e. it follows a travel or a different feature). Collect the loop-start XYs. With
≥2 loops, compute the centroid and the max distance from it: `< 1 mm → aligned`, `< 5 mm → clustered`,
else `scattered`. Fewer than two loops → `unknown`.

## Artifacts

| Path | What |
|---|---|
| `crates/core/src/forensics.rs` | spacing + seam computation, `SeamHint`, report fields |
| `spec/dry-reports-v1.schema.json` | extend `ForensicsReport` (+ `SeamHint`) |
| `conformance/reports/forensics*/` | re-blessed goldens |
| `crates/core/tests` | spacing on the parallel-infill PrusaSlicer sample; an aligned-seam two-loop sample |
| `docs/15` | note the new fields |

## Acceptance

- Infill spacing inferred on a sample with parallel infill (the PrusaSlicer 45° sample); `None` otherwise.
- Seam classified `aligned` when two loops start at the same XY; `unknown` with < 2 loops.
- Re-blessed goldens validate against the extended schema (independent Python check).

## Work breakdown (issues)

- Epic: Forensics round 3.
- K1 infill spacing/periodicity; K2 seam-strategy hint; K3 schema + goldens + tests + docs.

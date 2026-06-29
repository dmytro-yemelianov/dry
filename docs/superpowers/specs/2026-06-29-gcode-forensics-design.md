# Design: G-code forensics (Slice #29, first cut)

**Date:** 2026-06-29
**Status:** Approved — tracked in GitHub issues
**Branch:** `feat/gcode-forensics` (off main)
**Source:** issue #29 (doc 05 "G-code forensics" direction). Builds on the importer (#25) + trace (#28).

## Goal

Infer slicer behavior from a `.gcode` file and produce an **explainable** forensics report — every fact
tagged with how it was obtained (`from-comment`, `measured`, `inferred`), per the acceptance: *report
clearly marks inferred/probabilistic facts; slicer comments are used when present.*

## Scope (deterministic first cut)

Deliver the defensible, deterministic core; defer the harder probabilistic inference.

**In:**
- **Slicer detection** from header comments (Cura, PrusaSlicer, SuperSlicer, OrcaSlicer/Bambu, Simplify3D,
  ideaMaker; else `unknown`).
- **Feature attribution from comments** — parse `;TYPE:…`, `; FEATURE:…`, `; feature …` markers, map to
  canonical classes (outer-wall, inner-wall, infill, solid-infill, top-bottom, bridge, support,
  skirt-brim, travel, other), attribute each segment via `segment_source_lines`.
- **Per-feature stats** — segments, extruding distance, time, speed range, peak flow.
- **Layer model** — distinct extruding Z levels (count) + layer-height estimate (median Z delta).
- **Estimates** — line width (median `volume / (length × layer_height)`), each an `Estimate{value,
  confidence, note}`.
- **Travel stats** — travel moves, travel distance, retraction count.
- **Hotspots** — tiny-segment density (planner-load indicator).

**Out (noted as future):** infill angle/spacing/periodicity, extrusion-multiplier recovery, seam strategy,
resonance/planner-load modelling beyond tiny-segment density.

## Report (typed, serde)

```
ForensicsReport { slicer, source_lines, segment_count,
                  layers: LayerModel{ layer_count, layer_height_mm: Estimate },
                  line_width_mm: Estimate,
                  features: [FeatureStat{ feature, source, segments, extruding_distance_mm, time_s,
                                          min_speed_mm_min, max_speed_mm_min, peak_flow_mm3_s }],
                  travel: TravelStat{ travel_moves, travel_distance_mm, retractions },
                  hotspots: [Hotspot{ kind, count, note }] }
Estimate { value: Option<f64>, confidence: "measured"|"from-comment"|"inferred", note }
```

## Artifacts

| Path | What |
|---|---|
| `crates/core/src/forensics.rs` | `ForensicsReport` + `analyze_gcode(source, params)` |
| `crates/cli` | `forensics-gcode <file> [--profile/--filament-diameter/--line-width/--layer-height] [--json]` |
| `spec/dry-reports-v1.schema.json` | add `ForensicsReport` (+ sub-types) to `$defs` |
| `conformance/reports/forensics/` | golden `forensics.json` for a marker-bearing sample |
| `examples/sliced-sample.gcode` | a small Cura-style file with `;TYPE:` markers |
| `crates/core/tests` | analyze a marker sample (feature attribution) + a marker-less file (graceful) |
| `docs/15-cli-cookbook.md`, `docs/16-support-matrix.md` | a forensics recipe; mark the workflow |

## Acceptance → #29

- ✅ report marks inferred/probabilistic facts (the `confidence` tag on every estimate/feature)
- ✅ slicer comments are used when present (feature attribution + slicer detection); graceful when absent

## Work breakdown (issues)

- Epic: Slice #29 (first cut) — G-code forensics.
- I1 `forensics.rs` (analyze + report); I2 CLI `forensics-gcode`; I3 schema + golden + tests;
  I4 sample g-code + docs.

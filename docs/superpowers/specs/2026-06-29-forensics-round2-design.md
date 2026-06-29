# Design: G-code forensics — round 2 (probabilistic layer)

**Date:** 2026-06-29
**Status:** Approved — tracked in GitHub issues
**Branch:** `feat/forensics-round2` (off main)
**Source:** issue #29 round 2 — the probabilistic/inference layer on top of the deterministic first cut.

## Goal

Add three concrete inference capabilities to the forensics report, each tagged with confidence so a guess
is never presented as fact:

1. **Declared settings** — extract slicer config from `; key = value` comments (PrusaSlicer / SuperSlicer
   / OrcaSlicer emit a config block): `layer_height`, `extrusion_width`, `fill_angle`, `fill_density`.
   Tagged `from-comment`.
2. **Infill angle inference** — from the geometry: a direction histogram (mod 180°) over segments
   attributed to `infill`; report the dominant angle(s). Tagged `inferred`.
3. **Extrusion-multiplier estimate** — `median(volume / (width × layer_height × length))` over extruding
   moves, using the *declared* extrusion width as the nominal bead when available (else `None` with a
   note). Tagged `inferred`.

## Report additions

```
ForensicsReport {
  …,
  declared: DeclaredSettings {            // all from-comment; None when not present
     layer_height_mm: Option<f64>,
     extrusion_width_mm: Option<f64>,
     infill_angle_deg: Option<f64>,
     infill_density: Option<String>,
  },
  infill_angles_deg: Vec<f64>,            // inferred dominant infill directions (mod 180), [] if no infill
  extrusion_multiplier: Estimate,         // inferred; value None when no declared width
}
```

## Settings parsing

Scan comment lines for `<key> = <value>` (the PrusaSlicer-family config block). Known keys (first match
wins, prefer the generic): `layer_height` / `first_layer_height`; `extrusion_width` /
`perimeter_extrusion_width` / `infill_extrusion_width`; `fill_angle` / `infill_angle`; `fill_density` /
`infill_density`. Cura's base64 `;SETTING_3` block is out of scope.

## Artifacts

| Path | What |
|---|---|
| `crates/core/src/forensics.rs` | `DeclaredSettings`, settings parser, infill-angle histogram, multiplier estimate |
| `spec/dry-reports-v1.schema.json` | extend `ForensicsReport` (+ `DeclaredSettings`) |
| `conformance/reports/forensics/` | re-blessed golden (the existing Cura sample has no config block → declared all null, multiplier null) |
| `examples/sliced-prusa-sample.gcode` | a PrusaSlicer-style sample with a config block + 45° infill |
| `crates/core/tests` | settings extraction + infill-angle + multiplier tests |
| `docs/15` / `docs/16` | note the new fields |

## Acceptance

- Declared settings extracted when a config block is present (from-comment); absent → all `None`.
- Infill angle inferred from geometry on a marker-attributed infill sample.
- Extrusion multiplier estimated when a declared width is available; `None` (with note) otherwise.
- Re-blessed golden validates against the extended schema (independent Python check).

## Work breakdown (issues)

- Epic: Forensics round 2.
- J1 declared-settings parser + report fields; J2 infill-angle inference; J3 extrusion-multiplier estimate;
  J4 schema + golden + sample + tests + docs.

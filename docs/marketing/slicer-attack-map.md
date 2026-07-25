# Slicer Attack Map: Dry

Research date: 2026-07-02

This document uses "attack" in the product/GTM sense: where Dry can flank, complement, displace or embed around
existing slicers. It is not a security target list.

## Strategic Take

Dry should not attack slicers by promising "better slicing" first. That fight is too broad: mesh import, repair,
supports, infill, profiles, UI, printers and material presets are mature in existing products.

Dry should attack the weaker layer around slicers:

```text
slicer output
  -> Dry review / verify / trace / compare / rewrite
  -> accept, warn, reject, upload or archive
```

The wedge is: **your slicer can generate G-code, but Dry tells you whether that G-code is policy-safe, explainable,
reproducible and ready for the machine.**

## Attack Priority

| Slicer | Target users | Dry attack angle | Proof artifact | Priority |
|---|---|---|---|---:|
| OrcaSlicer | Klipper/Bambu/Voron power users, calibration-heavy users | "Calibration and speed still need independent verification before upload." | Orca output corpus -> Dry review/trace/compare report + Moonraker gate | 1 |
| PrusaSlicer | trusted desktop slicer users, farms using stable profiles | "Even trusted profiles need release gates and drift detection." | Compare PrusaSlicer profile versions and report changed risk metrics | 2 |
| UltiMaker Cura | broad hobby/pro/prosumer base, plugin-friendly ecosystem | "400+ settings create policy drift; Dry turns output into enforceable checks." | Cura post-processing/plugin proof that runs Dry after slice | 3 |
| SuperSlicer | advanced tuning users, legacy PrusaSlicer/Slic3r derivative users | "Advanced knobs need structured verification." | SuperSlicer tuned jobs -> Dry findings for flow, speed, first layer and retraction | 4 |
| Bambu Studio | Bambu ecosystem users, high-speed FFF users | "Fast closed workflows still benefit from independent local G-code audit." | Exported G-code review + compare against Orca/Bambu variants | 5 |
| Simplify3D | paid slicer users who value control | "Paid slicing control is not the same as auditable output." | Simplify3D output -> Dry report pack with metrics, findings and rewrite trial | 6 |
| ideaMaker | Raise3D/prosumer users, print farms | "Profile-rich slicers still need independent fleet policy gates." | ideaMaker/Raise3D profile output -> Dry batch review report | 7 |
| Kiri:Moto | browser slicer/CAM users, web workflow builders | "Dry wasm can be embedded as a browser-side verification layer." | Kiri:Moto export -> local/browser Dry verification demo | 8 |
| Lychee Slicer | resin + filament users | "FFF mode can be reviewed; resin is later." | FFF export only; mark resin outside current support | 9 |
| CHITUBOX | resin users | "Not first-wave; resin toolpaths need a different process model." | Watch-list, not attack-now | 10 |
| PreForm | Formlabs users, closed hardware workflow | "Enterprise print prep has audit value, but closed resin workflow is not first." | Later partner/adjacent research only | 11 |
| VoxelDance Additive | industrial additive users | "Industrial build prep is high value but outside FFF first wedge." | Later industrial workflow research | 12 |

## First-Wave Attack Plans

### 1. OrcaSlicer

Why target it:

- OrcaSlicer is popular with advanced FFF users and Klipper/Bambu workflows.
- Its audience already cares about speed, calibration, profiles and machine envelopes.
- These users are more likely to accept a CLI/report layer than beginner slicer users.

Dry wedge:

- profile-aware review of high-speed jobs;
- compare reports between calibration/profile variants;
- Moonraker upload gate after slicing;
- structured findings for max flow, speed, bounds, first-layer and retraction issues.

Positioning:

> Orca helps you tune. Dry helps you prove the resulting G-code is within policy before it reaches the printer.

Proof artifact:

- collect 20 Orca-generated jobs across common Klipper/Bambu/Voron profiles;
- run `review-gcode`, `trace-gcode`, `compare` and optional safe rewrite;
- publish a report pack showing actionable findings and before/after metrics.

### 2. PrusaSlicer

Why target it:

- PrusaSlicer has a strong trust brand, strong profiles and broad adoption.
- A head-on replacement message will not work.
- A "second opinion" release-gate message can work for labs and service bureaus.

Dry wedge:

- profile-version drift detection;
- structured reports for farms using PrusaSlicer as the standard slicer;
- regression checks when changing printer/material/process presets.

Positioning:

> Keep PrusaSlicer. Add Dry as the release gate for profile changes and production jobs.

Proof artifact:

- compare two PrusaSlicer profiles or versions for the same part;
- show metric deltas, verifier findings and changed risk classes.

### 3. UltiMaker Cura

Why target it:

- Cura has broad usage and a plugin/post-processing ecosystem.
- The large settings surface creates room for policy drift and operator inconsistency.

Dry wedge:

- post-processing script or plugin that runs Dry after slicing;
- machine/material policy checks for teams that standardize Cura profiles;
- report export for support and production review.

Positioning:

> Cura gives you settings. Dry turns the resulting G-code into enforceable checks.

Proof artifact:

- minimal Cura post-processing hook that calls Dry CLI;
- sample report for a standard printer/material profile;
- before/after profile-change comparison.

### 4. SuperSlicer

Why target it:

- SuperSlicer users often tune aggressively and understand slicer internals.
- This audience can appreciate strict verification and detailed reports.

Dry wedge:

- structured verification for advanced tuning experiments;
- compare tuned output against safer baseline;
- highlight flow, first-layer and retraction policy violations.

Positioning:

> SuperSlicer exposes power knobs. Dry tells you which outputs remain inside the machine/material envelope.

Proof artifact:

- tuned SuperSlicer jobs with a baseline and aggressive variant;
- Dry compare output showing risk and metric deltas.

### 5. Bambu Studio

Why target it:

- Bambu users care about speed and convenience.
- The ecosystem is more vertically integrated, which makes independent local review a useful story for advanced users.

Dry wedge:

- review exported G-code without changing Bambu's slicer flow;
- compare Bambu Studio and OrcaSlicer outputs for the same machine/material intent;
- local audit reports for teams that do not want to rely only on slicer preview.

Positioning:

> Keep the fast workflow, but add an independent local audit before release.

Proof artifact:

- Bambu Studio vs OrcaSlicer comparison pack;
- Dry reports showing output differences and verifier findings.

## Second-Wave / Partner Targets

### Simplify3D

Attack carefully. Simplify3D users are already paying for control and may be receptive to "control plus audit,"
but Dry cannot sell itself as a full premium slicer replacement yet.

Best wedge:

- independent report layer for paid slicer output;
- compare Simplify3D output to open slicer output;
- sell to teams that need evidence, not just slicing features.

### ideaMaker

Raise3D/ideaMaker workflows fit service bureaus and production users better than casual hobbyists.

Best wedge:

- fleet policy gate;
- batch review;
- profile-governance reports.

### Kiri:Moto

Kiri:Moto is interesting because it is browser-oriented. Dry's wasm runtime could become a client-side verification
engine for web slicer/CAM flows.

Best wedge:

- browser-side Dry verification demo;
- no-install report layer for generated output.

### Resin and industrial slicers

Lychee, CHITUBOX, PreForm and VoxelDance are not first-wave Dry targets because Dry's current strongest wedge is
FFF/G-code review and compiler infrastructure. They are worth monitoring for future process models, report
standards and enterprise workflow integration.

## Attack Vectors by Product Motion

### Post-slicer CLI gate

Best targets:

- OrcaSlicer;
- PrusaSlicer;
- Cura;
- SuperSlicer;
- Bambu Studio;
- Simplify3D.

Command shape:

```bash
dry review-gcode part.gcode --profile printer-material.json --json > dry-report.json
dry trace-gcode part.gcode --window-s 5 > dry-trace.json
dry compare baseline.gcode candidate.gcode --profile printer-material.json
```

### Upload gate

Best targets:

- OrcaSlicer + Klipper/Moonraker;
- PrusaSlicer + OctoPrint/Moonraker;
- Cura + OctoPrint/Moonraker.

Command shape:

```bash
dry upload part.gcode --moonraker http://printer.local --profile printer-material.json
```

### Plugin/post-processing hook

Best targets:

- Cura post-processing ecosystem;
- Fusion/CAD export hooks;
- Grasshopper/Rhino components;
- FreeCAD workbench/macro.

### Compare/regression workflow

Best targets:

- PrusaSlicer profile updates;
- OrcaSlicer calibration variants;
- Bambu Studio vs OrcaSlicer;
- Simplify3D vs open slicer output.

## Messaging Matrix

| Target | Avoid saying | Say instead |
|---|---|---|
| OrcaSlicer | "Dry replaces Orca." | "Dry verifies aggressive Orca output before upload." |
| PrusaSlicer | "Prusa profiles are unsafe." | "Dry catches profile drift and creates production reports." |
| Cura | "Cura has too many settings." | "Dry turns Cura output into enforceable checks." |
| SuperSlicer | "Advanced users make mistakes." | "Dry keeps advanced tuning inside known constraints." |
| Bambu Studio | "Closed ecosystems are bad." | "Dry adds independent local audit for high-speed output." |
| Simplify3D | "Paid slicers are obsolete." | "Paid control still benefits from auditable verification." |

## Proof Corpus to Build

Create `marketing/slicer-corpus/` or an external private corpus with:

- 5 OrcaSlicer jobs: baseline, speed-tuned, flow-tuned, first-layer-tuned, failed/risky known case.
- 5 PrusaSlicer jobs: stable profile, modified profile, version drift, different material, known good baseline.
- 5 Cura jobs: stock profile, custom profile, high-speed profile, support-heavy job, first-layer-sensitive job.
- 3 SuperSlicer jobs: conservative, aggressive, tuned retraction.
- 3 Bambu Studio jobs: default, speed profile, Orca comparison.

For each job:

- input slicer and version;
- printer/material profile;
- raw G-code;
- Dry review JSON;
- Dry trace JSON;
- optional compare report;
- short human interpretation.

## Source Notes

- Prusa: [PrusaSlicer](https://www.prusa3d.com/p/prusaslicer/).
- UltiMaker: [Cura](https://ultimaker.com/software/ultimaker-cura/).
- OrcaSlicer: [GitHub project](https://github.com/SoftFever/OrcaSlicer).
- Bambu Lab: [Bambu Studio GitHub project](https://github.com/bambulab/BambuStudio).
- SuperSlicer: [GitHub project](https://github.com/supermerill/SuperSlicer).
- Simplify3D: [software features](https://www.simplify3d.com/products/simplify3d-software/).
- Raise3D: [ideaMaker](https://www.raise3d.com/ideamaker/).
- Kiri:Moto: [Grid.Space Kiri:Moto](https://grid.space/kiri/).
- Mango3D: [Lychee Slicer](https://mango3d.io/lychee-slicer-for-sla-resin-3d-printers/).
- CHITUBOX: [slicer products](https://www.chitubox.com/en).
- Formlabs: [PreForm](https://formlabs.com/software/preform/).
- VoxelDance: [VoxelDance Additive](https://www.voxeldance.com/additive).

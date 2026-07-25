---
title: Slicer Attack Map
pageClass: marketing-page
---

<section class="market-hero">
  <p class="market-eyebrow">Competitive wedge</p>
  <h1>Slicer attack map</h1>
  <p class="market-lede">
    Dry should not attack slicers by claiming better slicing first. The stronger wedge is after slicing:
    review, verify, trace, compare, rewrite and gate the G-code before it reaches the machine.
  </p>
  <div class="market-actions">
    <a href="#attack-priority">Priority</a>
    <a href="#first-wave-plans">First wave</a>
    <a href="#proof-corpus">Proof corpus</a>
  </div>
</section>

## Strategic Take

Dry's sharper positioning:

<div class="market-flow" aria-label="Dry slicer attack workflow">
  <div>Slicer output</div>
  <span>-></span>
  <div>Dry review / verify / trace / compare / rewrite</div>
  <span>-></span>
  <div>Accept, warn, reject, upload or archive</div>
</div>

The message is: **your slicer can generate G-code, but Dry tells you whether that G-code is policy-safe,
explainable, reproducible and ready for the machine.**

## Attack Priority

| Slicer | Target users | Dry attack angle | Proof artifact | Priority |
|---|---|---|---|---:|
| OrcaSlicer | Klipper/Bambu/Voron power users | Calibration and speed still need independent verification before upload. | Orca corpus -> Dry review/trace/compare + Moonraker gate | 1 |
| PrusaSlicer | trusted desktop slicer users, farms | Even trusted profiles need release gates and drift detection. | Compare PrusaSlicer profile versions and risk metrics | 2 |
| UltiMaker Cura | broad hobby/pro/prosumer base | Large settings surface creates policy drift. | Cura post-processing/plugin proof that runs Dry after slice | 3 |
| SuperSlicer | advanced tuning users | Advanced knobs need structured verification. | Flow, speed, first-layer and retraction findings | 4 |
| Bambu Studio | Bambu ecosystem, high-speed FFF users | Fast closed workflows still benefit from independent local audit. | Bambu vs Orca comparison pack | 5 |
| Simplify3D | paid slicer users who value control | Paid slicing control is not the same as auditable output. | Dry report pack for Simplify3D output | 6 |
| ideaMaker | Raise3D/prosumer users, print farms | Profile-rich slicers still need fleet policy gates. | ideaMaker/Raise3D batch review report | 7 |
| Kiri:Moto | browser slicer/CAM users | Dry wasm can become browser-side verification. | Browser verification demo | 8 |
| Lychee Slicer | resin + filament users | FFF mode can be reviewed; resin is later. | FFF export only | 9 |
| CHITUBOX | resin users | Resin process model is later. | Watch-list | 10 |
| PreForm | Formlabs users | Closed resin workflow is not first-wave. | Later partner research | 11 |
| VoxelDance Additive | industrial additive users | High value but outside FFF first wedge. | Later industrial research | 12 |

## First-Wave Plans

<div class="market-grid">
  <article>
    <p class="card-label">Priority 1</p>
    <h3>OrcaSlicer</h3>
    <p>Attack aggressive tuning and Klipper/Bambu/Voron workflows with independent verification.</p>
    <ul>
      <li>Review high-speed jobs against real profiles.</li>
      <li>Compare calibration/profile variants.</li>
      <li>Gate Moonraker upload after slicing.</li>
    </ul>
  </article>
  <article>
    <p class="card-label">Priority 2</p>
    <h3>PrusaSlicer</h3>
    <p>Do not attack trust directly. Sell Dry as the production release gate.</p>
    <ul>
      <li>Detect profile-version drift.</li>
      <li>Compare output after printer/material preset changes.</li>
      <li>Create structured reports for farms and labs.</li>
    </ul>
  </article>
  <article>
    <p class="card-label">Priority 3</p>
    <h3>UltiMaker Cura</h3>
    <p>Turn the broad settings surface into the reason teams need enforceable checks.</p>
    <ul>
      <li>Build a post-processing hook proof.</li>
      <li>Run Dry after slicing.</li>
      <li>Export JSON reports for support or production review.</li>
    </ul>
  </article>
  <article>
    <p class="card-label">Priority 4</p>
    <h3>SuperSlicer</h3>
    <p>Advanced tuning users can understand and value strict verification.</p>
    <ul>
      <li>Compare conservative vs aggressive tuned jobs.</li>
      <li>Highlight flow, first-layer and retraction policy violations.</li>
      <li>Use the output as evidence for tuning decisions.</li>
    </ul>
  </article>
  <article>
    <p class="card-label">Priority 5</p>
    <h3>Bambu Studio</h3>
    <p>Add independent local audit without changing the fast Bambu workflow.</p>
    <ul>
      <li>Review exported G-code.</li>
      <li>Compare Bambu Studio and OrcaSlicer variants.</li>
      <li>Package as a local audit/report story.</li>
    </ul>
  </article>
  <article>
    <p class="card-label">Priority 6</p>
    <h3>Simplify3D</h3>
    <p>Attack the gap between paid control and auditable verification.</p>
    <ul>
      <li>Run Dry reports on premium slicer output.</li>
      <li>Compare to open slicer output.</li>
      <li>Sell evidence, not another slicing UI.</li>
    </ul>
  </article>
</div>

## Attack Vectors

### Post-slicer CLI gate

Best targets: OrcaSlicer, PrusaSlicer, Cura, SuperSlicer, Bambu Studio and Simplify3D.

```bash
dry review-gcode part.gcode --profile printer-material.json --json > dry-report.json
dry trace-gcode part.gcode --window-s 5 > dry-trace.json
dry compare baseline.gcode candidate.gcode --profile printer-material.json
```

### Upload gate

Best targets: OrcaSlicer + Klipper/Moonraker, PrusaSlicer + OctoPrint/Moonraker, Cura + OctoPrint/Moonraker.

```bash
dry upload part.gcode --moonraker http://printer.local --profile printer-material.json
```

### Compare/regression workflow

Best targets:

- PrusaSlicer profile updates;
- OrcaSlicer calibration variants;
- Bambu Studio vs OrcaSlicer;
- Simplify3D vs open slicer output.

## Messaging Matrix

| Target | Avoid saying | Say instead |
|---|---|---|
| OrcaSlicer | Dry replaces Orca. | Dry verifies aggressive Orca output before upload. |
| PrusaSlicer | Prusa profiles are unsafe. | Dry catches profile drift and creates production reports. |
| Cura | Cura has too many settings. | Dry turns Cura output into enforceable checks. |
| SuperSlicer | Advanced users make mistakes. | Dry keeps advanced tuning inside known constraints. |
| Bambu Studio | Closed ecosystems are bad. | Dry adds independent local audit for high-speed output. |
| Simplify3D | Paid slicers are obsolete. | Paid control still benefits from auditable verification. |

## Proof Corpus

Build a slicer corpus with:

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

- [PrusaSlicer](https://www.prusa3d.com/p/prusaslicer/)
- [UltiMaker Cura](https://ultimaker.com/software/ultimaker-cura/)
- [OrcaSlicer](https://github.com/SoftFever/OrcaSlicer)
- [Bambu Studio](https://github.com/bambulab/BambuStudio)
- [SuperSlicer](https://github.com/supermerill/SuperSlicer)
- [Simplify3D](https://www.simplify3d.com/products/simplify3d-software/)
- [ideaMaker](https://www.raise3d.com/ideamaker/)
- [Kiri:Moto](https://grid.space/kiri/)
- [Lychee Slicer](https://mango3d.io/lychee-slicer-for-sla-resin-3d-printers/)
- [CHITUBOX](https://www.chitubox.com/en)
- [PreForm](https://formlabs.com/software/preform/)
- [VoxelDance Additive](https://www.voxeldance.com/additive)

The full implementation map is maintained with the private product source.

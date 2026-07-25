---
title: Market Research
pageClass: marketing-page
---

<section class="market-hero">
  <p class="market-eyebrow">Marketing intelligence</p>
  <h1>Dry market research</h1>
  <p class="market-lede">
    Dry is best positioned as a deterministic toolpath review, verification and compiler layer for additive
    manufacturing teams that need machine-motion output they can inspect, gate, reproduce and embed.
  </p>
  <div class="market-actions">
    <a href="#ideal-customer-profile">ICP</a>
    <a href="#competitive-landscape">Competition</a>
    <a href="#package-strategy">Pricing</a>
    <a href="/marketing/gcode-machine-saas">G-code SaaS</a>
    <a href="/marketing/printer-capability-library">Printer library</a>
    <a href="/marketing/slicer-attack-map">Slicer map</a>
    <a href="/marketing/cad-embedding">CAD embedding</a>
  </div>
</section>

<section class="market-strip">
  <article>
    <strong>$24.2B</strong>
    <span>Wohlers 2026 reported AM market size for 2025.</span>
  </article>
  <article>
    <strong>EUR 11.33B</strong>
    <span>AMPOWER 2026 industrial metal/polymer AM market estimate for 2025.</span>
  </article>
  <article>
    <strong>$6.78B</strong>
    <span>ResearchAndMarkets AM software revenue forecast by 2033.</span>
  </article>
</section>

## Strategic Positioning

Dry should not start as a slicer replacement, MES, print-farm dashboard or visual-only G-code viewer. The first
commercial wedge is a narrower, defensible layer:

<div class="market-flow" aria-label="Dry go-to-market workflow">
  <div>CAD / slicer / generated G-code</div>
  <span>-></span>
  <div>Dry review / trace / verify / rewrite / report</div>
  <span>-></span>
  <div>Upload gate, CI gate, or embedded product workflow</div>
</div>

**Best one-line positioning:** Dry is a deterministic toolpath compiler and verification layer for teams that
need machine-motion output they can inspect, gate, reproduce and embed.

## Ideal Customer Profile

<div class="market-grid">
  <article>
    <p class="card-label">ICP 1</p>
    <h3>Post-slicer QA operators</h3>
    <p>Print farms, service bureaus, internal additive labs and production cells reviewing many files before release.</p>
    <ul>
      <li>20+ jobs reviewed per week, or high-value jobs where one failure is costly.</li>
      <li>3-5+ hours/week spent on review, triage, re-slicing or postmortem work.</li>
      <li>Can run a CLI in a post-processing, upload or CI-like workflow.</li>
    </ul>
  </article>
  <article>
    <p class="card-label">ICP 2</p>
    <h3>Integrator platform teams</h3>
    <p>Teams building slicers, CAD/CAM tools, print-management products, manufacturing SaaS or extrusion tools.</p>
    <ul>
      <li>Product already imports, emits, displays or routes G-code or equivalent motion files.</li>
      <li>Roadmap includes verification, traceability, optimization or cross-language SDK support.</li>
      <li>Buyer values a commercially licensed clean-room core over GPL-coupled code.</li>
    </ul>
  </article>
  <article>
    <p class="card-label">ICP 3</p>
    <h3>Toolpath R&D teams</h3>
    <p>University labs, consultancies, process-development teams and advanced makers generating custom paths.</p>
    <ul>
      <li>Ad hoc scripts are no longer reproducible enough.</li>
      <li>Need exact G-code/IR plus metrics for machine trials.</li>
      <li>Want reusable evidence for publications, demos or client work.</li>
    </ul>
  </article>
</div>

### ICP Fit Score

Use this scorecard before committing pilot effort.

| Criterion | Points |
|---|---:|
| Recurring production or review volume | 2 |
| Measurable failed-job, rework or manual-review cost | 2 |
| Clear profile/policy owner | 2 |
| Ability to run CLI/SDK in workflow | 2 |
| Clear buyer with pilot budget | 2 |

**8-10 points:** pursue pilot. **5-7 points:** nurture. **0-4 points:** not first-wave.

## Buyer Map

| Role | What they care about | Dry message | Objection to expect |
|---|---|---|---|
| Operations manager | fewer failed starts, faster release-to-print | Standardize pre-print review and reduce operator-dependent checks. | We already trust our slicer. |
| Lab manager | process consistency and documentation | Profile-backed reports for repeatable review. | Who maintains profiles? |
| Staff engineer | correctness, APIs, maintainability | One deterministic core across CLI/Python/TS/wasm. | Can we trust the IR contract? |
| CTO / VP Eng | roadmap velocity and support burden | Embed a tested toolpath compiler layer instead of writing one. | Is this stable enough to depend on? |
| Quality owner | traceability and evidence | Source-located findings and stable rule IDs. | How complete is the rule catalog? |

## Competitive Landscape

For tactical positioning against individual slicers, see the [slicer attack map](/marketing/slicer-attack-map).

<div class="market-grid wide">
  <article>
    <h3>Slicers</h3>
    <p><strong>Examples:</strong> PrusaSlicer, UltiMaker Cura, OrcaSlicer, Simplify3D.</p>
    <p><strong>Stance:</strong> complement first. Dry validates, traces, compares, transforms and embeds workflows after or below the slicer.</p>
  </article>
  <article>
    <h3>AM workflow / MES</h3>
    <p><strong>Examples:</strong> 3YOURMIND, AMFG, Authentise, Materialise workflow modules.</p>
    <p><strong>Stance:</strong> potential customer or integration partner. Dry understands toolpath artifacts below the MES layer.</p>
  </article>
  <article>
    <h3>Build-prep engines</h3>
    <p><strong>Examples:</strong> Materialise Magics, Dyndrite.</p>
    <p><strong>Stance:</strong> adjacent technical market. Avoid direct LPBF/build-prep competition early.</p>
  </article>
  <article>
    <h3>Print-farm tools</h3>
    <p><strong>Examples:</strong> SimplyPrint, Printago, OctoPrint, Moonraker ecosystems, 3DPrinterOS.</p>
    <p><strong>Stance:</strong> integrate as a preflight gate before upload, queueing or print start.</p>
  </article>
  <article>
    <h3>G-code viewers</h3>
    <p><strong>Examples:</strong> OctoPrint GCode Viewer, gcode.ws, PrintPal viewer, MeshInspector viewer, NC Viewer.</p>
    <p><strong>Stance:</strong> visual inspection is not enough. Dry sells structured findings, contracts and reports.</p>
  </article>
</div>

## Package Strategy

<div class="market-grid">
  <article>
    <p class="card-label">First wedge</p>
    <h3>Deterministic Review + Audit</h3>
    <p>For print farms, service bureaus and labs.</p>
    <ul>
      <li><code>review-gcode</code>, <code>trace-gcode</code>, structured reports and profile gates.</li>
      <li>Optional source-preserving rewrite after re-verification.</li>
      <li>Best initial pricing test: $1,500-$5,000 fixed pilot, then $99-$499/month team package.</li>
    </ul>
  </article>
  <article>
    <p class="card-label">Second offer</p>
    <h3>Embed SDK</h3>
    <p>For product/platform engineering teams.</p>
    <ul>
      <li>Commercial SDK/support license around CLI, Python, TypeScript and wasm surfaces.</li>
      <li>Best initial pricing test: $5,000-$15,000 developer pilot, then $15,000-$75,000/year support.</li>
    </ul>
  </article>
  <article>
    <p class="card-label">Proof channel</p>
    <h3>Research Authoring + Verification</h3>
    <p>For labs, consultancies and R&D groups.</p>
    <ul>
      <li>Workshops and project-specific path-generation support.</li>
      <li>Best initial pricing test: $2,000-$10,000 workshop or $10,000-$50,000 integration.</li>
    </ul>
  </article>
</div>

## G-code Machine SaaS

The broader service opportunity is a hosted control plane for any machine whose production artifact is G-code:

```text
G-code / controller config / CAM or slicer profile / sender state
  -> Dry machine adapters
  -> capability pack + review report + proof run
  -> registry, API, CI gate, edge agent and dashboard
```

That gives Dry a SaaS path around public analysis, private machine registries, proof retention, fleet drift checks
and integrations with CAD/CAM, slicers and sender tools. See the dedicated
[G-code Machine SaaS plan](/marketing/gcode-machine-saas).

## Printer Capability Library

Printers remain the best first vertical. Dry can turn fragmented printer configuration into a shared API and
package format:

```text
Klipper / Moonraker / OctoPrint / Cura / Prusa profiles
  -> Dry printer capability pack
  -> CLI, SDK, registry, checks, samples and proofs
```

This would give Dry a central product layer for printer properties, macros, calibration samples, proof artifacts
and runtime profile resolution. See the dedicated [Printer Capability Library plan](/marketing/printer-capability-library).

## CAD Embedding

Dry's most visible "compiler layer" package is a CAD connector. The first useful integrations should keep the host
CAD system responsible for geometry and UI, while Dry handles IR, verification, reports and target output.

Recommended host order:

| Host | First motion | Why | Priority |
|---|---|---|---:|
| Fusion 360 | local add-in | quickest polished CAD MVP, good maker/R&D overlap | 1 |
| Rhino/Grasshopper | components | best procedural toolpath fit | 2 |
| Onshape | cloud app + FeatureScript helper | strong cloud/platform story | 3 |

See the dedicated [CAD embedding page](/marketing/cad-embedding) for host-by-host MVPs, packaging and sources.

## Pilot Design

### Print farm preflight

Inputs: 25-50 historical G-code files, 2-3 printer/material profiles, operator notes on known issues and a target
report format.

Success criteria:

- catches at least 3 actionable classes of issues;
- reduces manual review time by 30%+ on the corpus;
- produces reports understandable by non-engine operators;
- no accepted rewrite creates a new verifier error.

### Platform embed

Inputs: one product workflow that imports/emits/displays G-code or Dry IR, one conformance vector target and one
report output requirement.

Success criteria:

- external product calls Dry without shell-only hacks;
- conformance vector reproduces;
- integration owner can explain error/report semantics;
- API gaps become specific roadmap items.

## Source Notes

- [Wohlers press archive](https://wohlersassociates.com/category/press-releases/)
- [AMPOWER Report 2026 summary](https://additive-manufacturing-report.com/)
- [ResearchAndMarkets via Business Wire](https://www.businesswire.com/news/home/20250506950344/en/Additive-Manufacturing-Software-Markets-Report-2025-Analysis-Data-and-Forecast---Revenues-Expected-to-Hit-%246.78B-by-2033---ResearchAndMarkets.com)
- [3YOURMIND distributed manufacturing software](https://www.3yourmind.com/distributed-manufacturing-software)
- [AMFG additive MES guide](https://www.amfg.ai/whitepapers/additive-manufacturing-mes-software-the-essential-guide)
- [Authentise workflow management](https://www.authentise.com/)
- [Materialise Magics](https://www.materialise.com/en/industrial/software/magics-data-build-preparation)
- [Dyndrite](https://www.dyndrite.com/)
- [PrusaSlicer](https://www.prusa3d.com/p/prusaslicer/)
- [UltiMaker Cura](https://ultimaker.com/software/ultimaker-cura/)
- [SimplyPrint pricing](https://simplyprint.io/pricing)
- [Printago](https://printago.io/)
- [OctoPrint](https://octoprint.org/) and [GCode Viewer](https://docs.octoprint.org/en/main/bundledplugins/gcodeviewer.html)
- [Moonraker file API](https://moonraker.readthedocs.io/en/latest/external_api/file_manager/)
- [Klipper configuration reference](https://www.klipper3d.org/Config_Reference.html)

The full implementation report is maintained with the private product source.

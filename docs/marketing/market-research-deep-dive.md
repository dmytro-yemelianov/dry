# Marketing Deep Research: Dry

Research date: 2026-07-02

## Executive Summary

Dry should be positioned as a **deterministic toolpath review, verification, and compiler layer** for additive
manufacturing workflows. The first commercial wedge is not "another slicer" and not a full additive MES. The
strongest wedge is a narrow but valuable layer between slicers/CAD tools and printers:

```text
CAD / slicer / generated G-code
  -> Dry review / trace / verify / rewrite / report
  -> upload, CI gate, or embedded product workflow
```

The best initial market is teams that already experience repeatable toolpath QA pain:

- print farms and service bureaus with recurring review/rework cost;
- technical labs and production cells running Klipper/Moonraker or mixed FFF fleets;
- product teams that need an embeddable motion-analysis layer rather than a full slicer;
- R&D groups that need reproducible, inspectable toolpath generation.

The core GTM package should be **Deterministic Review + Audit**: profile-backed G-code review, traceable
reports, source-located findings, and optional re-verified rewrites. The second package should be **Embed
SDK** for software teams that want Dry inside their own CAD, slicer, print-management, or manufacturing
workflow product.

## Market Context

Additive manufacturing software is growing, but the market is crowded at the obvious layers:

- Broad AM industry revenue remains meaningful: Wohlers 2026 reports the AM market at **$24.2B in 2025**.
- AMPOWER reports the 2025 industrial metal/polymer AM market at **EUR 11.33B**, up **5.7%** from 2024.
- ResearchAndMarkets/AM Research estimates AM software revenue rising from **$2.44B** to **$6.78B by 2033**.
- The AM software market is splitting into broad "core workflow" systems and application/process-specific
  software. Dry should target the latter: a focused compiler/verification layer.

Implication: there is enough market budget for AM software, but generic workflow software and slicers are
already saturated. Dry should not start by competing on full print management, quoting, mesh prep, or beginner
slicing UX.

## Strategic Positioning

### Best one-line positioning

Dry is a deterministic toolpath compiler and verification layer for teams that need machine-motion output they
can inspect, gate, reproduce, and embed.

### What Dry should not claim first

- not a turnkey consumer slicer;
- not a full MES;
- not a closed-loop machine tuning platform;
- not a generic print-farm dashboard;
- not a visual-only G-code viewer.

### What Dry can credibly claim

- typed IR and deterministic lowering;
- CLI + Python + TypeScript + wasm integration surfaces;
- profile-aware review/verify/trace/rewrite reports;
- support for post-slicer G-code inspection and policy gates;
- clean-room proprietary implementation available under a commercial embedding agreement.

## Ideal Customer Profile (ICP)

### ICP 1: Post-Slicer QA Operator

**Firm profile:** print farms, service bureaus, internal additive labs, manufacturing test cells, and advanced
FFF operations that review many files before release.

**Best buyer:** operations manager, print-lab manager, technical operations lead, founder/operator of a print
service business.

**Champion:** senior operator, production engineer, or automation-minded maker responsible for repeatable output.

**Trigger event:**

- failed jobs after slicer/profile changes;
- too much manual review of G-code;
- inconsistent checks across operators;
- need to approve files before Moonraker/Klipper upload or print start.

**ICP qualification criteria:**

- 20+ jobs reviewed per week, or recurring high-value jobs where one failure is costly;
- at least one person spends more than 3-5 hours/week on review, triage, re-slicing, or postmortem work;
- they can name failure classes: bounds, flow, speed, temp, retraction, first-layer, firmware/planner issues;
- they have a profile owner or can accept a machine/material profile as a process artifact;
- they can run a CLI in a post-processing, upload, or CI-like step.

**Dry entry package:** Deterministic Review + Audit.

**Why they pay:** reduced failed starts, faster review, less operator dependency, better audit trail.

### ICP 2: Integrator Platform Team

**Firm profile:** teams building slicers, CAD/CAM tools, print-management products, robotics/extrusion tools, or
manufacturing SaaS products that touch toolpath output.

**Best buyer:** CTO, VP Engineering, principal engineer, platform product owner.

**Champion:** staff engineer or technical lead responsible for motion, G-code, profiles, or browser/SDK surfaces.

**Trigger event:**

- duplicated parser/analyzer/emitter logic;
- need for deterministic outputs across CLI, browser, and SDK;
- customer demand for reports or policy gating;
- uncertainty around ownership, licence scope, and embeddability.

**ICP qualification criteria:**

- product already imports, emits, displays, or routes G-code or equivalent motion files;
- roadmap includes verification, traceability, optimization, or cross-language SDK support;
- team has the technical capacity to run conformance fixtures;
- buyer values a commercially licensed clean-room core rather than GPL-coupled code.

**Dry entry package:** Embed SDK.

**Why they pay:** lower engineering maintenance, faster feature delivery, less correctness risk, cleaner support
surface.

### ICP 3: Toolpath R&D / Applied Research Team

**Firm profile:** university labs, engineering consultancies, advanced maker groups, non-planar printing teams,
process-development teams.

**Best buyer:** principal investigator, R&D manager, program manager, consulting principal.

**Champion:** researcher or engineer writing custom Python/TypeScript toolpaths.

**Trigger event:**

- ad hoc scripts are not reproducible;
- researchers need exact G-code/IR plus metrics;
- path generation must be verified before machine trials;
- need to move from demo scripts to a repeatable pipeline.

**Dry entry package:** Research Authoring + Verification.

**Why they pay:** faster iteration with fewer hidden assumptions and reusable evidence for publications, demos,
or client work.

### Anti-ICP

- casual hobby users with low print volume and no repeatable QA pain;
- teams looking for full slicer parity today;
- teams that cannot define ownership for printer/material profiles;
- enterprise buyers who require mature MES features before toolpath validation;
- teams that want automatic closed-loop machine calibration as the core product.

### ICP Fit Score

Score each account out of 10:

| Criterion | Points |
|---|---:|
| Recurring production or review volume | 2 |
| Measurable failed-job, rework, or manual-review cost | 2 |
| Clear profile/policy owner | 2 |
| Ability to run CLI/SDK in workflow | 2 |
| Clear buyer with pilot budget | 2 |

Qualification:

- **8-10:** ICP-qualified, pursue pilot.
- **5-7:** nurture, needs clearer pain/budget.
- **0-4:** not a first-wave account.

## Buyer Map

| Role | What they care about | Dry message | Objection to expect |
|---|---|---|---|
| Operations manager | fewer failed starts, faster release-to-print | "standardize pre-print review and reduce operator-dependent checks" | "we already trust our slicer" |
| Lab manager | process consistency and documentation | "profile-backed reports for repeatable review" | "who maintains profiles?" |
| Staff engineer | correctness, APIs, maintainability | "one deterministic core across CLI/Python/TS/wasm" | "can we trust the IR contract?" |
| CTO / VP Eng | roadmap velocity and support burden | "embed a tested toolpath compiler layer instead of writing one" | "is this stable enough to depend on?" |
| Quality/compliance owner | traceability and evidence | "source-located findings and stable rule IDs" | "how complete is the rule catalog?" |
| Founder/operator | time saved and margin protection | "review more jobs with less manual triage" | "what is the ROI vs print-farm tools?" |

## Pain Points Resolved

### 1. Manual G-code review does not scale

**Pain:** operators inspect files manually or rely on slicer previews, which makes review quality inconsistent.

**Dry response:** `review-gcode`, `trace-gcode`, structured reports, source-line findings, and profile-backed
contracts.

**Commercial outcome:** lower review time, fewer missed issues, more repeatable policy enforcement.

### 2. Slicer/profile changes create silent behavior drift

**Pain:** a software update or profile change shifts output behavior without a clear gate.

**Dry response:** deterministic IR, conformance fixtures, verification reports, and `compare`/trace workflows.

**Commercial outcome:** teams can compare output before approving new slicer/profile settings.

### 3. Print-farm tools manage jobs but rarely own toolpath semantics

**Pain:** fleet dashboards help route jobs, but many do not deeply validate whether the G-code is safe or within
process policy.

**Dry response:** a focused pre-upload or pre-print gate that can sit before Moonraker/OctoPrint/fleet systems.

**Commercial outcome:** Dry complements print-farm software instead of replacing it.

### 4. Engineering teams duplicate fragile parser/analyzer logic

**Pain:** product teams build their own partial G-code parser, metrics estimator, profile logic, or browser preview
bridge.

**Dry response:** shared Rust core with CLI, Python, TypeScript, and wasm surfaces.

**Commercial outcome:** lower integration maintenance and fewer correctness regressions.

### 5. Rewrites and optimization are hard to trust

**Pain:** modifying G-code can create subtle machine behavior changes.

**Dry response:** source-preserving rewrites, explicit optimization modes, and re-verification before accepting
changes.

**Commercial outcome:** controlled optimization path instead of unbounded post-processing.

## Competitive Landscape

For a tactical slicer-by-slicer attack plan, see [`slicer-attack-map.md`](slicer-attack-map.md).

### Category 1: Slicers

Examples: PrusaSlicer, UltiMaker Cura, OrcaSlicer/Bambu Studio lineage, Simplify3D.

**What they do well:** mesh-to-G-code, profiles, supports, slicing settings, previews, broad user adoption.

**Evidence:** PrusaSlicer emphasizes free/open-source local slicing and tested profiles. Cura emphasizes an
open-source slicing engine and 400+ settings.

**Dry differentiation:** Dry should not compete for basic slicing. It should validate, trace, compare, transform,
and embed toolpath workflows after or below the slicer.

**Competitive stance:** complement first, compete later only if Dry grows a mesh-slicing frontend.

### Category 2: AM workflow / MES platforms

Examples: 3YOURMIND, AMFG, Authentise, Materialise order/MES modules.

**What they do well:** quoting, order intake, scheduling, traceability, shop-floor workflow, compliance workflows.

**Evidence:** 3YOURMIND describes order and production software for catalogs, auto-pricing, scheduling, and
traceability. AMFG positions around AM MES and workflow automation. Authentise targets compliance, production
scale, and operational transparency.

**Dry differentiation:** Dry is below MES: it understands motion/toolpath artifacts more deeply. MES platforms
are potential integration partners or customers, not only competitors.

**Competitive stance:** sell as a toolpath-quality engine for MES platforms that need stronger G-code/IR
semantics.

### Category 3: Build-prep and advanced toolpath engines

Examples: Materialise Magics/Build Processor, Dyndrite.

**What they do well:** industrial build preparation, automation, machine-specific build processing, advanced
toolpath APIs, high-end production workflows.

**Evidence:** Materialise Magics centers on data/build preparation and process automation. Dyndrite emphasizes a
GPU-accelerated engine and manufacturing toolkit.

**Dry differentiation:** proprietary, typed, embeddable, FFF/post-slicer review oriented, with an explicit
clean-room commercial licensing story. Dry is not trying to own high-end LPBF build prep first.

**Competitive stance:** adjacent technical market; avoid direct head-on positioning until Dry has stronger
industrial machine support.

### Category 4: Print-farm and printer-control software

Examples: SimplyPrint, Printago, OctoPrint, Mainsail/Fluidd/Moonraker ecosystems, 3DPrinterOS.

**What they do well:** queueing, upload, remote control, monitoring, users, files, business/order workflows.

**Evidence:** SimplyPrint prices a print-farm plan around multi-printer management, APIs, maintenance, inventory,
files, profiles, and permissions. Printago focuses on e-commerce order flow into production jobs. OctoPrint
emphasizes browser-based control/monitoring, plugins, and a built-in G-code viewer. Moonraker exposes file upload
and file-management APIs.

**Dry differentiation:** Dry is not a fleet dashboard. Dry is the preflight gate and report layer those dashboards
can call.

**Competitive stance:** integration surface. Package Dry as "review before upload" or "policy gate before print".

### Category 5: G-code viewers/analyzers

Examples: OctoPrint GCode Viewer, gcode.ws, PrintPal viewer, MeshInspector viewer, NC Viewer.

**What they do well:** visualize toolpaths, estimate time/material, inspect layers.

**Evidence:** OctoPrint documents its bundled GCode Viewer. Several public browser tools offer visualization and
basic statistics.

**Dry differentiation:** visual inspection is not enough. Dry should emphasize structured rule findings, profile
contracts, reproducible reports, and programmatic integration.

**Competitive stance:** compare against these only when selling the "viewer plus policy gate" story.

## White Space

Dry fits a gap between slicers, print-farm management, and MES:

```text
Slicers: create G-code
Dry: verify/trace/compare/rewrite/report G-code
Print-farm tools: route/start/monitor jobs
MES: manage orders, schedules, quality records
```

The white space is **machine-motion policy enforcement**:

- Is this file within machine/material limits?
- Which source lines caused findings?
- What changed between two generated files?
- Can this rewrite be accepted without creating new verifier errors?
- Can an external product embed this analysis in a deterministic way?

## Pricing Hypotheses

These are starting hypotheses, not validated price points.

### Package 1: Deterministic Review + Audit

Buyer: print farms, service bureaus, labs.

Model:

- time-limited private CLI evaluation for qualified pilots;
- paid team package for profile library, batch review, report templates, support, and upload integration;
- optional hosted/team dashboard later.

Initial pricing test:

- **Pilot:** $1,500-$5,000 fixed pilot for 2-4 weeks, including setup and report calibration.
- **Team subscription:** $99-$499/month for small teams, depending on printer count and report volume.
- **Enterprise/lab support:** $5,000-$25,000/year when procurement, profiles, or CI integration matter.

Reasoning: print-farm management software publicly anchors low-end fleet tools around tens to hundreds of dollars
per month, while enterprise compliance/workflow tools are sales-led. Dry should not charge like a full MES at
first, but can charge above commodity viewers because the value is policy gating and support.

### Package 2: Embed SDK

Buyer: platform/product engineering teams.

Model:

- commercial SDK license delivered through authenticated private artifacts;
- paid support, compatibility assurances, and integration help;
- optional OEM/white-label terms for product embedding.

Initial pricing test:

- **Developer pilot:** $5,000-$15,000 for integration proof and conformance harness.
- **Commercial SDK support:** $15,000-$75,000/year depending on distribution, support SLA, and feature requests.
- **OEM/custom:** custom pricing where Dry becomes a shipped product dependency.

Reasoning: the value is engineering replacement cost and correctness risk reduction, not seat count.

### Package 3: Research Authoring + Verification

Buyer: labs, consultancies, R&D groups.

Model:

- paid workshops, support, and project-specific implementation;
- approved public case studies and limited examples to drive adoption without distributing the engine.

Initial pricing test:

- **Workshop/support:** $2,000-$10,000 per engagement.
- **Project integration:** $10,000-$50,000 for custom path-generation or machine-specific work.

Reasoning: R&D users may not convert to recurring SaaS quickly, but can fund high-signal projects and
approved case studies.

## GTM Recommendation

### Start with this offer

**Deterministic Review + Audit for post-slicer workflows**

Position:

> Catch risky G-code before it reaches the printer. Dry reviews toolpaths against machine/material profiles and
> produces source-located, reproducible reports your team can gate, archive, and automate.

Workflow:

```text
slicer output
  -> dry review-gcode --profile printer-material.json --json
  -> accept/warn/reject
  -> optional trace/rewrite
  -> upload to Moonraker/OctoPrint or hand to farm software
```

Why this is first:

- fits current Dry capabilities;
- avoids competing with mature slicers;
- has clear buyer pain;
- creates data for later dashboard/SDK packaging;
- can integrate into existing printer-control workflows.

### Second offer

**Embed SDK for toolpath platform teams**

Position:

> Add deterministic toolpath parsing, verification, reports, and compiler-grade IR to your product without writing
> and maintaining a custom engine.

Why this is second:

- likely higher ACV;
- longer sales cycle;
- requires cleaner API stability, support expectations, and integration docs.

### CAD connector path

CAD embedding deserves its own package path because it is where Dry's "compiler layer" story becomes visible to
users before they adopt a full SDK relationship.

Best early hosts:

| Host | First motion | Why | Priority |
|---|---|---|---:|
| Fusion 360 | local add-in | quickest polished CAD MVP, good maker/R&D overlap | 1 |
| Rhino/Grasshopper | components | best procedural toolpath fit | 2 |
| Onshape | cloud app + FeatureScript helper | strong cloud/platform story | 3 |
| FreeCAD | workbench/macro | open validation channel | 4 |
| Blender | add-on | visual/demo surface for unusual deposition | 5 |
| SOLIDWORKS | enterprise add-in/bridge | high-value but heavy delivery burden | 6 |

Recommended pilot packaging:

- **CAD Connector Pilot:** $7,500-$25,000 for a 4-6 week host integration proof.
- Deliverables: one host connector, one machine/material profile, one validated workflow, one report template,
  documented limitations and next-step backlog.

Full detail: [`cad-embedding-playbook.md`](cad-embedding-playbook.md).

## Discovery Plan

Run 20 interviews in two waves:

### Wave 1: Operator ICP

Targets:

- 8 print farms/service bureaus;
- 4 internal additive labs or makerspaces with real utilization;
- 2 Klipper/Moonraker-heavy advanced operators.

Questions:

1. How many jobs do you review weekly before print?
2. What is the cost of a failed start, failed print, or late-detected issue?
3. How do you approve slicer/profile updates today?
4. What classes of G-code problems do you actually catch manually?
5. Could a CLI or upload hook fit your workflow?
6. Who owns machine/material profiles?
7. What report would make this worth paying for?

### Wave 2: Platform ICP

Targets:

- 5 slicer/CAD/print-management product teams;
- 3 manufacturing SaaS/integration teams;
- 2 robotics/extrusion toolpath teams.

Questions:

1. What parser, verifier, or emitter logic do you maintain today?
2. What correctness bugs create the most support load?
3. Do customers ask for explainable reports or preflight checks?
4. What are your licensing constraints?
5. Would wasm + native + Python/TS support reduce delivery cost?
6. What would block embedding a third-party toolpath core?

## Pilot Design

### Pilot A: Print farm preflight

Inputs:

- 25-50 historical G-code files;
- 2-3 printer/material profiles;
- operator notes on known issues;
- target report format.

Success criteria:

- catches at least 3 actionable classes of issues;
- reduces manual review time by 30%+ on the corpus;
- produces reports understandable by non-engine operators;
- no accepted rewrite creates a new verifier error.

### Pilot B: Platform embed

Inputs:

- one product workflow that imports/emits/displays G-code or Dry IR;
- conformance vector target;
- one report output requirement.

Success criteria:

- external product calls Dry without shell-only hacks;
- conformance vector reproduces;
- integration owner can explain error/report semantics;
- API gaps become specific roadmap items.

## Messaging

### Homepage/product copy

- "A deterministic toolpath compiler and review layer for additive manufacturing workflows."
- "Verify G-code before it reaches the printer."
- "Turn slicer output into source-located findings, trace reports, and policy gates."
- "Embed toolpath analysis across CLI, Python, TypeScript, and browser workflows."

### What to avoid

- "the best slicer";
- "automatic print success";
- "AI fixes your prints";
- "enterprise MES replacement";
- "safe for all machines/materials".

## Roadmap Implications

To sell the first package, the highest-leverage product work is:

1. batch review command or documented batch recipe;
2. example profiles for Klipper/Marlin/common FFF printer classes;
3. JSON report examples and a short report interpretation guide;
4. Moonraker upload-gate documentation;
5. comparison workflow for slicer/profile updates;
6. clear "supported vs experimental" language on rewrites.

To sell the second package:

1. SDK stability policy;
2. integration guide with conformance fixture;
3. language-specific error taxonomy;
4. package release path and version compatibility story;
5. embeddable browser/wasm example.

## Risks and Unknowns

| Risk | Why it matters | Mitigation |
|---|---|---|
| Operators trust slicer previews enough | reduces willingness to pay | sell report/gate automation, not visualization |
| Profile setup is too hard | blocks adoption | ship profile templates and import helpers |
| Rewrites feel unsafe | slows optimization upsell | lead with review-only mode |
| MES/farm tools add similar checks | compresses differentiation | become embeddable engine, not dashboard competitor |
| Industrial buyers need non-FFF depth | raises delivery risk | keep first ICP FFF/post-slicer, mark industrial as later |
| Open-source users resist subscriptions | lowers SMB revenue | charge for team workflow, support, batch, reports, integration |

## Source Notes

- Wohlers press archive: [Wohlers Report 2026 values AM market at $24.2B](https://wohlersassociates.com/category/press-releases/).
- AMPOWER: [AMPOWER Report 2026 summary](https://additive-manufacturing-report.com/) reports EUR 11.33B industrial metal/polymer AM market in 2025 and 5.7% growth.
- ResearchAndMarkets via Business Wire: [AM software revenue forecast](https://www.businesswire.com/news/home/20250506950344/en/Additive-Manufacturing-Software-Markets-Report-2025-Analysis-Data-and-Forecast---Revenues-Expected-to-Hit-%246.78B-by-2033---ResearchAndMarkets.com) cites $2.44B current AM software revenue and $6.78B by 2033.
- 3YOURMIND: [distributed manufacturing software](https://www.3yourmind.com/distributed-manufacturing-software).
- AMFG: [additive MES guide](https://www.amfg.ai/whitepapers/additive-manufacturing-mes-software-the-essential-guide).
- Authentise: [workflow management for regulated AM operations](https://www.authentise.com/).
- Materialise: [Magics data and build preparation](https://www.materialise.com/en/industrial/software/magics-data-build-preparation).
- Dyndrite: [GPU-accelerated additive manufacturing engine](https://www.dyndrite.com/).
- Prusa: [PrusaSlicer positioning and profiles](https://www.prusa3d.com/p/prusaslicer/).
- UltiMaker: [Cura slicing engine and settings](https://ultimaker.com/software/ultimaker-cura/).
- SimplyPrint: [print-farm pricing and feature anchors](https://simplyprint.io/pricing).
- Printago: [e-commerce print-farm positioning](https://printago.io/).
- OctoPrint: [remote monitoring and plugin ecosystem](https://octoprint.org/) and [GCode Viewer docs](https://docs.octoprint.org/en/main/bundledplugins/gcodeviewer.html).
- Moonraker: [file-management/upload API](https://moonraker.readthedocs.io/en/latest/external_api/file_manager/).
- Klipper: [printer configuration reference](https://www.klipper3d.org/Config_Reference.html).

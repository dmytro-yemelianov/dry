# Marketing Intel: Dry

## Executive Positioning

Dry is currently strongest as a **toolpath compilation and validation layer** rather than a full slicing
product. The immediate commercial value is in teams that already care about machine motion quality and are
willing to replace brittle, implicit tooling with a typed, deterministic compiler path.

For the full external market scan, ICP scoring, competitive landscape and pricing hypotheses, see
[`market-research-deep-dive.md`](market-research-deep-dive.md).

## Who needs this

### Primary segments (user/problem-side)

1. **Print labs and service providers**
   - Need recurring, high-volume job review and repeatable quality gates across printers, materials and operators.
   - Daily friction: variable output quality, delayed issue detection, and hard-to-compare tooling outputs.

2. **Advanced makers and hardware R&D teams**
   - Need experimental or custom path strategies (non-planar patterns, variable width, custom extrusion logic).
   - Daily friction: custom scripts break across versions and are difficult to verify before machine output.

3. **Slicer/platform developers**
   - Need a composable engine to import, transform, verify, or emit motion instead of owning a full native G-code
     stack internally.
   - Daily friction: duplicated toolpath logic, inconsistent verification, and high cost of maintaining multi-language
     bindings.

4. **Machine OEMs, control integrators, and industrial software vendors**
   - Need process-aware toolpath pipelines for post-slicer analytics, optimization, compliance, and reportability.
   - Daily friction: fragmented QA, weak provenance, and weak cross-team interoperability for future target expansion
     (5-axis, CNC, non-FFF pathways).

## Who would pay for this

1. **Operations leaders at print service businesses**
   - Typical payer: operations director, technical lead, or facilities manager.
   - Value driver: fewer failed jobs, faster review cycles, and stronger process consistency.

2. **Product engineering teams in hardware or software companies**
   - Typical payer: VP Engineering, Principal/Staff Engineers, Tech Lead of CAM/production teams.
   - Value driver: reduce internal maintenance cost by centralizing motion compilation and verification logic.

3. **B2B SaaS / application vendors in manufacturing workflows**
   - Typical payer: CTO, Head of Product, or GTM owner for the platform.
   - Value driver: differentiate through deterministic output, audit-ready toolpath reports, and a clean API surface.

4. **Research labs and engineering consultancies**
   - Typical payer: program manager, project lead, or technical principal.
   - Value driver: faster prototyping with reproducible output and language-specific APIs for Python/TypeScript.

## Ideal Customer Profile (ICP)

### ICP A: Repeatable print-production teams

- **Profile:** print labs, small-to-mid production print farms, managed service providers.
- **Why they fit:** they already run many jobs with recurring quality incidents and need repeatable review/verification before print.
- **Decision trigger:** measurable manual review overload, failed-print cost, and willingness to standardize on profiles + quality gates.
- **Minimum ICP conditions:**
  - 20+ job reviews per week or equivalent QA volume.
  - At least one operator role spending more than 1 hour/day on repeatable G-code checks.
  - explicit process owner for print quality outcomes.

### ICP B: Motion-toolpath platform teams

- **Profile:** CAD/CAM, slicer plugin, or manufacturing SaaS teams that already own some motion-related backend.
- **Why they fit:** they need a shared compiler/verification surface and want to avoid duplicating gcode parser, analyzer, and emitter logic.
- **Decision trigger:** engineering cost pressure and pressure to improve determinism across integrations.
- **Minimum ICP conditions:**
  - at least one engineering team embedding toolpath output into a product.
  - recurring defects from inconsistent downstream gcode output.
  - willingness to define and maintain machine/material profiles.

### ICP exclusion (not first release target)

- Teams looking for full end-user slicer parity (mesh import, supports, infill, placement) now.
- Teams with low print volume and no repeatable QA workflows.
- Teams expecting closed-loop printer tuning (input shaper, pressure advance, adaptive control) in the first release scope.

### ICP fit score (starting point)

- Use this internal scoring for discovery:

  - 2 points if team has 20+ weekly jobs with operator review.
  - 2 points if jobs are failing after toolchain changes and they track this cost.
  - 2 points if there is an owner for profiles/gate policies.
  - 2 points if they can run CLI/SDK in CI or post-processing workflows.
  - 2 points if a single clear buyer can fund the initial pilot.

**ICP-qualified threshold:** 8+ points.

### Recommended first packages

1. **Deterministic Review + Audit**
   - For print farms, labs and service providers.
   - Core workflow: profile-backed `review-gcode`, `trace-gcode`, reports and optional re-verified rewrites.

2. **Embed SDK**
   - For slicer, CAD/CAM, print-management and manufacturing software teams.
   - Core workflow: deterministic parser/IR/report engine embedded through privately delivered CLI,
     Python, TypeScript or wasm artifacts.

## Pain points resolved

### 1) "Slicing is a black box"

- **Pain**: teams cannot reliably explain why output changed or where defects came from.
- **Dry response**: deterministic compiler pipeline (`design -> path -> motion -> target`), explicit IR, and trace/inspection
  commands.
- **Commercial implication**: makes support triage and audit conversations defensible.

### 2) "Quality breaks after updates"

- **Pain**: behavior drift after software version updates leads to re-tuning and rework.
- **Dry response**: unit-typed model, strict validity checks, profile schema, and conformance-oriented behavior.
- **Commercial implication**: easier release control and reduced surprise regressions in production pipelines.

### 3) "Review is too slow and manual"

- **Pain**: humans inspect large G-code artifacts after failures, causing delays and operator fatigue.
- **Dry response**: profile-aware `review`, `verify`, `trace`, and rewrite/optimization hooks with machine-readable reports.
- **Commercial implication**: lower cost-per-job and quicker release-to-print.

### 4) "We can't prove process quality to clients"

- **Pain**: limited evidence for why a job met or failed internal standards.
- **Dry response**: structured reports, consistent metrics, and explicit machine/material/process constraints.
- **Commercial implication**: stronger handoff documentation for quality systems and compliance-heavy environments.

### 5) "Legal/licensing risk blocks shipping"

- **Pain**: teams avoid risky dependencies despite performance needs.
- **Dry response**: proprietary clean-room implementation, explicit third-party notices, and clear
  provenance/oracle separation.
- **Commercial implication**: controlled embedding rights and a reviewable IP boundary for procurement.

## Market/Investigation notes

- **Ideal customer profile:** teams already spending on production tooling and already handling repetitive G-code
  review, not teams looking for their first slicer.
- **Near-term wedge:** post-slicer review + deterministic verification first, then API embedding and higher-order
  compiler workflows.
- **Go-to-market narrative:** sell outcomes (failure-rate reduction, review speed, reproducibility, auditability),
  not raw geometry features.
- **Positioning risk to watch:** users may expect turnkey slicing, material profiles and closed-loop machine calibration;
  current docs show those areas as partial or in-flight.
- **Commercial design cue:** packaging can be sold as 2-3 clear value tracks:
  (1) Quality gate/inspection service,
  (2) SDK embedding license, and
  (3) advanced workflow/consulting support for custom pipeline integration.

## Suggested next actions

1. Validate this segmentation with 8-12 discovery interviews and tag each lead as:
   - pain urgency,
   - current spend on QA/ops, and
   - time-to-successor integration.
2. Add 2-3 short customer proof pages:
   - "Print farm pain reduction,"
   - "SDK integration case,"
   - "Post-slicer review workflow."
3. Define pricing tests by workflow bundle (review, SDK, enterprise batch support) rather than by user count.

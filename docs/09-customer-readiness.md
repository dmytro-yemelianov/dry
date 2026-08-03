# Dry customer readiness matrix

This document maps Dry's current capabilities to likely user/customer needs. It is a go-to-market and
pilot guide, not a promise that every segment is production-ready today.

## Readiness scale

- **High**: usable now by technical users with normal validation.
- **Medium**: useful in pilots, but needs packaging, UX or workflow hardening.
- **Low**: architecturally supported, but not ready to sell or promise.

## Segment matrix

| Segment | Current fit | Why Dry helps | Missing before broad use |
|---|---:|---|---|
| Algorithmic toolpath researchers | High | Python/TS authoring, exact paths, arcs, channels, simulation, byte-stable emit. | Published examples, notebooks, packaging, clearer experimental flags. |
| Advanced makers and print labs | Medium-high | Review, verify, trace, rewrite and optimize G-code with profile-aware checks. | Easier profiles, installer, visual report, safer defaults and documentation. |
| Post-slicer QA / farm operators | Medium-high | Source-line findings, bounds/flow/temp checks, trace summaries, binary archives. | Batch workflow, report export, profile library, CI-style integration docs. |
| SDK integrators | Medium | Rust core plus Python/TS/wasm adapters and deterministic conformance. | Formal IR spec, semver policy, versioned release packages, integration examples. |
| CAD/workbench plugin users | Medium | Strong compiler layer from authored paths to verified target code. | Host integration, UI workflow, CAD geometry adapters, installer story. |
| Education and demos | Medium-high | Browser gallery, visual authoring, explainable IR pipeline, deterministic output. | Curated lessons, hosted demo, resettable examples, simpler terminology. |
| Product teams building a SaaS/desktop tool | Medium-low | Engine and wasm are strong enough to embed. | Auth/storage/project UX, release artifacts, support model, product analytics. |
| Industrial 5-axis / non-planar FFF | Low-medium | Tool orientation and rotary emit exist as a foundation. | Machine models, IK validation, fixtures, collision/singularity handling, real machine gates. |
| CNC, laser, robot customers | Low | Architecture anticipates multiple targets. | Target dialects, process models, machine profiles, conformance corpora, real machine testing. |
| General slicer users | Low | Dry can become a compiler layer under slicing. | Mesh import/repair, slicing, supports, infill, placement, cooling, broad compatibility. |

## Best first customer profiles

### 1. Researcher or advanced maker generating custom paths

Need:

- precise algorithmic paths;
- visible g-code;
- repeatable metrics;
- quick Python/TypeScript iteration;
- ability to inspect and verify output.

Current offer:

- Python or TypeScript SDK;
- CLI emit/simulate/verify;
- browser preview;
- conformance-backed output.

Pilot success metric:

- user replaces a hand-written or ad hoc script with Dry and produces validated g-code for one controlled
  printer profile.

### 2. Print lab reviewing slicer output

Need:

- detect risky G-code before printing;
- inspect source lines;
- compare or normalize output;
- summarize long jobs.

Current offer:

- `review-gcode`;
- `trace-gcode`;
- `rewrite-gcode`;
- profile-aware bounds, flow, speed and temperature checks.

Pilot success metric:

- Dry catches actionable issues or produces reports that reduce manual review time on a known corpus of
  sliced jobs.

### 3. SDK integrator

Need:

- stable API;
- deterministic output;
- documented data model;
- bindings in their stack.

Current offer:

- Rust core;
- Python binding;
- TypeScript SDK over wasm;
- binary and JSON IR round trips.

Pilot success metric:

- integrator embeds Dry in one workflow and reproduces a conformance fixture without manual patching.

### 4. CAD-connected toolpath workbench

Need:

- convert sketches/curves/features into machine motion;
- preserve process metadata;
- preview and verify before emission.

Current offer:

- suitable core architecture and L1/L2 lowering;
- enough SDK surface to prototype host integrations.

Pilot success metric:

- one host can export a path into Dry, verify it, preview it, and emit target g-code.

## Product packages to grow toward

### Dry CLI

Audience: print labs, CI pipelines, power users.

Core jobs:

- inspect IR;
- import/review/rewrite G-code;
- verify against profiles;
- emit target G-code;
- pack/unpack binary archives.

Production gaps:

- verified v0.4 public GitHub Release artifacts;
- profile templates;
- machine-readable report docs;
- shell recipes;
- exit-code policy.

### Dry SDKs

Audience: researchers, integrators, CAD/plugin authors.

Core jobs:

- author L1 designs;
- resolve to L2;
- simulate/verify/emit;
- keep output consistent across Python, TypeScript and Rust.

Production gaps:

- versioned public GitHub Release delivery;
- API stability policy;
- Rust authoring SDK;
- generated API docs;
- integration examples.

### Dry Workbench

Audience: technical makers, educators, early product users.

Core jobs:

- choose or author a design;
- preview motion;
- inspect metrics and verification;
- export g-code/IR.

Production gaps:

- project persistence;
- profile editor;
- visual report pages;
- hardened public hosted deployment;
- clearer separation of stable and experimental controls.

### Dry Review Service

Audience: print farms and teams with repeatable G-code review needs.

Core jobs:

- batch review;
- trace summaries;
- finding reports;
- policy gates before print release.

Production gaps:

- batch runner;
- report schema;
- dashboard/API;
- profile fleet management;
- audit logs.

## Pilot design

Every pilot should define:

1. **Workflow**: generation, review, verification, rewrite, or integration.
2. **Inputs**: source designs, G-code, profiles and expected outputs.
3. **Supported scope**: firmware flavor, printer/material, features allowed.
4. **Acceptance**: output parity, findings quality, runtime, memory, user time saved.
5. **Fallback**: what happens if Dry rejects or cannot represent a job.

Good pilot artifacts:

- fixed corpus of 10 to 50 jobs or designs;
- one or two machine profiles;
- baseline manual review notes;
- target report format;
- post-pilot issue taxonomy.

## Readiness gates by segment

### Researchers and advanced makers

Gate:

- Python/TS install path works;
- examples cover arcs, splines, channels, verification and emit;
- docs explain how to validate output on a real machine.

### Print labs and post-slicer reviewers

Gate:

- profile schema reference exists; ✅ [`docs/11-profiles-and-reports.md`](11-profiles-and-reports.md) + [`spec/dry-profile-v1.schema.json`](../spec/dry-profile-v1.schema.json) + example profiles
- batch CLI is documented;
- JSON report schema is stable; ✅ [`spec/dry-reports-v1.schema.json`](../spec/dry-reports-v1.schema.json) (verify/review/trace/forensics/rewrite/explain/compare), drift-gated + independently validated
- trace output has examples; ✅ [`conformance/reports/`](../conformance/reports)
- rewrite limitations are explicit.

### Integrators

Gate:

- Dry IR schema and semver policy are published; ✅ [`docs/10-dry-ir-v0-spec.md`](10-dry-ir-v0-spec.md) + [`spec/dry-ir-v0.schema.json`](../spec/dry-ir-v0.schema.json)
- package releases are tagged;
- conformance vectors are public; ✅ [`conformance/vectors/`](../conformance/vectors) (independently validated, no `dry-core`)
- bindings expose typed errors consistently.

### CAD/workbench users

Gate:

- one host integration exists;
- geometry ownership is clear;
- profile and export UX is understandable;
- the workbench can save and reload projects.

### Industrial and multi-target users

Gate:

- machine models are target-specific;
- target emitters have conformance corpora;
- real-machine validation exists;
- collision/singularity/safety limits are documented.

## Messaging

Use this positioning now:

> Dry is a deterministic, typed toolpath compiler foundation for algorithmic and inspected machine
> motion. It is strongest today for FFF-centered generation, review, verification, optimization and
> research workflows.

Avoid this positioning for now:

> Dry is a complete slicer, certified industrial CAM system, or turnkey multi-axis manufacturing suite.

## Immediate customer-readiness tasks

1. Publish a known-limitations page. ✅ [`docs/14-known-limitations.md`](14-known-limitations.md)
2. Create installable CLI/Python/TS release artifacts.
3. Write profile schema docs with 3 to 5 real examples.
4. Publish a JSON report schema for verification and trace outputs.
5. Add large-file benchmarks and document streaming guarantees.
6. Create 3 pilot guides: custom path authoring, post-slicer review, SDK integration. ✅ [`docs/pilots/`](pilots/) + [CLI cookbook](15-cli-cookbook.md) + [`examples/`](../examples)
7. Turn pilot feedback into stable rule IDs, examples and release blockers.

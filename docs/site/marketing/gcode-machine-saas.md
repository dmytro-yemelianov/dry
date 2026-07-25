---
title: G-code Machine SaaS
pageClass: marketing-page
---

<section class="market-hero">
  <p class="market-eyebrow">SaaS architecture</p>
  <h1>G-code Machine SaaS</h1>
  <p class="market-lede">
    A hosted review, registry and proof service for printers, CNC routers, mills, lasers, plasma tables
    and other machines whose production artifact is G-code.
  </p>
  <div class="market-actions">
    <a href="#product-thesis">Thesis</a>
    <a href="#machine-scope">Machines</a>
    <a href="#service-shape">Service</a>
    <a href="#implementation-plan">Plan</a>
  </div>
</section>

## Product Thesis

Dry should turn the printer capability library into a broader machine control plane:

<div class="market-flow" aria-label="Dry G-code machine SaaS pipeline">
  <div>G-code, configs, CAM/slicer profiles and sender state</div>
  <span>-></span>
  <div>Dry adapters, capability packs, review reports and proofs</div>
  <span>-></span>
  <div>registry, API, CI gates, edge agents and dashboards</div>
</div>

The "honeypot" is a product-led data flywheel: a free analyzer attracts real files, profiles, controller
fingerprints and error cases. Paid users get private storage, policy gates, proof retention and machine-drift
checks.

## Machine Scope

<div class="market-grid">
  <article>
    <p class="card-label">First vertical</p>
    <h3>FFF printers</h3>
    <p>Preflight, upload gates, build-volume checks, slicer profile import, macro proofing and live Moonraker/OctoPrint drift checks.</p>
  </article>
  <article>
    <p class="card-label">Second vertical</p>
    <h3>CNC routers and mills</h3>
    <p>CAM post review, work-envelope checks, modal-state hazards, tool table assumptions, spindle/feed sanity and safe-Z policy.</p>
  </article>
  <article>
    <p class="card-label">Adjacent</p>
    <h3>Lasers, plasma and dispensers</h3>
    <p>Power/speed sanity, pierce/cut sequencing, unsupported dialect detection, dwell risks and bounds checks.</p>
  </article>
</div>

The common abstraction is:

```text
machine envelope + controller dialect + modal state + tools + process limits + artifact policy
```

## Service Shape

### Public Analyzer

The top-of-funnel product accepts `.gcode`, `.nc`, `.tap` and related files, then returns:

- bounds and envelope findings;
- unsupported-command and dialect findings;
- modal-state hazards such as missing units or distance mode;
- feed, speed, spindle, heater, laser or tool-state warnings;
- downloadable reports;
- an optional contribution path for anonymized findings.

Raw files should be stored only with explicit consent. For paid tenants, raw artifact retention belongs to the
tenant policy.

### Machine Capability Registry

Printer packs become one subtype of a generic machine pack:

```text
dry-machine-pack/
  manifest.json
  machine.json
  controller/
    grbl-settings.txt
    linuxcnc.ini
    klipper.cfg
    moonraker.objects.json
  tools/
    endmills.json
    nozzles.json
    laser-heads.json
  processes/
    pla-0.4.json
    aluminum-router-6mm.json
  checks/
    bounds.json
    modal-state.json
    tool-change.json
    spindle-feed.json
    safe-z.json
  samples/
    first-layer.gcode
    pocketing.nc
  proofs/
    manifest.proof.json
    sample.review.json
```

### Edge Agent

Machines are usually behind LANs, firewalls or USB serial. The SaaS should use an outbound-only local agent that:

- reads controller and sender state;
- imports configs and machine properties;
- performs local review for private files;
- uploads reports and proof summaries;
- optionally blocks upload/start through local policy;
- does not initiate motion by itself in the MVP.

First adapters: Moonraker, OctoPrint, LinuxCNC config/status import, Grbl read-only serial settings and CNCjs-style
CNC sender integration where available.

### Developer API

```http
POST /v1/artifacts/review
POST /v1/artifacts/trace
POST /v1/packs/import
GET  /v1/machines
GET  /v1/machines/{id}/capabilities
POST /v1/machines/{id}/proofs
GET  /v1/reports/{id}
```

```bash
curl -X POST https://api.dry.dev/v1/artifacts/review \
  -H "Authorization: Bearer $DRY_TOKEN" \
  -F artifact=@part.nc \
  -F machine=shapeoko-xxl-grbl \
  -F process=aluminum-router-6mm
```

## CLI Surface

Generalize `dry printer` into `dry machine`, while keeping `dry printer` as a friendly alias.

```bash
dry machine search shapeoko
dry machine inspect shapeoko-xxl-grbl
dry machine import grbl --port /dev/tty.usbserial -o ./packs/shapeoko/
dry machine import linuxcnc ./linuxcnc-config/ -o ./packs/router/
dry machine import moonraker http://printer.local -o ./packs/voron/
dry machine review part.nc --machine shapeoko-xxl-grbl --process aluminum-router-6mm
dry machine prove shapeoko-xxl-grbl --sample pocketing.nc
dry machine diff shop-router-old shop-router-new
```

## Checks

Cross-machine checks:

- missing or conflicting units;
- absolute/relative distance-mode hazards;
- unsupported command or dialect;
- motion outside envelope;
- feedrate above machine/process/tool limits;
- unsafe rapid moves before homing or safe-Z;
- arc and plane assumptions;
- missing tool, spindle, laser, heater or extrusion state;
- incompatible CAM/slicer/post fingerprints.

Additional CNC checks include work coordinate systems, tool length offsets, spindle speed vs feed per tooth,
coolant commands, canned cycles, probing support and safe-Z relative to stock/fixture data when present.

## Data Model

| Entity | Role |
|---|---|
| `Machine` | physical or virtual machine target |
| `MachinePack` | versioned capability package |
| `ControllerAdapter` | importer/query bridge for firmware or sender |
| `ProcessProfile` | material/tool/process-specific runtime constraints |
| `Artifact` | uploaded or referenced G-code-like file |
| `ReviewReport` | deterministic findings and metrics |
| `ProofRun` | reproducible pack/sample validation |
| `Observation` | timestamped live machine/controller state |
| `Policy` | upload/start/release gate rules |
| `Agent` | local outbound connector |

## Safety Boundary

Dry should sell review, proof and policy first. Remote actuation is a later, local-agent-only capability.

Do not claim CNC/laser/plasma collision safety unless stock, fixture, tool, workholding and setup data are present.
Do not run calibration samples without local human confirmation. Rewrites must be reviewed again before export.

## Packaging

<div class="market-grid">
  <article>
    <p class="card-label">Free</p>
    <h3>Public analyzer</h3>
    <p>Small file limits, public packs and optional anonymized contribution. Built to collect corpus and reveal adapter demand.</p>
  </article>
  <article>
    <p class="card-label">Pro</p>
    <h3>Private review</h3>
    <p>Saved machines, private artifacts, CLI/API tokens, report history and local registry sync.</p>
  </article>
  <article>
    <p class="card-label">Team</p>
    <h3>Fleet governance</h3>
    <p>Organization registry, edge agent, drift checks, CI/upload gates, approvals, webhooks and proof retention.</p>
  </article>
  <article>
    <p class="card-label">Enterprise / OEM</p>
    <h3>Embedded control plane</h3>
    <p>Private registry, custom adapters, signed packs, vendor-maintained libraries, support SLA and embedded SDK/API license.</p>
  </article>
</div>

## Implementation Plan

| Phase | Deliverables | Exit |
|---|---|---|
| 0. Generic schema | `dry-machine-pack-v1`, printer compatibility mapping, Klipper + Grbl example packs | `dry machine validate/inspect/review` works locally |
| 1. Public analyzer | upload UI, wasm/Rust review, machine selector, report download, consent controls | users get useful reports from `.gcode`, `.nc`, `.tap` |
| 2. Registry/proofs | pack registry, trust levels, proof runner, sample corpus, Git-backed packs | packs can be installed, pinned and proven in CI |
| 3. Edge agent | outbound local agent, Moonraker/OctoPrint, Grbl read-only import, LinuxCNC import | team compares live state to registered pack before accepting a job |
| 4. Integrations | sender plugins, GitHub Action, CAD/CAM post-checks, MES/fleet webhooks | Dry sits before upload/start in customer workflows |

## Source Notes

- [Moonraker HTTP and JSON-RPC API](https://moonraker.readthedocs.io/en/latest/external_api/introduction/)
- [Moonraker file API](https://moonraker.readthedocs.io/en/latest/external_api/file_manager/)
- [OctoPrint REST API](https://docs.octoprint.org/en/main/api/index.html)
- [LinuxCNC G-code overview](https://linuxcnc.org/docs/html/gcode/overview.html)
- [LinuxCNC G-code reference](https://linuxcnc.org/docs/html/gcode/g-code.html)
- [Grbl command documentation](https://github.com/gnea/grbl/blob/master/doc/markdown/commands.md)
- [CNCjs documentation](https://cnc.js.org/docs/)
- [Universal Gcode Sender](https://winder.github.io/ugs_website/)

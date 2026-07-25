# G-code Machine SaaS and Honeypot Plan

Research/planning date: 2026-07-02

## Thesis

Dry should not limit the capability-library idea to 3D printers. The stronger product is a **G-code machine
control plane**: a SaaS registry, analyzer and proof service for any machine whose production artifact is
G-code or a close dialect.

The "honeypot" should be interpreted as a consent-based product data flywheel inside controlled
evaluations, not a deceptive trap:

- a low-friction private evaluation that attracts representative files, profiles and error cases;
- consented upload of anonymized findings, machine fingerprints and adapter gaps;
- private registries, policy gates, fleet agents and audit reports for teams with real production risk.

```text
G-code / controller config / CAM or slicer profile / sender state
  -> Dry machine adapters
  -> capability pack + review report + proof run
  -> registry, API, CI gate, edge agent, dashboard
  -> better machine packs and stronger checks over time
```

## Machine Scope

Start with machines where Dry can add value without becoming the primary real-time controller.

| Machine class | First value | Capability additions beyond printers |
|---|---|---|
| FFF printers | preflight, upload gate, profile proof | build volume, heaters, filament, macros, slicer profile |
| CNC routers/mills | CAM post check, work envelope, modal-state review | spindle, tool table, work coordinate systems, fixture clearance |
| Lasers | power/speed sanity, bounds, unsupported dialect detection | power mode, air assist, material presets, scan/cut modes |
| Plasma/waterjet | pierce/cut sequencing, bounds, feed/power checks | pierce height/time, torch state, kerf/process tables |
| Plotters/dispensers | motion/retraction/flow review | pen/tool state, extrusion or dispense channels |

The unifying abstraction is not "printer". It is:

```text
machine envelope + controller dialect + modal state + tools + process limits + artifact policy
```

## Why SaaS

A local CLI proves the engine. SaaS creates the commercial surface:

- **central registry:** teams need one maintained source of machine properties, not scattered slicer/CAM configs;
- **policy gates:** production files need approval, not just viewing;
- **fleet drift detection:** live machine/controller state diverges from stored profiles;
- **proof retention:** customers need evidence that a file was checked before release;
- **data moat:** anonymized issue patterns reveal which controllers, post processors and slicers to support next;
- **integrator API:** CAD/CAM, slicer, print-farm and CNC-sender vendors need a hosted service they can call.

## Product Components

### 1. Private Evaluation Analyzer

The top-of-funnel product:

- upload or paste G-code in an authenticated evaluation workspace;
- choose or infer a machine/controller profile;
- get bounds, modal-state, dialect, tool, feed/speed and safety findings;
- download a report;
- optionally contribute anonymized fingerprints and findings to Dry's corpus.

This is the consented discovery surface. It should collect:

- controller dialect hints;
- unsupported command patterns;
- slicer/CAM/post fingerprints;
- anonymized modal-state sequences;
- machine-envelope failures;
- adapter import failures;
- user-supplied labels such as "worked", "failed", "near miss", "wrong machine".

Do not retain raw customer files by default. Store raw artifacts only when a customer explicitly opts in,
and support redaction, tenant isolation, retention controls and deletion requests.

### 2. Machine Capability Registry

Generalize the printer registry into a machine registry:

```text
dry-machine-pack/
  manifest.json
  machine.json
  controller/
    grbl-settings.txt
    linuxcnc.ini
    klipper.cfg
    moonraker.objects.json
  senders/
    octoprint.json
    cncjs.json
  tools/
    endmills.json
    nozzles.json
    laser-heads.json
  processes/
    pla-0.4.json
    aluminum-6mm-router.json
    acrylic-laser-cut.json
  checks/
    bounds.json
    modal-state.json
    tool-change.json
    spindle-feed.json
    safe-z.json
  samples/
    first-layer.gcode
    pocketing.nc
    laser-test.gcode
  proofs/
    manifest.proof.json
    sample.review.json
  provenance.json
```

Printer packs become one subtype of machine pack.

### 3. Edge Agent

Most machines live on private LANs or USB serial links. The SaaS should not require inbound network access.

Run a small local agent that:

- reads live controller/sender state;
- imports configs and machine properties;
- performs local review for private files;
- uploads metadata, reports and proof summaries;
- optionally gates upload/start commands through local policy;
- never starts a job unless the local operator or existing sender explicitly allows it.

First adapters:

- Moonraker for Klipper printers;
- OctoPrint for printer profiles/files/jobs;
- CNCjs for Grbl/Smoothieware/TinyG-class CNC senders where available;
- LinuxCNC config/status importer;
- Grbl serial setting importer in read-only mode.

### 4. Team Dashboard

Paid teams need:

- machine inventory;
- pack trust level and drift status;
- artifact review history;
- failed-check trends;
- per-machine policy;
- proof reports for audits;
- private pack registry;
- API tokens and webhooks.

### 5. Developer API

The API should expose review and registry operations, not raw control first.

```http
POST /v1/artifacts/review
POST /v1/artifacts/trace
POST /v1/packs/import
GET  /v1/machines
GET  /v1/machines/{id}/capabilities
POST /v1/machines/{id}/proofs
GET  /v1/reports/{id}
POST /v1/webhooks
```

Example:

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

Printer-specific convenience remains:

```bash
dry printer import klipper printer.cfg -o ./packs/voron/
dry printer resolve voron-2.4-350-klipper --material ABS --nozzle 0.4
```

## API Model

```ts
type MachineKind =
  | "fff-printer"
  | "cnc-router"
  | "cnc-mill"
  | "laser"
  | "plasma"
  | "waterjet"
  | "plotter"
  | "dispenser";

interface MachineCapabilities {
  identity: {
    id: string;
    kind: MachineKind;
    vendor?: string;
    model?: string;
  };
  controller: {
    dialect: "marlin" | "klipper" | "reprap" | "grbl" | "linuxcnc" | "smoothieware" | "tinyg" | "unknown";
    version?: string;
    supportedCommands?: string[];
  };
  envelope: {
    axes: string[];
    limitsMm?: Record<string, [number, number]>;
    softLimits?: boolean;
    homingRequired?: boolean;
  };
  modalPolicy: {
    unitsRequired?: "G20" | "G21";
    distanceModeRequired?: "G90" | "G91";
    planeRequired?: "G17" | "G18" | "G19";
    allowedWorkOffsets?: string[];
  };
  tools: ToolCapability[];
  processes: ProcessCapability[];
  checks: CheckSummary[];
}
```

## Checks That Transfer Across G-code Machines

Cross-machine checks:

- missing or conflicting units (`G20`/`G21`);
- absolute/relative distance-mode hazards (`G90`/`G91`);
- unsupported command/dialect detection;
- motion outside declared envelope;
- feedrate above machine/process/tool limits;
- unsafe rapid moves before homing or safe-Z;
- arc mode and plane assumptions (`G2`/`G3`, `G17`/`G18`/`G19`);
- tool/spindle/laser state changes missing before cutting/extrusion;
- unexpected pauses, dwell, program stop or tool-change behavior;
- comments or metadata that identify incompatible post processors.

CNC-specific checks:

- work coordinate system mismatch (`G54`-`G59`);
- tool length offset and tool table mismatch;
- spindle speed vs feed per tooth sanity;
- coolant command compatibility;
- canned cycle support;
- probe cycle support;
- safe-Z relative to stock/fixture model when provided.

Laser/plasma-specific checks:

- power mode and PWM range;
- air assist or torch enable sequencing;
- pierce delay and cut-height parameters;
- excessive dwell in one location;
- scan/cut mode confusion.

## SaaS Data Model

Core entities:

- `Organization`;
- `User`;
- `Machine`;
- `MachinePack`;
- `ControllerAdapter`;
- `ProcessProfile`;
- `Artifact`;
- `ReviewReport`;
- `ProofRun`;
- `Observation`;
- `Finding`;
- `Policy`;
- `Webhook`;
- `Agent`.

Data retention defaults:

| Data | Free analyzer | Paid SaaS |
|---|---|---|
| Raw G-code | opt-in only, short retention | tenant-controlled |
| Normalized metrics | stored | stored |
| Findings | stored | stored |
| Machine fingerprints | stored anonymized | tenant-owned |
| Proof reports | public/private by user | retained for audit |
| Live machine state | not supported | metadata only unless enabled |

## Safety Boundary

The service should be conservative:

- default product is review, proof and policy, not remote control;
- edge agent can block upload/start, but should not initiate motion by itself in MVP;
- raw files are private by default for paid tenants;
- evaluation analyzer stores raw uploads only with explicit consent;
- all generated rewrites must be re-reviewed before export;
- calibration samples require local human confirmation before running;
- CNC/laser/plasma checks must avoid claiming physical collision safety unless stock, fixture, tool and setup data are present.

## Packaging and Pricing

### Evaluation

- time-limited private analyzer;
- agreed file and report limits;
- curated evaluation machine packs;
- explicit opt-in for contributing anonymized findings.

Purpose: prove value, create trust and reveal high-demand adapters without publicly distributing the
product.

### Pro

- private artifacts;
- saved machines;
- CLI/API tokens;
- local registry sync;
- report history.

Likely buyers: advanced makers, consultants, small labs.

### Team

- organization registry;
- edge agent;
- machine drift checks;
- CI/upload gates;
- role-based approvals;
- webhooks;
- proof retention.

Likely buyers: print farms, fab labs, university labs, CNC job shops and product teams.

### Enterprise / OEM

- private/on-prem registry;
- custom adapters;
- signed packs;
- vendor-maintained machine libraries;
- support SLA;
- embedded SDK/API license.

Likely buyers: machine OEMs, slicer/CAM vendors, manufacturing platforms, regulated internal labs.

## Implementation Phases

### Phase 0: Rename the Abstraction Internally

Keep existing printer terminology in UX where helpful, but define the core schema as `dry-machine-pack-v1`.

Deliverables:

- generic machine manifest schema;
- printer pack compatibility mapping;
- two hand-authored packs: one Klipper printer, one Grbl CNC router;
- `dry machine validate`, `inspect`, `review`.

### Phase 1: Public Analyzer MVP

Deliverables:

- web upload page;
- local wasm/Rust review execution;
- machine selector;
- downloadable report;
- anonymous findings store;
- clear consent controls.

Exit:

- users can upload `.gcode`, `.nc`, `.tap` and receive a useful report;
- unsupported command patterns are summarized for roadmap planning.

### Phase 2: Registry and Proofs

Deliverables:

- machine-pack registry;
- pack trust levels;
- proof runner;
- sample corpus;
- Git-backed community packs.

Exit:

- a pack can be installed, pinned, reviewed and proven in CI.

### Phase 3: Edge Agent

Deliverables:

- local agent with outbound-only SaaS connection;
- Moonraker and OctoPrint adapters;
- Grbl read-only serial importer;
- LinuxCNC config importer;
- policy gate mode.

Exit:

- team can compare live machine state to the registered pack before accepting a job.

### Phase 4: Integrations

Deliverables:

- OctoPrint/Moonraker plugin;
- CNCjs sender integration or sidecar;
- GitHub Action;
- Fusion/Onshape/CAM post-check integrations;
- webhooks for MES/fleet systems.

Exit:

- Dry sits naturally before upload/start in real customer workflows.

## Near-Term Technical Decisions

1. Make `MachineCapabilities` generic and implement `PrinterCapabilities` as a specialization.
2. Add a `dialect` layer for parser/emitter support instead of assuming FFF semantics.
3. Treat sender/controller state as observations with timestamps, not canonical truth.
4. Separate consented evaluation-corpus ingestion from customer artifact storage.
5. Build the first SaaS path as "review only"; add control/gating through a local agent later.

## Source Notes

- Moonraker exposes APIs for Klipper over HTTP and JSON-RPC: https://moonraker.readthedocs.io/en/latest/external_api/introduction/
- Moonraker file operations include G-code file roots and upload/download APIs: https://moonraker.readthedocs.io/en/latest/external_api/file_manager/
- OctoPrint publishes a REST API including files, jobs, printer profiles and printer operations: https://docs.octoprint.org/en/main/api/index.html
- LinuxCNC documents RS274/NGC-derived G-code and modal command behavior: https://linuxcnc.org/docs/html/gcode/overview.html
- LinuxCNC G-code reference covers core motion, arc and canned-cycle commands: https://linuxcnc.org/docs/html/gcode/g-code.html
- Grbl documents settings, parser state, startup blocks and check mode commands: https://github.com/gnea/grbl/blob/master/doc/markdown/commands.md
- CNCjs targets Grbl, Smoothieware and TinyG-class CNC controllers: https://cnc.js.org/docs/
- Universal Gcode Sender targets Grbl, TinyG, g2core and Smoothieware controllers: https://winder.github.io/ugs_website/

# Printer Capability Library Plan

Research/planning date: 2026-07-02

## Product Thesis

Dry should grow a **Printer Capability Library**: a unified API and package format for printer properties,
firmware/slicer mappings, macros, checks, calibration routines, sample jobs and proof artifacts.

This should not replace firmware configuration, slicer profiles or fleet dashboards. It should normalize them
into a versioned, testable layer that Dry can use for review, verification, upload gates, CAD connectors and
SDK embedding.

This is the printer-specific vertical of the broader G-code machine SaaS plan in
`docs/marketing/gcode-machine-saas-honeypot.md`. The underlying abstraction should be generic enough for
CNC, laser, plasma, plotter and dispenser machines, while this plan keeps the first implementation focused on
printer profiles, firmware, slicers and macros.

```text
Klipper printer.cfg / Moonraker API / OctoPrint profile / Cura def.json / Prusa bundle
  -> Dry adapters
  -> Dry printer capability pack
  -> CLI + SDK + registry API
  -> profile, checks, macros, samples, proofs
```

## Existing Dry Foundation

Dry already has a small validated `Profile` model:

- firmware flavor;
- machine build volume, feedrate range and kinematics;
- material filament diameter, max volumetric flow and temperature limits;
- process defaults and checks;
- optional start/end G-code;
- mapping to verifier contracts and G-code import/emit defaults;
- Klipper `printer.cfg` import via `import-printer-cfg`.

The capability library should extend this model rather than fork it. The existing `dry-profile-v1` becomes the
runtime profile emitted by a larger printer pack.

## Core Concepts

### Capability Pack

A versioned directory or archive describing one printer class, printer instance or fleet policy.

```text
dry-printer-pack/
  manifest.json
  machine.json
  firmware/
    klipper.cfg
    moonraker.objects.json
  slicers/
    prusa.ini
    cura.def.json
    orca.json
  profiles/
    abs-0.4.json
    pla-0.4.json
  macros/
    start_print.klipper.cfg
    end_print.klipper.cfg
    flow_test.klipper.cfg
  checks/
    bounds.json
    max-flow.json
    first-layer.json
    retraction.json
  samples/
    flow-test.gcode
    pressure-advance.gcode
    first-layer.gcode
  proofs/
    manifest.proof.json
    flow-test.review.json
    pressure-advance.trace.json
  provenance.json
```

### Runtime Profile

A `dry-profile-v1` JSON document produced from a pack for a specific material/process/nozzle combination.

Example:

```bash
dry printer resolve voron-2.4-350 --material ABS --nozzle 0.4 -o voron-abs-0.4.profile.json
```

### Check

A machine-readable policy or calibration assertion that Dry can run locally:

- "review this sample G-code and expect no errors";
- "max volumetric flow must be present";
- "first-layer speed must stay in this range";
- "macro `START_PRINT` must accept `BED_TEMP` and `EXTRUDER_TEMP` params";
- "Moonraker reported `max_accel` must match pack kinematics within tolerance".

### Proof

A reproducible artifact proving that a pack was validated against Dry and, where possible, imported sources.

Proof types:

- **schema proof:** pack files validate against schemas;
- **adapter proof:** imported Klipper/Cura/Prusa/OctoPrint source maps to expected Dry profile fields;
- **sample proof:** sample G-code review/trace/verify matches expected reports;
- **macro proof:** macro syntax and declared params match the manifest;
- **hardware-observed proof:** optional report from live Moonraker/OctoPrint query;
- **human sign-off:** explicit manual validation note when hardware behavior cannot be inferred.

## Pack Manifest

Draft `manifest.json`:

```json
{
  "schema": "dry-printer-pack-v1",
  "id": "voron-2.4-350-klipper",
  "name": "Voron 2.4 350 Klipper",
  "version": "0.1.0",
  "kind": "printer-class",
  "license": "CC-BY-4.0",
  "maintainers": [
    { "name": "Example Lab", "url": "https://example.com" }
  ],
  "firmware": {
    "flavor": "klipper",
    "sources": ["firmware/klipper.cfg"]
  },
  "slicers": {
    "orca": ["slicers/orca.json"],
    "prusa": ["slicers/prusa.ini"],
    "cura": ["slicers/cura.def.json"]
  },
  "profiles": {
    "abs-0.4": "profiles/abs-0.4.json",
    "pla-0.4": "profiles/pla-0.4.json"
  },
  "macros": ["macros/start_print.klipper.cfg", "macros/end_print.klipper.cfg"],
  "checks": ["checks/bounds.json", "checks/max-flow.json"],
  "samples": ["samples/flow-test.gcode"],
  "proofs": ["proofs/manifest.proof.json"],
  "support": {
    "status": "experimental",
    "firmware_versions": ["klipper >= 0.12"],
    "notes": "No input-shaper or pressure-advance model in Dry profile v1."
  }
}
```

## Library API

### TypeScript SDK

```ts
import { PrinterRegistry } from "@dry/sdk/printers";

const registry = await PrinterRegistry.open({
  sources: ["./printer-packs", "https://registry.dry.dev"],
});

const printer = await registry.get("voron-2.4-350-klipper");

const caps = await printer.capabilities();
console.log(caps.machine.buildVolume);
console.log(caps.machine.kinematics);

const profile = await printer.resolveProfile({
  material: "ABS",
  nozzleDiameter: 0.4,
});

const report = await printer.reviewGcode("part.gcode", {
  profile,
});

const proof = await printer.runProofs();
if (!proof.ok) throw new Error(proof.summary);
```

### Python SDK

```python
from dry.printers import PrinterRegistry

registry = PrinterRegistry.open(
    sources=["./printer-packs", "https://registry.dry.dev"]
)

printer = registry.get("voron-2.4-350-klipper")
caps = printer.capabilities()

profile = printer.resolve_profile(material="ABS", nozzle_diameter=0.4)
report = printer.review_gcode("part.gcode", profile=profile)
proof = printer.run_proofs()
```

### Rust API

```rust
use dry_core::printers::{PrinterRegistry, ResolveProfile};

let registry = PrinterRegistry::open(["./printer-packs"])?;
let printer = registry.get("voron-2.4-350-klipper")?;

let profile = printer.resolve_profile(ResolveProfile {
    material: Some("ABS".into()),
    nozzle_diameter_mm: Some(0.4),
    ..Default::default()
})?;

let proof = printer.run_proofs()?;
```

## CLI Surface

Use a `dry printer ...` namespace.

### Registry and pack management

```bash
dry printer list --source ./printer-packs
dry printer search voron --source https://registry.dry.dev
dry printer inspect voron-2.4-350-klipper
dry printer validate ./packs/voron-2.4-350-klipper
dry printer pack ./packs/voron-2.4-350-klipper -o voron-2.4-350-klipper.drypack
dry printer unpack voron-2.4-350-klipper.drypack -o ./packs/
```

### Import/adapters

```bash
dry printer import klipper printer.cfg --id voron-2.4-350-klipper -o ./packs/voron/
dry printer import moonraker http://printer.local --id voron-live -o ./packs/voron-live/
dry printer import octoprint http://octopi.local --id mk3-octoprint -o ./packs/mk3/
dry printer import cura fdmprinter.def.json --id custom-cura -o ./packs/custom/
dry printer import prusa bundle.ini --id prusa-bundle -o ./packs/prusa/
```

### Profile resolution

```bash
dry printer resolve voron-2.4-350-klipper --material ABS --nozzle 0.4 -o profile.json
dry printer diff old-pack/ new-pack/
dry printer explain voron-2.4-350-klipper
```

### Checks and proofs

```bash
dry printer checks voron-2.4-350-klipper
dry printer run-check voron-2.4-350-klipper max-flow
dry printer run-sample voron-2.4-350-klipper flow-test
dry printer prove voron-2.4-350-klipper --update
dry printer prove voron-2.4-350-klipper --ci
```

### Integration with existing commands

```bash
dry review-gcode part.gcode --printer voron-2.4-350-klipper --material ABS --nozzle 0.4
dry upload part.gcode --printer voron-2.4-350-klipper --moonraker http://printer.local
dry compare baseline.gcode candidate.gcode --printer voron-2.4-350-klipper
```

`--printer` should resolve to a profile and pass the resulting `dry-profile-v1` into existing review/verify/upload
flows. The existing `--profile` flag remains the low-level explicit option.

## API Model

### Key interfaces

```ts
type RegistrySource =
  | { type: "local"; path: string }
  | { type: "git"; url: string; ref?: string }
  | { type: "http"; url: string };

interface PrinterRegistry {
  list(query?: PrinterQuery): Promise<PrinterSummary[]>;
  get(id: string, options?: { version?: string }): Promise<PrinterPack>;
  validate(pack: PrinterPack): Promise<ValidationReport>;
}

interface PrinterPack {
  id: string;
  manifest(): Promise<PrinterManifest>;
  capabilities(): Promise<PrinterCapabilities>;
  resolveProfile(input: ResolveProfileInput): Promise<DryProfile>;
  checks(): Promise<CheckSummary[]>;
  macros(): Promise<MacroSummary[]>;
  samples(): Promise<SampleSummary[]>;
  runProofs(options?: ProofOptions): Promise<ProofReport>;
}
```

### Capability model

```ts
interface PrinterCapabilities {
  identity: {
    id: string;
    name: string;
    vendor?: string;
    model?: string;
    variant?: string;
  };
  firmware: {
    flavor: "klipper" | "marlin" | "duet" | "unknown";
    version?: string;
    supportedCommands?: string[];
  };
  machine: {
    buildVolume?: [[number, number], [number, number], [number, number]];
    bedShape?: "rect" | "circle" | "custom";
    kinematics?: {
      kind?: "cartesian" | "corexy" | "delta" | "rotary" | "robot" | "unknown";
      maxAccelerationMmS2?: number;
      maxJunctionVelocityMmS?: number;
    };
  };
  toolheads: ToolheadCapability[];
  materials: MaterialCapability[];
  processes: ProcessCapability[];
}
```

## Registry Model

### Local registry

Directory of packs:

```text
~/.dry/printers/
  index.json
  packs/
    voron-2.4-350-klipper/
    prusa-mk3s-octoprint/
```

Best for:

- individual users;
- offline labs;
- CI pipelines;
- early implementation.

### Git registry

A Git repo of pack directories with signed releases/tags.

Best for:

- open community pack sharing;
- reviewable provenance;
- pull-request workflow;
- version pinning.

### HTTP registry

Read-only API for search, metadata and downloading signed packs.

Best for:

- commercial distribution;
- private organization registries;
- team/fleet management.

### Trust levels

| Level | Meaning | Can gate production? |
|---|---|---:|
| `draft` | manually authored, schema-valid only | no |
| `imported` | generated from a firmware/slicer source | no |
| `dry-verified` | Dry proofs pass against samples | maybe |
| `hardware-observed` | live API/hardware observation included | yes, with review |
| `maintained` | versioned, reviewed, signed, regression-tested | yes |

## Proof Report

Draft output:

```json
{
  "pack": "voron-2.4-350-klipper",
  "version": "0.1.0",
  "ok": true,
  "trust_level": "dry-verified",
  "checks": [
    {
      "id": "schema",
      "status": "pass",
      "message": "all pack files validate"
    },
    {
      "id": "flow-test-review",
      "status": "pass",
      "sample": "samples/flow-test.gcode",
      "expected": "proofs/flow-test.review.json"
    }
  ],
  "warnings": [
    "pressure advance declared but not modeled by dry-profile-v1"
  ]
}
```

## Adapters

### Klipper / Moonraker

Current state: Dry already imports Klipper `printer.cfg` into a basic profile.

Next adapter work:

- keep raw parsed config alongside normalized fields;
- import macros as macro entries;
- optionally query Moonraker printer objects for live limits/status;
- compare live values to pack expectations.

### OctoPrint

Adapter work:

- import printer profile API data: volume, bed, nozzle, extruders, speeds;
- import configured G-code scripts if available;
- export Dry pack/profiles for OctoPrint workflows.

### Cura

Adapter work:

- import machine and extruder definition files;
- map known machine settings into `PrinterCapabilities`;
- preserve unknown Cura keys in provenance/raw source.

### PrusaSlicer / OrcaSlicer / SuperSlicer

Adapter work:

- parse `.ini` config bundles;
- split printer, filament and print settings into pack profiles;
- map compatible printers/materials/process settings;
- generate Dry profile candidates.

### RepRapFirmware

Adapter work:

- parse `config.g` enough to extract kinematics, bounds, heaters and toolheads;
- import object model snapshots where available;
- preserve G-code command provenance.

## MVP Implementation Plan

### Phase 0: Spec-only foundation

Deliverables:

- `docs/marketing/printer-capability-library-plan.md`;
- draft pack examples under `spec/examples/printer-packs/`;
- draft JSON schemas:
  - `spec/dry-printer-pack-v1.schema.json`;
  - `spec/dry-printer-capabilities-v1.schema.json`;
  - `spec/dry-printer-proof-v1.schema.json`.

Exit:

- one hand-authored pack validates against schema;
- pack resolves to existing `dry-profile-v1`.

### Phase 1: Local library and CLI

Deliverables:

- local pack loader in Rust;
- `dry printer validate`;
- `dry printer inspect`;
- `dry printer resolve`;
- `dry printer prove`;
- TypeScript/Python wrappers for loading and resolving packs.

Exit:

- one Klipper pack can produce a Dry profile and run sample proofs.

### Phase 2: Importers

Deliverables:

- promote current `import-printer-cfg` into `dry printer import klipper`;
- Cura `.def.json` importer;
- Prusa/Orca/SuperSlicer `.ini` importer;
- OctoPrint profile importer.

Exit:

- imported packs preserve raw source + normalized fields;
- imports produce diffable profiles.

### Phase 3: Registry and signing

Deliverables:

- local registry index;
- Git registry format;
- pack integrity hash;
- optional signatures;
- `dry printer search/list/install/update`.

Exit:

- CI can pin a pack version and reproduce proof results.

### Phase 4: Live self-checks

Deliverables:

- Moonraker live query adapter;
- optional OctoPrint live query adapter;
- macro existence checks;
- observed-vs-declared report.

Exit:

- a live printer can be compared to its pack before accepting uploads.

## Product Packaging

### Proprietary Product Core

- pack schema;
- local loader;
- validation/proof runner;
- Klipper/Cura/Prusa importers where licensing permits.

These components are publicly inspectable in the repository and distributed under the proprietary
Dry licence or a negotiated OEM agreement; they are not published as an open-source package.

### Paid Team/Enterprise Layer

- private registry;
- team pack governance;
- signed/pinned packs;
- CI/report dashboards;
- vendor-maintained pack library;
- support for custom adapters and fleet migration.

## Risks

| Risk | Why it matters | Mitigation |
|---|---|---|
| Source formats are incomplete or undocumented | adapter fidelity will vary | preserve raw sources and mark confidence per field |
| Printer configs are instance-specific | sharing packs can be unsafe | distinguish printer-class vs printer-instance packs |
| Macros encode hidden assumptions | proofs may miss behavior | require declared params, sample jobs and human sign-off |
| Users expect auto-calibration | out of current Dry scope | frame as verification/test runner, not closed-loop tuning |
| Registry trust becomes a liability | bad packs can damage machines | trust levels, signed packs, explicit support status |

## Immediate Next Steps

1. Add example pack schema and one `voron-2.4-350-klipper` skeleton.
2. Add `dry printer validate` against manifest + existing Dry profile schema.
3. Add `dry printer resolve` to emit `dry-profile-v1`.
4. Convert current `import-printer-cfg` into a pack-producing path.
5. Add a proof runner for sample G-code review reports.

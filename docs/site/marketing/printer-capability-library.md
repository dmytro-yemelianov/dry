---
title: Printer Capability Library
pageClass: marketing-page
---

<section class="market-hero">
  <p class="market-eyebrow">Product architecture</p>
  <h1>Printer Capability Library</h1>
  <p class="market-lede">
    A unified API and package format for printer properties, firmware/slicer mappings, macros, checks,
    calibration routines, sample jobs and proof artifacts.
  </p>
  <div class="market-actions">
    <a href="#library-shape">Library</a>
    <a href="#cli-surface">CLI</a>
    <a href="#sdk-api">SDK API</a>
    <a href="#implementation-plan">Plan</a>
  </div>
</section>

## Product Thesis

Dry should normalize fragmented printer data into a versioned, testable capability layer:

<div class="market-flow" aria-label="Dry printer capability library pipeline">
  <div>Klipper / Moonraker / OctoPrint / Cura / Prusa profiles</div>
  <span>-></span>
  <div>Dry printer capability pack</div>
  <span>-></span>
  <div>CLI, SDK, registry, profiles, checks and proofs</div>
</div>

The existing `dry-profile-v1` remains the runtime profile used by `review`, `verify`, `rewrite` and `upload`.
The new library adds packaging, provenance, APIs and proofs around that profile.

This is the first vertical of the broader [G-code Machine SaaS](/marketing/gcode-machine-saas): printers use
the same registry, proof and edge-agent model as CNC, laser and other G-code-driven machines, but with
printer-specific fields for build volume, heaters, filament, slicer profiles and macros.

## Library Shape

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
    flow_test.klipper.cfg
  checks/
    bounds.json
    max-flow.json
    first-layer.json
  samples/
    flow-test.gcode
    pressure-advance.gcode
  proofs/
    manifest.proof.json
    flow-test.review.json
  provenance.json
```

<div class="market-grid">
  <article>
    <p class="card-label">Pack</p>
    <h3>Shared capability unit</h3>
    <p>A directory or archive that describes a printer class, instance or fleet policy.</p>
  </article>
  <article>
    <p class="card-label">Profile</p>
    <h3>Runtime contract</h3>
    <p>A resolved `dry-profile-v1` for one material/process/nozzle combination.</p>
  </article>
  <article>
    <p class="card-label">Proof</p>
    <h3>Reproducible evidence</h3>
    <p>Schema, adapter, sample, macro and optional hardware-observed validation reports.</p>
  </article>
</div>

## CLI Surface

### Registry and pack management

```bash
dry printer list --source ./printer-packs
dry printer search voron --source https://api.dry.yemelianov.dev
dry printer inspect voron-2.4-350-klipper
dry printer validate ./packs/voron-2.4-350-klipper
dry printer pack ./packs/voron-2.4-350-klipper -o voron-2.4-350-klipper.drypack
```

### Importers

```bash
dry printer import klipper printer.cfg --id voron-2.4-350-klipper -o ./packs/voron/
dry printer import moonraker http://printer.local --id voron-live -o ./packs/voron-live/
dry printer import octoprint http://octopi.local --id mk3-octoprint -o ./packs/mk3/
dry printer import cura fdmprinter.def.json --id custom-cura -o ./packs/custom/
dry printer import prusa bundle.ini --id prusa-bundle -o ./packs/prusa/
```

### Profile resolution and proofs

```bash
dry printer resolve voron-2.4-350-klipper --material ABS --nozzle 0.4 -o profile.json
dry printer diff old-pack/ new-pack/
dry printer checks voron-2.4-350-klipper
dry printer run-check voron-2.4-350-klipper max-flow
dry printer run-sample voron-2.4-350-klipper flow-test
dry printer prove voron-2.4-350-klipper --ci
```

### Existing command integration

```bash
dry review-gcode part.gcode --printer voron-2.4-350-klipper --material ABS --nozzle 0.4
dry upload part.gcode --printer voron-2.4-350-klipper --moonraker http://printer.local
dry compare baseline.gcode candidate.gcode --printer voron-2.4-350-klipper
```

`--printer` resolves to a profile and passes the resulting `dry-profile-v1` into existing flows. `--profile`
remains the explicit low-level option.

## SDK API

### TypeScript

```ts
import { PrinterRegistry } from "@dry/sdk/printers";

const registry = await PrinterRegistry.open({
  sources: ["./printer-packs", "https://api.dry.yemelianov.dev"],
});

const printer = await registry.get("voron-2.4-350-klipper");
const caps = await printer.capabilities();

const profile = await printer.resolveProfile({
  material: "ABS",
  nozzleDiameter: 0.4,
});

const report = await printer.reviewGcode("part.gcode", { profile });
const proof = await printer.runProofs();
```

### Python

```python
from dry.printers import PrinterRegistry

registry = PrinterRegistry.open(
    sources=["./printer-packs", "https://api.dry.yemelianov.dev"]
)

printer = registry.get("voron-2.4-350-klipper")
profile = printer.resolve_profile(material="ABS", nozzle_diameter=0.4)
report = printer.review_gcode("part.gcode", profile=profile)
proof = printer.run_proofs()
```

## Registry Model

| Registry | Best for | Notes |
|---|---|---|
| Local directory | users, offline labs, CI | simplest implementation path |
| Git registry | community sharing, version pinning | reviewable provenance and pull requests |
| HTTP registry | private teams, commercial distribution | search, signed downloads, support tiers |

Trust levels:

| Level | Meaning | Can gate production? |
|---|---|---:|
| `draft` | manually authored, schema-valid only | no |
| `imported` | generated from firmware/slicer source | no |
| `dry-verified` | Dry proofs pass against samples | maybe |
| `hardware-observed` | live API/hardware observation included | yes, with review |
| `maintained` | versioned, reviewed, signed, regression-tested | yes |

## Adapters

<div class="market-grid">
  <article>
    <h3>Klipper / Moonraker</h3>
    <p>Promote current `import-printer-cfg` into pack import, keep raw config, import macros and optionally query live printer objects.</p>
  </article>
  <article>
    <h3>OctoPrint</h3>
    <p>Import printer profile data, G-code scripts and physical properties for review/upload workflows.</p>
  </article>
  <article>
    <h3>Cura</h3>
    <p>Import machine and extruder definitions and preserve unknown Cura keys as provenance.</p>
  </article>
  <article>
    <h3>Prusa / Orca / SuperSlicer</h3>
    <p>Parse config bundles into printer, filament and process candidates for Dry profiles.</p>
  </article>
  <article>
    <h3>RepRapFirmware</h3>
    <p>Parse `config.g` enough to extract kinematics, bounds, heaters and toolheads.</p>
  </article>
</div>

## Proof Report

```json
{
  "pack": "voron-2.4-350-klipper",
  "version": "0.1.0",
  "ok": true,
  "trust_level": "dry-verified",
  "checks": [
    { "id": "schema", "status": "pass" },
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

## Implementation Plan

| Phase | Deliverables | Exit |
|---|---|---|
| 0. Spec foundation | draft schemas and one example pack | one pack validates and resolves to `dry-profile-v1` |
| 1. Local CLI/library | loader, validate, inspect, resolve, prove | one Klipper pack produces a profile and runs sample proofs |
| 2. Importers | Klipper, Cura, Prusa/Orca/SuperSlicer, OctoPrint | imports preserve raw source and normalized fields |
| 3. Registry/signing | local index, Git registry, hashes/signatures | CI can pin pack version and reproduce proofs |
| 4. Live self-checks | Moonraker/OctoPrint live adapters | live printer is compared to declared pack before upload |

## Product Packaging

Proprietary product core:

- pack schema;
- local loader;
- validation/proof runner;
- commercially licensed importers where upstream terms permit.

Paid team/enterprise:

- private registry;
- signed/pinned packs;
- team governance;
- CI/report dashboard;
- vendor-maintained pack library;
- custom adapters and fleet migration support.

The full implementation plan is maintained in the public repository at
`docs/marketing/printer-capability-library-plan.md`.

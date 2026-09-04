# Dry (DryMachina)

[![Version](https://img.shields.io/badge/version-0.9.1-blue.svg)](Cargo.toml)
[![License: FSL-1.1-MIT](https://img.shields.io/badge/License-FSL--1.1--MIT-blue.svg)](LICENSE)
[![Rust: 1.88+](https://img.shields.io/badge/rust-1.88+-orange.svg)](https://www.rust-lang.org/)
[![Python: 3.9+](https://img.shields.io/badge/python-3.9+-green.svg)](https://www.python.org/)
[![TypeScript: 5.6+](https://img.shields.io/badge/typescript-5.6+-blue.svg)](https://www.typescriptlang.org/)
[![Formal: Lean 4](https://img.shields.io/badge/formal-Lean%204-purple.svg)](formal/)

**Deterministic Toolpath Compiler & Verification Engine** — a typed, units-aware, multi-level intermediate representation (the **Dry IR**) with a high-performance Rust kernel, formal numeric assurance proofs, and native front-ends in Python, TypeScript, WebAssembly, and AI Model Context Protocol (MCP).

Think **LLVM/MLIR for machine motion & precision CAM**.

---

## 1. Why Dry?

Traditional CAM and slicing tools treat toolpaths as lossy polyline G-code without semantic typing, dynamic safety contracts, or multi-axis awareness. 

Dry solves this by treating machine motion as a **formal compiler pipeline**:
- **Multi-Level Dialects**: Progressive lowering from L0 Features $\rightarrow$ L1 Operations $\rightarrow$ L2 Kinematic Toolpaths $\rightarrow$ L3 Dialect Machine Code.
- **Dimensional Type System**: Lengths, Speeds, Volumes, Flow Rates, Angles, and Temperatures are enforced at compile time.
- **Formal Verification & Safety Contracts**: Lean 4 verified proofs for curvature continuity and singularity avoidance, paired with continuous bounding volume checks.
- **Multi-Modal Motion**: Native support for Additive (FFF), High-Speed CNC Milling (Trochoidal/Helical), CNC Lathe Turning/Facing, 5-Axis Non-Planar Draping, and 6-Axis Industrial Robotics (KUKA KRL, ABB RAPID).

---

## 2. Architecture & Capabilities Matrix

```
  Authoring Front-Ends                Rust Core Engine                       Target Machine Dialects
┌──────────────────────┐    ┌───────────────────────────────────┐    ┌───────────────────────────────────┐
│ • Python SDK (PyO3)  │───►│ L0: Feature Program & STEP Solids │───►│ • 3D Printing: Marlin, Klipper,   │
│ • TypeScript / Wasm  │    │ L1: Ordered Ops (Arc/Clothoid)    │    │   Duet, RepRapFirmware            │
│ • Rust Native API    │    │ L2: Kinematic Toolpaths & Physics │    │ • CNC Milling/Lathe: RS-274, GRBL │
│ • AI MCP Agent Server│    │ L3: Optimizer & Verifier Engine   │    │ • Industrial Robotics: KUKA KRL,  │
└──────────────────────┘    └───────────────────────────────────┘    │   ABB RAPID, STEP-NC AP238        │
                                                                     └───────────────────────────────────┘
```

| Manufacturing Domain | Primitives & Capabilities | Target Dialects |
|---|---|---|
| **Additive Manufacturing** | Helical vase, continuous perimeter, retraction kinematics, volumetric flow clamping | Marlin, Klipper, Duet, Bambu Lab, Prusa |
| **High-Speed CNC Milling** | Rectangular/circular pockets, trochoidal milling, helical ramps, corner chip thinning | RS-274 / LinuxCNC, GRBL, ISO 14649 STEP-NC |
| **CNC Lathe Turning** | Multi-pass facing, stepped outer-diameter (OD) roughing & finishing, spindle speed sync | RS-274, GRBL Lathe ($XZ$ plane) |
| **5-Axis & Surface CAM** | Surface normal draping, tilted toolholder collision detection, table kinematics (AC/BC/AB) | 5-Axis G-code, ISO 14649 STEP-NC AP238 |
| **Industrial Robotics** | Forward/Inverse kinematics, wrist singularity hold, synchronized multi-robot workcells | KUKA KRL, ABB RAPID |
| **Cellular Metamaterials** | Triply Periodic Minimal Surfaces (Gyroid, Schwarz P/D, Lidinoid, Neovius, Fischer-Koch) | Direct contour slicing to G-code / IR |

---

## 3. Quickstart

### A. Rust CLI (`dry-cli`)
```bash
# Build the standalone compiler binary
cargo build -p dry-cli --release

# Inspect & simulate a design
cargo run -p dry-cli --bin dry -- inspect conformance/gcode/square.json
cargo run -p dry-cli --bin dry -- simulate conformance/gcode/square.json

# Pre-flight G-code verification against machine safety contracts
cargo run -p dry-cli --bin dry -- review-gcode examples/part.gcode --bounds 0,250,0,210,0,220 --max-feedrate 18000

# Optimize G-code (S-Curves, Clothoid blending, Arc fitting)
cargo run -p dry-cli --bin dry -- rewrite-gcode examples/part.gcode --optimize -o optimized.gcode
```

### B. Python SDK (`py/`)
```python
import dry

# Build a parametric toolpath with arc-native moves
design = (
    dry.Design()
    .geometry(width=0.6, height=0.2)
    .extruder(True)
    .point(0, 0, 0.2)
    .point(50, 0, 0.2)
    .arc(cx=50, cy=25, x=50, y=50) # G3 arc
    .point(0, 50, 0.2)
)

# Simulate physics and kinematic cycle time
metrics = design.simulate()
print(f"Machining Time: {metrics['total_time_s']:.1f}s, Segments: {metrics['segment_count']}")

# Verify safety contracts
report = design.verify(bounds=[[0, 200], [0, 200], [0, 200]], max_feedrate=18000)
print(f"Safety Violations: {len(report['findings'])}")

# Emit machine-ready G-code
gcode_lines = design.gcode(flavor="klipper")
```

### C. TypeScript & In-Browser WebAssembly SDK (`sdk/ts/` & `web/`)
```typescript
import { Design, pocket_ops, lathe_facing_ops } from '@dry/sdk';

const design = new Design()
  .geometry(0.6, 0.2)
  .extruder(true)
  .point(10, 0, 0.2)
  .arc({ cx: 0, cy: 0, x: 0, y: 10 })
  .point(0, 20, 0.2);

// Client-side simulation & verification via WebAssembly
const report = design.verify({ bounds: [[0, 200], [0, 200], [0, 200]] });
console.log(design.gcode().join('\n'));
```

### D. Docker Container Verification Daemon (`containers/verify-runner`)
```bash
# Build multi-arch container image
docker build -f containers/verify-runner/Dockerfile -t dry-verify-runner .

# Run high-throughput streaming verification daemon
docker run -p 8080:8080 dry-verify-runner

# Verify G-code payload via HTTP API
curl -X POST http://localhost:8080/verify \
  -H "Content-Type: text/plain" \
  -H "X-Dry-Contracts: {\"bounds\":[[0,250],[0,210],[0,220]],\"max_feedrate\":18000}" \
  --data-binary @examples/part.gcode
```

### E. AI Model Context Protocol (MCP) Server (`sdk/mcp/`)
Connect Claude Desktop, Cursor, or autonomous AI agents directly to Dry:
```json
{
  "mcpServers": {
    "dry": {
      "command": "node",
      "args": ["/path/to/dry/sdk/mcp/dist/index.js"]
    }
  }
}
```

---

## 4. Documentation Index

| Guide / Specification | Topic & Scope |
|---|---|
| [`docs/00-vision-and-scope.md`](docs/00-vision-and-scope.md) | Thesis, core architecture, success criteria, and non-goals |
| [`docs/01-architecture.md`](docs/01-architecture.md) | Detailed IR dialect specifications (L0–L3), toolframe, and units |
| [`docs/02-roadmap.md`](docs/02-roadmap.md) | Multi-phase development roadmap and milestone gates |
| [`docs/03-conformance.md`](docs/03-conformance.md) | Conformance oracle testing and parity gates |
| [`docs/06-lattice-research-codegen.md`](docs/06-lattice-research-codegen.md) | Star-polygon architectured planar lattice generators |
| [`docs/07-tpms-codegen.md`](docs/07-tpms-codegen.md) | TPMS minimal surface implicit-field contour generators |
| [`docs/10-dry-ir-v0-spec.md`](docs/10-dry-ir-v0-spec.md) | Normative Dry IR v0 specification (JSON + DRY0/DRY1 binary) |
| [`docs/11-profiles-and-reports.md`](docs/11-profiles-and-reports.md) | Machine profiles and verification report schemas |
| [`docs/13-performance-and-scale.md`](docs/13-performance-and-scale.md) | Memory model, streaming codecs, and benchmarks |
| [`docs/17-provenance-and-licensing.md`](docs/17-provenance-and-licensing.md) | Auditable corpus-provenance ledger & dependency audit |
| [`docs/22-krl-emit.md`](docs/22-krl-emit.md) | KUKA Robot Language (KRL) kinematics and emission |
| [`docs/26-industrial-certification-and-standards.md`](docs/26-industrial-certification-and-standards.md) | Industrial compliance evidence kits (DO-178C, ISO 26262, IEC 62304) |
| [`docs/27-verification-deployment-architecture.md`](docs/27-verification-deployment-architecture.md) | 3-Tier Multi-Modal Verification Architecture (Wasm, Edge, Container) |
| [`docs/CLEANROOM.md`](docs/CLEANROOM.md) | Clean-room provenance policy and oracle quarantine rules |
| [`AUTHORS.md`](AUTHORS.md) | Authors, maintainers, academic research citations, and DCO 1.1 |

---

## 5. Clean-Room Provenance & Oracle Isolation

Dry is an **independent, clean-room implementation** authored from specification and first principles. FullControl (GPLv3) is used solely as an external dev/CI-time test oracle generating reference outputs for regression verification. No FullControl source code is copied into or shipped with Dry. See [`docs/CLEANROOM.md`](docs/CLEANROOM.md) and [`docs/17-provenance-and-licensing.md`](docs/17-provenance-and-licensing.md).

---

## 6. License & Commercial Terms

Dry is licensed under the **Functional Source License, Version 1.1, MIT Future License (FSL-1.1-MIT)**.

- **Free for Developers & Startups:** You may freely copy, modify, distribute, and integrate Dry into non-competing applications, internal manufacturing, research, and hobby projects.
- **Competing Use Protection:** A commercial license is required only if you use Dry to provide a competing CAM, slicing, or toolpath compilation/verification service or proprietary OEM hardware bundling.
- **2-Year Automatic MIT Conversion:** Every release automatically converts to standard permissive **MIT License** exactly two (2) years after release (September 4, 2028 for v0.9.1).

See [`LICENSE`](LICENSE), [`NOTICE`](NOTICE), [`AUTHORS.md`](AUTHORS.md), and [`TRADEMARKS.md`](TRADEMARKS.md) for full terms.

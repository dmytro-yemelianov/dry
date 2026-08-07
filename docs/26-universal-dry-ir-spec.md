# Universal Dry IR — Architecture, Ontology & Mathematical Specification

**Version:** 1.0.0  
**Status:** Normative Standard  
**Document ID:** `DRY-SPEC-2026-V1`

---

## Executive Summary

Dry IR is a self-contained, machine-independent, unit-typed, multi-level **Universal Intermediate Representation** for automated manufacturing toolpath compilation. It serves as the single public contract between design front-ends (Python, TypeScript, Rust, CAD plugins) and target execution engines (FFF G-code, RS-274 CNC, GRBL laser, KUKA KRL robot arms).

This specification formalizes the **Manufacturing Ontology**, **SE(3) Kinematic Pose Graph**, **Lean 4 Mathematical Preservation Laws**, **Declarative Machine Capability Schemas**, **Bidirectional Lifter Taxonomy**, and **Cryptographic Provenance Model**.

---

## 1. Domain Ontology & Multi-Level Dialect Taxonomy

Dry IR defines four formal representation levels ($L_0$ to $L_3$), enforcing strict separation of concerns:

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Level 0: Manufacturing Intent (Features, Pockets, TPMS Fields, Volumes) │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │ expand_features
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ Level 1: Path Dialect (Non-modal Waypoints, Curves, Process Channels)   │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │ resolve
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ Level 2: Motion Dialect (Absolute SE(3) Trajectories, Process Traits)   │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │ emit (gated by Machine Capability)
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ Level 3: Target Dialect (Marlin, Klipper, RS-274 CNC, GRBL, KUKA KRL)   │
└─────────────────────────────────────────────────────────────────────────┘
```

### 1.1 Dimensional Quantities Algebra
All numerical fields carry explicit SI dimensions ($L^a M^b T^c I^d \Theta^e N^f J^g$). Mixed-unit operations are verified at compile time:

$$\text{Length} \times \text{Length} = \text{Area} \quad (L^1 \cdot L^1 = L^2)$$
$$\text{Volume} \div \text{Area} = \text{Length} \quad (L^3 \cdot L^{-2} = L^1)$$
$$\text{Length} \div \text{Feedrate} = \text{Time} \quad (L^1 \cdot (L^1 T^{-1})^{-1} = T^1)$$

### 1.2 SE(3) Kinematic Pose Graph
Every waypoint and trajectory segment is defined as a rigid-body pose $T \in \text{SE}(3) = \mathbb{R}^3 \rtimes \text{SO}(3)$, carrying 3D spatial position $(X, Y, Z)$ and a 3D unit orientation vector $\mathbf{u} = (i, j, k)$ where $\|\mathbf{u}\|_2 = 1$.

Positions are defined relative to a directed acyclic **Kinematic Frame Graph**:
$$\text{World} \xrightarrow{T_{\text{WM}}} \text{Machine} \xrightarrow{T_{\text{MW}}} \text{Workpiece} \xrightarrow{T_{\text{WT}}} \text{Tool} \xrightarrow{T_{\text{TCP}}} \text{TCP}$$

---

## 2. Machine Capability Schemas & Failure-Closed Lowering

Target compilation is gated by a normative **Machine Capability Schema** (`spec/machine-capability.schema.json`).

### 2.1 Capability Matching Protocol
Before emitting $L_3$ code, the compiler checks the $L_2$ toolpath against target constraints:
1. **Kinematic Bounds**: Maximum rotary travel ($\theta_{min..max}$), angular acceleration, and singularity cone limits ($\mathbf{k} = \pm 1$).
2. **Process Limits**: Maximum volumetric flow rate ($Q_{\max}$), nozzle temperature envelope ($T_{\min..\max}$), build-volume bounding box ($[X_{\min..\max}, Y_{\min..\max}, Z_{\min..\max}]$).
3. **Target Vocabulary**: Supported G-code words ($G_0, G_1, G_2, G_3, G_4$), M-codes ($M_3, M_5, M_{104}, M_{106}$), and controller macro policies.

### 2.2 Failure-Closed Policy
If an IR toolpath requires capabilities unsupported by the target schema (e.g., 5-axis orientation on a 3-axis FFF printer, or laser power $S$ on an extruder-only firmware), compilation fails closed at the IR gate with a located diagnostic:

$$\text{Match}(P_{\text{IR}}, \mathcal{C}_{\text{Machine}}) = \text{Refusal}(\text{RuleID}, \text{SegmentIndex}, \text{Message})$$

---

## 3. Mathematical Preservation & Assurance Laws (Lean 4)

Lowering passes and optimization transforms must satisfy machine-checked Lean 4 theorems:

### 3.1 Trace Equivalence ($\equiv_{\text{trace}}$)
Lowering $L_1 \to L_2$ preserves the ordered continuous spatiotemporal deposition trace:
$$\text{Trace}(L_1) \equiv_{\text{trace}} \text{Trace}(\text{resolve}(L_1))$$

### 3.2 Bounded Geometric Approximation ($\approx_\varepsilon$)
Curve discretization (Catmull-Rom splines, Euler-spiral clothoids, arc fitting) satisfies an explicit floating-point error bound $\varepsilon$:
$$\sup_{t \in [0, 1]} \|\gamma(t) - \hat{\gamma}(t)\|_2 \le \varepsilon$$

### 3.3 Material Volume Conservation
$L_2$ optimization passes (`merge_collinear`, `travel_reorder`, `coasting`, `z_hop`) conserve total extruded volume:
$$\int_{0}^{T_{\text{end}}} Q_{\text{flow}}(t) \, dt = V_{\text{target}}$$

### 3.4 Top-Level Conditional Compiler Safety Theorem
$$\text{Compile}(P_{\text{IR}}, \mathcal{C}, \text{Target}) = \text{Success}(P_{\text{L3}}) \implies \text{Trace}(P_{\text{L3}}) \sqsubseteq_{\mathcal{C}} \text{Trace}(P_{\text{IR}})$$

---

## 4. Bidirectional Lifter & Lossiness Taxonomy ($L_3 \to L_2$)

Importing existing machine code ($L_3$) back into Dry IR ($L_2$) uses a tri-tier lossiness classification:

1. **Exact Recovered Semantics**: Linear moves, arc center/radius, feedrate, coordinates.
2. **Inferred Semantics**: Layer linkage (`trace.layers`), feature classification (infill vs perimeter), extrusion bead geometry.
3. **Quarantined Opaque Nodes**: Vendor macros (`M109`, `G29`, custom scripts) are isolated into `manual_gcode` IR nodes, preserving surrounding motion state without fabricating intent.

---

## 5. Multi-Process Hybrid Manufacturing Taxonomy

Dry IR natively supports four distinct manufacturing processes:

| Process Family | Primary Channel | Motion Primitive | Target Backend |
|---|---|---|---|
| **Additive (FFF/DED/WAAM)** | `volume`, `flow`, `temperature` | $L_2$ Extruding Line / Arc / Spline | Marlin, Klipper, RepRapFirmware |
| **Subtractive (CNC)** | `spindle_rpm`, `coolant`, `wcs` | Pocket / Profile / 5-Axis Contour | RS-274 (LinuxCNC, Fanuc), STEP-NC |
| **Directed Energy (Laser)** | `power` (PWM / RPM $S$-word) | Modal Power Line / Arc | GRBL, Marlin Laser |
| **Robotic Manipulation** | `orientation` $(i, j, k)$, `tool` | 6-DOF Cartesian / Euler Pose | KUKA KRL, ABB RAPID, Fanuc |

---

## 6. Spatiotemporal Trace Analytics & Forensic Partitioning

Dry IR incorporates an analytic trace model for quality assurance and fleet telemetry:
- **Layer Partitioning (`trace.layers`)**: Exact spatial grouping keyed on extrusion $Z$ height.
- **Statistical Analytics (`trace.analytics`)**: Phase-split time-weighted statistics, nearest-rank flow percentiles, feedrate distributions, and flow-outlier windowing ($K$-sigma detection).
- **Export Encodings**: JSON, CSV, and zero-copy Arrow/Parquet streams for fleet management.

---

## 7. Cryptographic Security & Provenance Model

- **Canonical Manifest Hashing**: Every Dry IR binary (`DRY0` columnar, `DRY1` chunked streaming) carries a SHA-256 hash of its design intent and resolved motion stream.
- **Cryptographic License Stamping**: Signed tokens (`dry-license`) authenticate toolpath provenance and enforce customer IP security over Cloud/Moonraker network boundaries.
- **Software Bill of Materials (SBOM)**: Emitted programs contain structured headers recording compiler version, verification hashes, and audit logs.

# Chapter 1: System Specification & Formal Contracts

## 1. Dialect Invariants & Lowering Pipeline

The Dry architecture is governed by strict mathematical dialect lowerings:

### Dialect L0: The Feature Graph
* **Purpose**: High-level parametric authoring, geometric repetition, and hierarchical assembly composition.
* **Core Primitives**: `FeatureProgram`, `Feature@pose` (translation + planar rotation), `Group` (ordered composition), `Repeat` (affine progression).
* **Guarantees**: Coordinate-local isolation; deterministic transform propagation; rejects transformed manual G-code.

### Dialect L1: The Path Dialect
* **Purpose**: Canonical machine path authoring before physical discretization.
* **Core Primitives**: `Op::Move`, `Op::Arc` (centre-form or 3-point), `Op::Clothoid` (Euler spiral transition), `Op::Spline` (Catmull-Rom), `Op::Power`, `Op::Temperature`, `Op::Fan`, `Op::Tool`, `Op::Dwell`, `Op::Orient`.
* **Lowering ($L0 \to L1$)**: Performed by `expand_features`, producing a linear stream of operations with inherited running positions.

### Dialect L2: The Motion Dialect (Dry IR)
* **Purpose**: Fully resolved, kinematic-aware physical segments with explicit coordinates, flow rates, and active machine channels.
* **Core Primitives**: `Toolpath`, `Segment { start, end, speed, extrusion, temperature, fan, tool, power, orientation }`.
* **Lowering ($L1 \to L2$)**: Performed by `resolve`, evaluating extrusion volume from nozzle geometry, propagating process channels, and validating unit boundaries.

### Dialect L3: Target Machine Programs
* **Purpose**: Physical machine execution without ambient runtime calculation.
* **Targets**: Marlin, Klipper, RepRapFirmware (Duet), LinuxCNC (RS-274), GRBL, KUKA KRL, STEP-NC XML.

---

## 2. Dry IR Wire Encodings

Dry IR supports three complementary serializations:

1. **JSON Encoding (`.dry.json`)**: Human-readable, schema-validated canonical representation.
2. **Columnar Binary `DRY0` (`Toolpath::to_bytes`)**:
   * Arrow-style columnar layout under DEFLATE compression.
   * Employs bit-packed validity maps for optional fields (`temperature`, `fan`, `power`, `orientation`).
   * Achieves **25.3× size reduction** over JSON.
3. **Streaming Block `DRY1` (`Toolpath::to_streaming_bytes`)**:
   * Chunked, framed row-blocks enabling bounded-memory streaming execution on multi-million segment files ($>1\text{M}$ segments).

---

## 3. Dimensional Units System

Dry enforces physical unit correctness at compile time in `crates/core/src/units.rs`:
* `Length` ($\text{mm}$), `Area` ($\text{mm}^2$), `Volume` ($\text{mm}^3$)
* `Feedrate` ($\text{mm/s}$ or $\text{mm/min}$), `Time` ($\text{s}$ or $\text{ms}$)
* `Flow` ($\text{mm}^3\text{/s}$), `Angle` ($\text{rad}$ or $\text{deg}$)

All operators enforce dimensional conservation:
$$\text{Length} \times \text{Length} = \text{Area}$$
$$\text{Area} \times \text{Length} = \text{Volume}$$
$$\text{Volume} \div \text{Time} = \text{Flow}$$

Mixed-unit arithmetic without explicit dimensional operators fails at compilation.

---

## 4. Machine Safety Verification Contracts

`dry verify` enforces 15 automated rules producing structured `Report` findings:

| Contract Rule | Severity | Condition Checked |
|---|---|---|
| `bounds` | Error | Segment coordinates exceed `[[x0, x1], [y0, y1], [z0, z1]]` |
| `monotonic-z` | Error | Toolpath Z-coordinate decreases during vase-mode printing |
| `flow-rate` | Warning/Error | Volumetric extrusion rate exceeds `max_flow` ($\text{mm}^3\text{/s}$) |
| `cold-extrusion` | Error | Extrusion attempted when hotend $< \text{min\_temp}$ |
| `finite-coordinates` | Error | Coordinates contain `NaN`, `+Inf`, or `-Inf` |
| `retraction-distance` | Warning | Filament retraction exceeds `max_retraction_distance` |
| `retraction-speed` | Warning | Retraction feedrate exceeds `max_retraction_speed` |
| `travel-without-retract`| Warning | Non-extruding travel distance exceeds stringing ceiling |
| `first-layer-speed` | Warning | First layer feedrate exceeds adhesion limit |
| `first-layer-height`| Warning | First layer layer height exceeds adhesion envelope |
| `orientation-not-unit` | Error | Toolframe orientation vector magnitude $\ne 1.0$ |
| `spindle-ceiling` | Warning | Spindle RPM / laser power exceeds machine max |
| `feedrate-ceiling` | Warning | Feedrate exceeds rapid limit for active axis |

---

## 5. Industrial Standards & Qualification Framework

The platform architecture complies with formal industrial and safety-critical standards documented in [`docs/26-industrial-certification-and-standards.md`](../26-industrial-certification-and-standards.md):

* **ISO/ASTM 52915 (3MF)**: Toolpath Extension XML interchange.
* **ISO 14649 (STEP-NC AP 238)**: Feature-based CNC process plans.
* **DO-178C / DO-333**: Machine-checked formal verification in Lean 4 for flight-critical manufacturing.
* **IEC 62304**: Medical device software life cycle for patient-specific orthopedic implants.
* **SOC 2 Type II & SLSA Level 3**: Cryptographically verified supply chain and zero-retention cloud verification.


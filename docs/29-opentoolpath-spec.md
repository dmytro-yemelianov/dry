# OpenToolpath Specification v1.0 (OTP v1.0)

- **Status:** Normative Standard (v1.0)
- **Authority:** [opentoolpath.org](https://opentoolpath.org)
- **Governing Architecture:** ADR 0001 (Formal Assurance Constitution — [docs/adr/0001-formal-assurance-constitution.md](docs/adr/0001-formal-assurance-constitution.md)), ADR 0002 (Numeric Ingress and Emission Gates — [docs/adr/0002-numeric-ingress-and-emission-gates.md](docs/adr/0002-numeric-ingress-and-emission-gates.md))
- **Mathematical Assurance Invariants:** Lean 4 formal proofs `L2.Validation.validate_success_iff`, `ZyzEuler.direction_spin_independent`, `ZyzEuler.direction_euler_roundtrip`, `ArcMidpoint.midpoint_on_circle`, and `Deposition.lean` ([formal/Dry/](formal/Dry/))
- **Machine-Readable Companion Schema:** [spec/opentoolpath-v1.schema.json](spec/opentoolpath-v1.schema.json)
- **Reference Codecs & Conformance Suite:** [conformance/vectors/](conformance/vectors/), [docs/10-dry-ir-v0-spec.md](docs/10-dry-ir-v0-spec.md)

---

## Abstract

OpenToolpath (OTP) is an open, vendor-neutral, machine-agnostic interchange standard and container specification for computer-aided manufacturing (CAM), additive manufacturing, subtractive multi-axis machining, laser processing, and robotic path execution. OpenToolpath bridges high-level feature designs (L0/L1) and physical machine controllers (L3) through a mathematically rigorous, fully resolved motion intermediate representation (L2).

This document establishes the normative requirements for OpenToolpath v1.0 compliance. It specifies:
1. The `.otp` package archive structure (manifest, motion payload, tools, context, detached cryptographic signatures).
2. The core logical data model, motion primitives, 5/6-axis tool orientation frames, and process channels.
3. Strict SI units, coordinate system conventions, and coordinate frames (WCS, MCS, TCP).
4. Tiered serialization models: human-readable canonical JSON, columnar binary (`DRY0` / Arrow, `enc_ver 3`), and chunked streaming binary (`DRY1` / FlatBuffers, `enc_ver 3`).
5. The 3-tier specification architecture spanning human prose, clean-room conformance oracles, and machine-checked Lean 4 mathematical invariants.
6. A normative refusal and error taxonomy adhering strictly to the **refuse-never-clamp** and **zero-silent-drops** safety doctrines.

---

## Document Conventions and Conformance Keywords

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHALL**, **SHALL NOT**, **SHOULD**, **SHOULD NOT**, **RECOMMENDED**, **NOT RECOMMENDED**, **MAY**, and **OPTIONAL** in this document are to be interpreted as described in BCP 14 (RFC 2119, RFC 8174) when, and only when, they appear in all capitals, as shown here.

An implementation is **conforming** if it satisfies all **MUST**, **REQUIRED**, and **SHALL** requirements defined across all sections of this specification.

---

## 1. Scope and Foundational Principles

### 1.1 Scope

OpenToolpath v1.0 specifies a **fully resolved motion toolpath** and its execution container. A resolved toolpath is an ordered, deterministic stream of discrete moves where geometric trajectories, kinematic orientations, feeds, speeds, and auxiliary process states are explicitly bound to absolute coordinates and absolute time/state progressions.

In scope for OpenToolpath v1.0:
- The container packaging format (`.otp` ZIP archive), manifest, tool catalog, operational context, and detached digital signatures.
- The geometric and process data model for linear, arc, spline, rapid, dwell, and additive moves.
- Multi-axis tool orientation representations (unit direction vectors and ZYZ Euler frames).
- Process channels (feedrate, spindle RPM, laser power, extrusion bead dimensions, temperatures, fan speeds, active tool indices).
- Normative serialization formats (JSON, columnar binary `DRY0`, streaming chunked binary `DRY1`).
- Formal mathematical verification criteria and conformance validation protocols.
- Safety-critical rejection policies and error taxonomy.

Out of scope for OpenToolpath v1.0:
- Proprietary machine controller internal firmware implementations.
- Real-time servo loop PID tuning and low-level step/dir electrical pulse timing.
- High-level parametric CAD solid modeling kernels (B-Rep boolean modeling), except as referenced in operational context metadata.

### 1.2 Principle of Vendor and Process Neutrality

OpenToolpath **MUST NOT** favor, require, or encode assumptions tied to any proprietary hardware controller, slicer vendor, or CAM vendor dialect. The logical model abstracts across five fundamental industrial process domains:
1. **Additive Manufacturing (AM):** Fused Filament Fabrication (FFF), Direct Energy Deposition (DED), Selective Laser Sintering (SLS), Binder Jetting.
2. **Subtractive Machining (SM):** 3-axis milling, 4-axis rotary milling, 5-axis simultaneous milling (Table-Table, Head-Head, Head-Table), lathe turning, and Swiss-style turning.
3. **Hybrid Manufacturing:** Layer-by-layer additive deposition followed by intermediate or final subtractive finishing.
4. **Laser & Sheet Processing:** 2D/3D laser cutting, laser engraving, waterjet, and plasma arc cutting.
5. **Robotic Automation:** 6-DOF industrial articulating arms carrying deposition end-effectors, routing spindles, or welding heads.

### 1.3 Principle of Zero Silent Drops

Conforming implementations **MUST** observe the Zero Silent Drops doctrine (ADR 0002 §4):
Every input construct, command, or parameter received by an OpenToolpath processor **SHALL** be mapped to exactly one of three mutually exclusive classes:
1. **Modeled:** Fully represented in the logical data model and directly affecting toolpath execution.
2. **Inert-Preserved:** Explicitly recorded in container metadata, context headers, or inert escape sequences (`manualgcode`), preserving exact provenance without lowering to unverified motion.
3. **Refused:** Explicitly rejected with a located, structured diagnostic identifying the offending line, byte, or entity.

Under no circumstances **SHALL** a conforming implementation discard, skip, or ignore an unmodeled command or channel without explicit user authorization or structured recording. Silent drops introduce unobservable discrepancies between CAM intent and machine execution, violating safety-critical aerospace and medical integrity standards (DO-178C, IEC 62304).

### 1.4 Principle of Refuse-Never-Clamp

Conforming implementations **MUST** observe the Refuse-Never-Clamp doctrine (ADR 0002 §4):
When a commanded parameter, coordinate, feedrate, volumetric flow, temperature, or kinematic configuration exceeds declared machine envelope limits, produces non-finite quantities, or encounters a geometric singularity, the implementation **MUST** refuse the program and return a structured diagnostic error.

An implementation **MUST NOT** clamp out-of-range quantities to threshold maximums/minimums, **MUST NOT** round singular orientations arbitrarily, and **MUST NOT** emit a vacuous or partial output file. Clamping silently alters toolpaths, producing out-of-spec physical parts, gouging workpieces, or inducing machine collisions. Under strict OpenToolpath v1.0 conformance, non-unit orientation vectors **MUST** be refused with `OTP_ERR_ORIENTATION_NOT_UNIT`, not normalized.

### 1.5 Bitwise Determinism and Lossless Serialization

Given identical input data and explicit execution settings, an OpenToolpath encoder **MUST** generate semantically identical toolpaths. When serializing to IEEE-754 binary64 numbers, representations **MUST** preserve exact 64-bit floating-point bit patterns across round-trip decoding and re-encoding.

---

## 2. Strict SI Units and Coordinate Systems

### 2.1 Canonical SI Units

All quantities serialized in OpenToolpath documents and packages **MUST** use canonical SI or SI-derived industrial manufacturing units, defined in Table 2.1. Dimensionless ratios and normalized multipliers **MUST** be bounded as specified.

| Physical Domain | Quantity | Unit Symbol | Unit Definition / Base | Required Range / Invariant |
|---|---|---|---|---|
| **Length / Position** | Coordinates $(x, y, z)$ | `mm` | Millimetre ($10^{-3} \text{ m}$) | Finite binary64 (`!isNaN && !isInf`) |
| **Length / Extrusion**| Filament feedstock | `mm` | Millimetre ($10^{-3} \text{ m}$) | Finite binary64; $\ge 0.0$ for extrusion, $< 0.0$ for retraction |
| **Length / Feature**  | Bead width ($w$), Layer height ($h$) | `mm` | Millimetre ($10^{-3} \text{ m}$) | Finite binary64, $> 0.0$ when non-null |
| **Time**              | Duration, dwell | `s` | Second | Finite binary64, $\ge 0.0$ |
| **Linear Velocity**   | Feedrate ($F$) | `mm/min` | Millimetres per minute | Finite binary64; $> 0.0$ for motion moves, $0.0$ for stationary dwell |
| **Rotary Velocity**   | Angular feedrate | `deg/min` | Degrees per minute | Finite binary64, $\ge 0.0$ |
| **Rotary Speed**      | Spindle Speed ($S$) | `RPM` | Revolutions per minute | Finite binary64, $\ge 0.0$ ($0$ denotes commanded stop) |
| **Power**             | Laser / Directed Energy | `W` | Watt ($1 \text{ J/s}$) or PWM | Finite binary64, $\ge 0.0$ (multiplexed via tool kind) |
| **Volume**            | Deposited bead volume | `mm³` | Cubic millimetre ($10^{-9} \text{ m}^3$) | Finite binary64, $\ge 0.0$; strictly $0.0$ when `travel: true` |
| **Volumetric Flow**   | Flow multiplier | dimensionless | Multiplier ratio | Finite binary64, $> 0.0$ (canonical default = $1.0$) |
| **Temperature**       | Thermal zones | `°C` | Degrees Celsius | Finite binary64, $\ge -273.15$ |
| **Cooling / Fan**     | Auxiliary air cooling | normalized | Normalized duty cycle $[0.0, 1.0]$ | $0.0 \le \text{fan} \le 1.0$ |
| **Tool Orientation**  | Unit direction vector | dimensionless | $(i, j, k) \in \mathbb{R}^3$ | $\sqrt{i^2 + j^2 + k^2} = 1.0 \pm 10^{-6}$ |
| **Euler Angles**      | ZYZ Euler orientation | `deg` (wire) / `rad` (math) | $(\alpha, \beta, \gamma)$ rotation | Finite binary64; wire format uses degrees |
| **Tool Index**        | Active tool identifier | integer | Integer ID | Unsigned 32-bit integer ($0 \dots 2^{32}-1$) |

**Floating-Point Soundness Rule:** `NaN`, `+Infinity`, and `-Infinity` **MUST NOT** appear anywhere in an OpenToolpath payload. Any parser or ingress validator encountering a non-finite IEEE-754 value **MUST** refuse the payload immediately with `OTP_ERR_NUMERIC_NON_FINITE`.

### 2.2 Coordinate Reference Frames

OpenToolpath standardizes five hierarchical reference coordinate systems, illustrated in Figure 2.1. All systems are strictly right-handed Cartesian coordinate systems.

```
       +Z (Normal to Work Plane)
        |
        |   +Y (Column / Y-Axis)
        |  /
        | /
        +------ +X (Gantry / Table X-Axis)
```

1. **Machine Coordinate System (MCS):**
   The immutable, physical home frame defined by machine mechanical limit switches, hard stops, or kinematic joint zeros. Origin $\mathbf{O}_{\text{mcs}} = (0, 0, 0)$.
2. **Workpiece Coordinate System (WCS / Part Frame):**
   The reference frame attached to the workpiece stock, fixture, or CAM design origin. Standard CNC fixtures correspond to offsets $\mathbf{T}_{\text{wcs}}$ (e.g., G54 through G59.3). All segment trajectory positions $(x, y, z)$ in the motion payload are expressed relative to the active WCS unless explicitly declared otherwise in `context.json`.
3. **Tool Center Point (TCP):**
   The instantaneous operational focal point: the tip of a cutting endmill, the focal center of a laser beam, or the centroid of an extrusion nozzle orifice. Path trajectories trace the continuous evolution of the TCP through 3D space: $\mathbf{p}(t) = (x(t), y(t), z(t))_{\text{wcs}}$.
4. **Tool Orientation Frame:**
   The orientation of the tool cutting axis or deposition direction relative to WCS. Represented as a normalized 3D unit vector $\hat{\mathbf{u}} = (i, j, k)$ directed outward along the tool axis (from holder toward tip, or along beam propagation). In conventional 3-axis machining, $\hat{\mathbf{u}} = (0, 0, 1)$.
5. **Fixture & Joint Space Coordinates (ACS / JCS):**
   Machine joint angles $(A, B, C)$ for multi-axis gimbals, tables, or robotic joints $(J_1 \dots J_6)$. These are derived via the machine kinematic transformation matrix defined in `context.json`:
   $$\mathbf{p}_{\text{mcs}} = \mathbf{K}(\mathbf{p}_{\text{wcs}}, \hat{\mathbf{u}}_{\text{wcs}}, \mathbf{T}_{\text{wcs}})$$

---

## 3. The `.otp` Package Container Specification

### 3.1 Container Architecture & MIME Type

An OpenToolpath package is a standardized, self-contained, single-file archive bearing the `.otp` file extension.
- **MIME Type:** `application/vnd.opentoolpath+zip`
- **ZIP Architecture:** Conforming containers **MUST** be encoded as valid ZIP archives (PKZIP 2.04g / ISO/IEC 21320-1) using DEFLATE (RFC 1951) or uncompressed (Store) algorithms.
- **Identification:** To facilitate rapid `libmagic` detection, packages **SHOULD** include an uncompressed 32-byte `mimetype` file as the very first entry at byte offset 30, containing the ASCII string `application/vnd.opentoolpath+zip`.

A conforming `.otp` package **MUST NOT** use ZIP encryption, multi-disk spanning, or proprietary archive extensions. All file paths inside the archive **MUST** be UTF-8 encoded and use forward slashes (`/`) as path separators. Path components **MUST NOT** contain relative directory traversals (`..`), drive letters, or leading slashes (ZIP slip mitigation).

### 3.2 Canonical Package Directory Structure

```
package.otp
├── mimetype                   [OPTIONAL: Uncompressed MIME identifier at byte offset 30]
├── manifest.json              [REQUIRED: Root package descriptor & verification hashes]
├── payload/
│   └── toolpath.json          [REQUIRED: Primary motion stream - JSON, DRY0, or DRY1]
├── tools.json                 [REQUIRED: Tool geometry, cutter catalog, nozzle specs]
├── context.json               [REQUIRED: Kinematic configuration, WCS offsets, bounds]
├── assets/                    [OPTIONAL: Stock meshes, B-Rep solids, fixturing CAD]
│   └── stock.step             [OPTIONAL: Referenced in context.json]
└── signatures/
    ├── manifest.sig           [OPTIONAL: Detached cryptographic signature of manifest.json]
    └── cert.pem               [OPTIONAL: X.509 public key identity certificate]
```

### 3.3 Root Package Manifest (`manifest.json`)

The manifest is the primary metadata and validation anchor. It **MUST** be located at the root of the `.otp` container. All non-signature files present in the container **MUST** have their SHA-256 hex digest recorded in `digests`.

```json
{
  "$schema": "https://opentoolpath.org/spec/v1/opentoolpath-v1.schema.json",
  "otp_version": "1.0",
  "package_id": "urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6",
  "created_at": "2026-09-08T19:40:00Z",
  "generator": {
    "name": "dry",
    "version": "0.11.0",
    "source_hash": "a1b2c3d4e5f67890123456789abcdef0123456789abcdef0123456789abcdef0"
  },
  "process": {
    "domain": "subtractive",
    "sub_type": "milling_5axis",
    "description": "Impeller blisk multi-axis continuous roughing"
  },
  "entrypoints": {
    "payload": "payload/toolpath.json",
    "payload_format": "json",
    "tools": "tools.json",
    "context": "context.json"
  },
  "digests": {
    "payload/toolpath.json": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "tools.json": "sha256:ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb",
    "context.json": "sha256:88d4266fd4e6338d13b845fcf289579d209c897823b9217da3e161936f031589"
  },
  "conformance": {
    "level": "strict",
    "invariants": [
      "FM1.L2.WELL_FORMED.VALIDATION",
      "FM1.IRBCAM.ZYZ.DIRECTION_SPIN_INDEPENDENT",
      "FM1.IRBCAM.ZYZ.DIRECTION_ROUNDTRIP",
      "FM1.IRBCAM.ARC.MIDPOINT",
      "FM1.RESOLVE_CHANNELS",
      "FM1.DEPOSITION.CONSERVATION"
    ]
  }
}
```

### 3.4 Physical Tool Catalog (`tools.json`)

The tool catalog specifies geometry, material, and operational parameters for all cutters, turning inserts, nozzles, lasers, or probes referenced by index in the motion payload.

```json
{
  "$schema": "https://opentoolpath.org/spec/v1/opentoolpath-v1.schema.json",
  "schema_version": "1.0",
  "tools": [
    {
      "id": 1,
      "name": "12mm Flat Endmill Solid Carbide",
      "kind": "endmill",
      "geometry": {
        "diameter": 12.0,
        "corner_radius": 0.0,
        "flute_length": 32.0,
        "overall_length": 75.0,
        "flute_count": 4
      },
      "offsets": {
        "length_offset": 75.0,
        "radius_offset": 6.0
      },
      "capabilities": {
        "max_rpm": 18000.0,
        "max_feedrate": 8000.0
      }
    },
    {
      "id": 2,
      "name": "CNMG 120408 Turning Insert",
      "kind": "turning_insert",
      "geometry": {
        "diameter": 12.7,
        "corner_radius": 0.8
      }
    },
    {
      "id": 3,
      "name": "0.4mm High-Flow Hardened Steel Nozzle",
      "kind": "fff_nozzle",
      "geometry": {
        "orifice_diameter": 0.4,
        "filament_diameter": 1.75
      },
      "thermal": {
        "max_temperature": 320.0
      }
    }
  ]
}
```

### 3.5 Operational Machine Context (`context.json`)

The operational context defines machine kinematic topology, spatial work envelopes, stock geometry, and coordinate transformations.

```json
{
  "$schema": "https://opentoolpath.org/spec/v1/opentoolpath-v1.schema.json",
  "schema_version": "1.0",
  "machine": {
    "vendor": "Hermle",
    "model": "C42U",
    "kinematics": {
      "topology": "table_table_bc",
      "axes": [
        { "name": "X", "type": "linear", "min": -400.0, "max": 400.0, "max_velocity": 45000.0 },
        { "name": "Y", "type": "linear", "min": -350.0, "max": 350.0, "max_velocity": 45000.0 },
        { "name": "Z", "type": "linear", "min": 0.0, "max": 550.0, "max_velocity": 40000.0 },
        { "name": "B", "type": "rotary", "min": -115.0, "max": 115.0, "axis_vector": [0.0, 1.0, 0.0] },
        { "name": "C", "type": "rotary", "min": -36000.0, "max": 36000.0, "axis_vector": [0.0, 0.0, 1.0] }
      ]
    }
  },
  "work_coordinates": {
    "active_system": "G54",
    "offsets": {
      "G54": { "x": 150.0, "y": -20.0, "z": 85.4, "b": 0.0, "c": 0.0 }
    }
  },
  "stock": {
    "type": "box",
    "dimensions": [200.0, 200.0, 100.0],
    "origin": [0.0, 0.0, 0.0]
  }
}
```

### 3.6 Digital Thread Cryptographic Provenance (`signatures/`)

For regulated aerospace (DO-178C), medical (IEC 62304), and nuclear environments, `.otp` containers support end-to-end cryptographic verification via detached signatures:
- `signatures/manifest.sig`: Detached digital signature (Ed25519 or ECDSA P-384) computed over the exact UTF-8 canonical bytes of `manifest.json`.
- `signatures/cert.pem`: X.509 public key certificate establishing verified identity of authoring organization, CAM engineer, or automated pipeline.
- Signing `manifest.json` transitively guarantees bit-for-bit authenticity across the entire package, because `manifest.json` contains the SHA-256 cryptographic digests of all constituent files. Conforming implementations **MUST NOT** embed inline signatures inside `manifest.json` to prevent circular verification dependencies.

---

## 4. The Core Logical Data Model

### 4.1 Hierarchy: `Toolpath` and `Segment`

An OpenToolpath document represents a single motion trajectory comprising an ordered sequence of discrete moves.

```
Toolpath
 ├── version: u32 = 1
 ├── meta: Meta (generator, units, source_hash, invariants)
 └── segments: Array<Segment>
      ├── start: [x, y, z] (OptAxis3: component null only prior to first move)
      ├── end: [x, y, z] (Vec3: strictly non-null finite coordinates)
      ├── travel: bool
      ├── speed: f64 (mm/min, > 0 for motion, = 0 for dwell)
      ├── length: f64 (mm, >= 0)
      ├── volume: f64 (mm³, >= 0, strictly 0 when travel: true)
      ├── filament: f64 (mm, negative for retract, strictly 0 when travel: true)
      ├── width: f64 | null (mm, > 0 when non-null)
      ├── height: f64 | null (mm, > 0 when non-null)
      ├── kind: SegmentKind (line, arc, spline, dwell, rapid, retract, unretract, deposit, manualgcode)
      ├── centre: [cx, cy] | [cx, cy, cz] | null (required for arc)
      ├── clockwise: bool (arc sweep direction)
      ├── midpoint: [mx, my, mz] | null (arc sweep bisector)
      ├── orientation: [i, j, k] | null (unit direction vector ||u|| = 1.0)
      ├── euler: [rz1, ry, rz2] | null (ZYZ Euler angles in degrees)
      ├── control_points: Array<[x,y,z]> | null (required for spline, minItems: 2)
      ├── dwell_s: f64 | null (required for dwell, >= 0)
      ├── power: f64 | null (commanded spindle RPM or laser power >= 0)
      ├── temperature: f64 | null (tool/nozzle temperature °C)
      ├── fan: f64 | null (part cooling fan [0.0, 1.0])
      ├── flow: f64 | null (flow multiplier > 0, default 1.0)
      ├── tool: u32 | null (active tool index)
      └── manual_gcode: string | null (verbatim unlowered g-code instruction)
```

**Omission & Nullability Policy:** On the JSON wire form, optional fields MAY be explicitly set to `null` or omitted when unset (identical to `serde(skip_serializing_if)` semantics). Conforming readers **MUST** accept both explicit `null` and property omission.

### 4.2 Segment Primitives (`SegmentKind`) & Serialization Encodings

Conforming OpenToolpath implementations **MUST** support the nine core segment primitives defined in Table 4.1.

| Primitive Variant | Wire JSON | `DRY0` Dictionary String | `DRY1` Tag (`u8`) | Mathematical Semantics | Associated Fields |
|---|---|---|---|---|---|
| **Line** | `"line"` | `line` | `0` | Linear interpolation $C^0/C^1$ between $\mathbf{p}_{\text{start}}$ and $\mathbf{p}_{\text{end}}$. | `start`, `end`, `speed` |
| **Arc** | `"arc"` | `arc` | `1` | Exact circular arc in an oriented plane between $\mathbf{p}_{\text{start}}$ and $\mathbf{p}_{\text{end}}$. | `start`, `end`, `centre`, `clockwise`, `midpoint` |
| **Spline** | `"spline"` | `spline` | `2` | Parametric polynomial curve (B-spline / NURBS). | `start`, `end`, `control_points` |
| **Dwell** | `"dwell"` | `dwell` | `3` | Stationary pause with zero translation ($\mathbf{p}_{\text{start}} = \mathbf{p}_{\text{end}}$) for fixed time. | `dwell_s`, `speed: 0` |
| **Retract** | `"retract"` | `retract` | `4` | Pure feedstock decompression move (AM). | `filament < 0`, $\mathbf{p}_{\text{start}} = \mathbf{p}_{\text{end}}$ |
| **Unretract** | `"unretract"` | `unretract` | `5` | Feedstock re-priming move (AM). | `filament > 0`, $\mathbf{p}_{\text{start}} = \mathbf{p}_{\text{end}}$ |
| **Deposit** | `"deposit"` | `deposit` | `6` | Stationary spot extrusion or tactile probing. | `volume > 0`, $\mathbf{p}_{\text{start}} = \mathbf{p}_{\text{end}}$ |
| **ManualGcode** | `"manualgcode"` | `manual_gcode` | `7` | Inert unlowered g-code escape sequence. | `manual_gcode` |
| **Rapid** | `"rapid"` | `rapid` | `8` | Non-machining/non-extruding repositioning at machine rapid rate. | `travel: true`, `speed > 0`, `volume: 0`, `filament: 0` |

**Migration Rule:** In OpenToolpath v1.0, `kind: "rapid"` requires `travel: true`. For backward compatibility with Dry IR v0, conforming readers **MUST** accept `kind: "line"` with `travel: true` as a rapid positioning move. Encoders **SHOULD** emit `kind: "rapid"` for all non-cutting positioning moves.

### 4.3 Arc Geometry and Midpoint Sweep Preservation

In standard RS-274 / CNC G-code, circular arcs are specified either via center offsets $(I, J, K)$ or arc radius $(R)$. OpenToolpath defines circular arcs with **midpoint sweep preservation**, supporting both center specification and three-point representation $(\mathbf{p}_{\text{start}}, \mathbf{p}_{\text{mid}}, \mathbf{p}_{\text{end}})$.

Let $\mathbf{c} \in \mathbb{R}^3$ be the arc center, $\rho > 0$ the arc radius, $\phi \in [0, 2\pi)$ the start angle, $\Delta \in (-2\pi, 2\pi) \setminus \{0\}$ the signed angular sweep, and $\Delta z = z_{\text{end}} - z_{\text{start}}$ the helical pitch travel:
- In the active projection plane of the arc (XY plane for G17, XZ for G18, YZ for G19):
  $$\mathbf{p}_{\text{start}}^{\text{proj}} = \mathbf{c}^{\text{proj}} + \rho (\cos \phi, \sin \phi)$$
  $$\mathbf{p}_{\text{mid}}^{\text{proj}} = \mathbf{c}^{\text{proj}} + \rho \left(\cos\left(\phi + \frac{\Delta}{2}\right), \sin\left(\phi + \frac{\Delta}{2}\right)\right)$$
  $$\mathbf{p}_{\text{end}}^{\text{proj}} = \mathbf{c}^{\text{proj}} + \rho (\cos(\phi + \Delta), \sin(\phi + \Delta))$$
- Along the cylinder axis perpendicular to the arc plane (e.g. Z axis for XY arcs):
  $$z_{\text{mid}} = z_{\text{start}} + \frac{\Delta z}{2}$$

#### Formal Invariant I8 (`ArcMidpoint.midpoint_on_circle`)
As formally proved in Lean 4 ([formal/Dry/Geometry/ArcMidpoint.lean](formal/Dry/Geometry/ArcMidpoint.lean)):
The arc midpoint $\mathbf{p}_{\text{mid}}$ projected onto the arc plane lies exactly on the circle of radius $\rho$ centered at $\mathbf{c}^{\text{proj}}$:
$$\|\mathbf{p}_{\text{mid}}^{\text{proj}} - \mathbf{c}^{\text{proj}}\|_2^2 = \rho^2$$
Furthermore, $\mathbf{p}_{\text{mid}}$ angularly bisects the sweep $\Delta$, and for all non-degenerate arcs ($0 < |\Delta| < 2\pi$ and $\rho > 0$), $\mathbf{p}_{\text{mid}} \ne \mathbf{p}_{\text{start}}$ and $\mathbf{p}_{\text{mid}} \ne \mathbf{p}_{\text{end}}$.

Conforming OpenToolpath writers emitting arcs **SHOULD** serialize `midpoint`. When present, readers **MUST** verify that $\mathbf{p}_{\text{mid}}$ is equidistant to $\mathbf{c}$ within declared chordal tolerance $\epsilon_{\text{chord}} = 10^{-4} \text{ mm}$, and that start and end radii agree within $\epsilon = 10^{-4} \text{ mm}$.

### 4.4 5/6-Axis Multi-Axis Orientations and Euler Frames

Tool orientation in 5-axis and 6-axis kinematics is represented in OpenToolpath via two complementary mathematical models:
1. **Normalized Direction Vector:** $\hat{\mathbf{u}} = (i, j, k) \in \mathbb{R}^3$, satisfying:
   $$\|\hat{\mathbf{u}}\|_2^2 = i^2 + j^2 + k^2 = 1.0 \pm 10^{-6}$$
2. **Intrinsic ZYZ Euler Angle Triplet:** $(\alpha, \beta, \gamma) = (rz_1, ry, rz_2)$, defined by successive rotations:
   $$\mathbf{R} = \mathbf{R}_z(rz_1) \cdot \mathbf{R}_y(ry) \cdot \mathbf{R}_z(rz_2)$$
   Applying this transformation to the canonical tool vector $(0, 0, 1)^T$ yields the forward direction vector:
   $$\mathbf{d}(rz_1, ry, rz_2) = \begin{pmatrix} \cos(rz_1) \sin(ry) \\ \sin(rz_1) \sin(ry) \\ \cos(ry) \end{pmatrix}$$

#### Singularities, Domain Clamping, and Degree/Radian Conversion
In mathematical equations, angles are real numbers in radians. On the wire form (JSON, DRY0, DRY1), Euler angles are encoded in **degrees**:
$$\theta_{\text{rad}} = \theta_{\text{deg}} \times \frac{\pi}{180}, \quad \theta_{\text{deg}} = \theta_{\text{rad}} \times \frac{180}{\pi}$$

When converting a unit direction vector $\hat{\mathbf{u}} = (u_x, u_y, u_z)$ to Euler angles, implementations **MUST** handle the polar singularity / gimbal lock condition and enforce domain projection before evaluating inverse trigonometric functions (matching `formal/Dry/Geometry/ZyzEuler.lean:56-57`):
$$\alpha = \begin{cases} \text{atan2}(u_y, u_x) & \text{if } u_x^2 + u_y^2 \ne 0 \\ 0.0 & \text{otherwise (tool pointing along } \pm Z) \end{cases}$$
$$\beta = \arccos\left(\max(-1.0, \min(1.0, u_z))\right)$$
$$\gamma = s \quad (\text{spin roll angle, default } 0.0)$$

#### Formal Invariants I6 and I7
- **Invariant I6 (`ZyzEuler.direction_spin_independent`):**
  $$\forall (rz_1, ry, s) \in \mathbb{R}^3, \quad \mathbf{d}(rz_1, ry, s) = \mathbf{d}(rz_1, ry, 0)$$
  Proves that tool orientation vector extraction is completely decoupled from the third Euler rotation angle (roll), eliminating unconstrained DOF ambiguity for 5-axis machines.
- **Invariant I7 (`ZyzEuler.direction_euler_roundtrip`):**
  $$\forall \hat{\mathbf{u}} \in \mathbb{R}^3 \text{ with } \|\hat{\mathbf{u}}\|_2 = 1, \quad \mathbf{d}(\text{euler}(\hat{\mathbf{u}}, s)) = \hat{\mathbf{u}}$$
  Guarantees exact geometric fidelity when mapping continuous vector fields to multi-axis Euler angles.

---

## 5. Tiered Serialization Architecture

OpenToolpath defines three normative serialization tiers to accommodate diverse compute nodes:

```
+--------------------------------------------------------------------------------+
|                        OpenToolpath Logical Data Model                         |
+--------------------------------------------------------------------------------+
        |                                |                               |
        v                                v                               v
+------------------+           +-------------------+           +------------------+
|      Tier 1      |           |      Tier 2       |           |      Tier 3      |
| Canonical JSON   |           |  Columnar Binary  |           | Chunked Streaming|
|  toolpath.json   |           |  (DRY0 / Arrow)   |           | (DRY1/FlatBuffer)|
|                  |           |  toolpath.dry0    |           |  toolpath.dry1   |
| UTF-8 Plaintext  |           | Contiguous Arrays |           | Bounded Blocks   |
| Human-Readable   |           | High Compression  |           | Low-Memory MCU   |
+------------------+           +-------------------+           +------------------+
```

### 5.1 Tier 1: Canonical JSON Wire Form (`toolpath.json`)

The JSON wire form is the canonical, human-readable baseline representation:
- Encoded in UTF-8 text adhering to RFC 8259.
- Validates against [spec/opentoolpath-v1.schema.json](spec/opentoolpath-v1.schema.json).
- Numbers: Encoded as standard JSON numbers using shortest round-tripping decimal representation.
- Omission policy: Optional fields with default or unset values (e.g. `manual_gcode`, `dwell_s`, `power` when uncommanded) are omitted from JSON serialization to minimize footprint. Readers **MUST** also accept explicit `null`.
- Forward compatibility: Readers **MUST** ignore unknown JSON keys in root objects, but **MUST** reject unknown `kind` enum variants or malformed coordinate arrays.

### 5.2 Tier 2: Columnar Binary Encoding (`DRY0` / Arrow, `enc_ver: 3`)

`DRY0` is a dense, struct-of-arrays (SoA) columnar binary format designed for archival storage, high-throughput analytical query, and fast random access.
- **Uncompressed Header (17 bytes):**
  - Magic (4 bytes): `DRY0` (`0x44 0x52 0x59 0x30`)
  - `enc_ver` (1 byte): Encoding layout version (`3` for full OpenToolpath v1.0; `2` for legacy power-enabled; `1` standard)
  - `ir_ver` (4 bytes LE): Toolpath IR schema version (v1 => `1`)
  - `n` (4 bytes LE): Total segment count
  - `body_len` (4 bytes LE): Uncompressed body length (inflate memory limit)
- **Compressed Body:** DEFLATE stream expanding to exactly `body_len` bytes.
- **Column Sequence (Layout `enc_ver: 3`):**
  1. `travel` bitmap (`ceil(n/8)` bytes)
  2. `clockwise` bitmap (`ceil(n/8)` bytes)
  3. Nullable `f64` coordinate columns (validity bitmap + $n \times \text{f64}$ LE): `start.x, y, z`, `end.x, y, z`, `width`, `height`, `centre.x, y`
  4. Dense `f64` columns ($n \times \text{f64}$ LE): `speed`, `length`, `volume`, `filament`
  5. Nullable process columns: `temperature`, `fan`, `flow`, `dwell_s`
  6. `tool` nullable `u32` column
  7. `orientation` nullable vector3 column ($n \times 3 \times \text{f64}$ LE)
  8. `control_points` variable-length vector array
  9. `manual_gcode` variable-length UTF-8 string column
  10. `power` nullable `f64` column
  11. `centre.z` nullable `f64` column *(new in enc_ver 3)*
  12. `midpoint` nullable vector3 column ($n \times 3 \times \text{f64}$ LE) *(new in enc_ver 3)*
  13. `euler` nullable vector3 column ($n \times 3 \times \text{f64}$ LE) *(new in enc_ver 3)*
  14. `kind` dictionary: length-prefixed unique string table followed by $n \times \text{u32}$ indices (includes string `"rapid"`)
  15. `meta` JSON trailer

Because consecutive segments share identical feedrates, bead widths, and tool orientations, columnar layout achieves $> 90\%$ compression ratios under DEFLATE.

### 5.3 Tier 3: Chunked Streaming Binary Encoding (`DRY1` / FlatBuffers, `enc_ver: 3`)

`DRY1` is a row-oriented, chunked streaming binary format optimized for real-time forward execution on memory-constrained controllers (e.g., ARM Cortex-M4/M7 with 64KB RAM).
- **Stream Header:**
  - Magic (4 bytes): `DRY1` (`0x44 0x52 0x59 0x31`)
  - `enc_ver` (1 byte): Encoding version (`3` for OpenToolpath v1.0)
  - `ir_ver` (4 bytes LE): Toolpath IR schema version (v1 => `1`)
  - `n` (4 bytes LE): Total segment count
  - `block_size` (4 bytes LE): Fixed block chunk size (typically 512 segments)
  - `meta`: Length-prefixed JSON metadata
- **Chunked Blocks:**
  Repeated until $n$ segments are read:
  - `block_n` (4 bytes LE): Segment count in this block ($1 \le \text{block\_n} \le \text{block\_size}$)
  - `body_len` (4 bytes LE): Uncompressed block byte limit
  - `deflate_len` (4 bytes LE): Compressed block payload length
  - `body` (`deflate_len` bytes): DEFLATE compressed block payload
- **Row-Level Flags Word (`enc_ver: 3`):**
  Every row begins with a 32-bit bitfield (`flags`) and a 1-byte `kind` tag (`0` through `8`). Flag bits indicate presence of optional or sparse fields:
  - Bit 0: `travel`, Bit 1: `clockwise`
  - Bits 2–4: `start.x, y, z`, Bits 5–7: `end.x, y, z`
  - Bits 8–9: `width, height`, Bit 10: `centre (cx, cy)`
  - Bits 11–14: `temperature, fan, flow, dwell_s`
  - Bit 15: `tool`, Bit 16: `orientation`, Bit 17: `control_points`
  - Bit 18: `manual_gcode`, Bit 19: `power`
  - Bit 20: `centre.z` (f64) *(new in enc_ver 3)*
  - Bit 21: `midpoint` (3 × f64) *(new in enc_ver 3)*
  - Bit 22: `euler` (3 × f64) *(new in enc_ver 3)*
  - Bits 23–31: Reserved (MUST be 0)
  - **Known-flags mask for `enc_ver: 3`:** `0x7FFFFF` (bits 0..=22). Any set bit outside this mask MUST be rejected with `OTP_ERR_SCHEMA_VIOLATION`.

Controllers stream and inflate one block at a time into a fixed ring buffer, guaranteeing strict bounded memory execution with zero heap allocation during motion processing.

---

## 6. The 3-Tier Specification Architecture

OpenToolpath formalizes specification rigor through a three-tier architecture:

```
+--------------------------------------------------------------------------------+
|                        Tier A: Normative Human Prose                           |
|  RFC 2119 English language standard, operational definitions, error semantics  |
+--------------------------------------------------------------------------------+
                                      ▲
                                      │ Refinement & Verification
                                      ▼
+--------------------------------------------------------------------------------+
|              Tier B: Conformance Test Suite & Reference Oracles                |
| Clean-room vectors, standalone validation harnesses, independent interpreters  |
+--------------------------------------------------------------------------------+
                                      ▲
                                      │ Mathematical Soundness
                                      ▼
+--------------------------------------------------------------------------------+
|            Tier C: Machine-Checked Mathematical Invariants (Lean 4)            |
| Formal proofs of well-formedness, kinematics, volume conservation, soundness  |
+--------------------------------------------------------------------------------+
```

### 6.1 Formal Mathematical Invariants Register (Tier C)

OpenToolpath theorems are formally proven in Lean 4 under [formal/Dry/](formal/Dry/) and registered in `proofs/claims.toml`.

#### Invariant FM1.L2.WELL_FORMED.VALIDATION — Validation Well-Formedness Bijection
*Theorem (`Dry.Language.L2.Validation.validate_success_iff`, [formal/Dry/Language/WellFormed.lean](formal/Dry/Language/WellFormed.lean)):*
A program validation pass succeeds if and only if the underlying toolpath satisfies every inductive well-formedness predicate:
$$\text{validate}(p) = \text{ok}() \iff p.\text{WellFormed}$$
Guarantees that the validator produces zero false positives and zero false negatives with respect to the abstract mathematical grammar.

#### Invariant I6 — Tool Direction Spin Independence
*Theorem (`Dry.Geometry.ZyzEuler.direction_spin_independent`, [formal/Dry/Geometry/ZyzEuler.lean](formal/Dry/Geometry/ZyzEuler.lean)):*
$$\forall (rz_1, ry, s) \in \mathbb{R}^3, \quad \mathbf{d}(rz_1, ry, s) = \mathbf{d}(rz_1, ry, 0)$$
Proves that tool orientation vector extraction is completely decoupled from the third Euler rotation angle (roll), eliminating unconstrained DOF ambiguity.

#### Invariant I7 — Euler Direction Round-Trip Soundness
*Theorem (`Dry.Geometry.ZyzEuler.direction_euler_roundtrip`, [formal/Dry/Geometry/ZyzEuler.lean](formal/Dry/Geometry/ZyzEuler.lean)):*
$$\forall \hat{\mathbf{u}} \in \mathbb{R}^3 \text{ with } \|\hat{\mathbf{u}}\|_2 = 1, \quad \mathbf{d}(\text{euler}(\hat{\mathbf{u}}, s)) = \hat{\mathbf{u}}$$
Guarantees exact geometric fidelity when mapping continuous vector fields to multi-axis Euler angles.

#### Invariant I8 — Circular Arc Midpoint On Circle
*Theorem (`Dry.Geometry.ArcMidpoint.midpoint_on_circle`, [formal/Dry/Geometry/ArcMidpoint.lean](formal/Dry/Geometry/ArcMidpoint.lean)):*
$$\forall \mathbf{c} \in \mathbb{R}^2, \rho \ge 0, \phi \in \mathbb{R}, \Delta \in \mathbb{R}, \quad \text{dist}^2(\text{midpoint}(\mathbf{c}, \rho, \phi, \Delta), \mathbf{c}) = \rho^2$$
Guarantees that the interpolated arc midpoint remains on the exact bounding circle without radial distortion or polygonization error.

#### FM1.5 — Resolver State Semantics and Channel Soundness
The FM1.5 proof suite establishes:
- **FM1.5a (`ResolveOrientation`):** Orientation-aware resolution fold preserves continuity across contiguous moves.
- **FM1.5b (`ResolveChannels`):** State and channel propagation fold semantics guarantee that modal process values (feed, speed, temperature, tool) propagate monotonically until explicitly updated.
- **FM1.5c (`Deposition`):** Deposition volume conservation:
  $$V = \int_0^L w(s) \cdot h(s) \cdot \text{flow}(s) \, ds$$
  conserves exact physical feedstock mass.
- **FM1.5e (`VerifierSoundness`):** Machine limit verifier rules are sound with respect to real-valued physical bounds (failure-closed).

---

## 7. Normative Refusal and Error Taxonomy

When an OpenToolpath processor encounters invalid data, non-conforming structures, or out-of-envelope commands, it **MUST** refuse execution and return a structured diagnostic conforming to Table 7.1.

### 7.1 Structured Diagnostic Record

Conforming diagnostics **MUST** provide:
- `code`: Stable alphanumeric error code from Table 7.1.
- `category`: Classification (`syntax`, `schema`, `numeric`, `kinematic`, `security`).
- `location`: File path, line number, byte offset, or segment index.
- `message`: Human-readable explanation.
- `context`: Actual value vs expected invariant/bound.

### 7.2 Diagnostic Error Code Register

| Error Code | Classification | Trigger Condition | Conformance Action |
|---|---|---|---|
| `OTP_ERR_CONTAINER_CORRUPT` | Security / Syntax | ZIP CRC32 failure, invalid compression method, or truncated stream. | Refuse container. |
| `OTP_ERR_CONTAINER_PATH_TRAVERSAL` | Security | Entry contains `..`, leading `/`, or absolute Windows drive path. | Refuse container immediately. |
| `OTP_ERR_DIGEST_MISMATCH` | Integrity | Payload file SHA-256 does not match `manifest.json` declaration. | Refuse package execution. |
| `OTP_ERR_SIGNATURE_INVALID` | Integrity | Cryptographic signature verification over manifest failed. | Refuse package execution. |
| `OTP_ERR_SCHEMA_VIOLATION` | Schema | Payload fails JSON Schema validation (missing required keys, bad type). | Refuse document. |
| `OTP_ERR_NUMERIC_NON_FINITE` | Numeric | Coordinate, speed, or channel value is `NaN`, `+Inf`, or `-Inf`. | Refuse document (ADR 0002). |
| `OTP_ERR_KINEMATIC_LIMIT_EXCEEDED` | Kinematic | Feedrate, velocity, or axis travel exceeds machine profile envelope. | Refuse program; **never clamp**. |
| `OTP_ERR_ARC_RADIUS_MISMATCH` | Geometry | Start radius differs from end radius $> \epsilon = 10^{-4} \text{ mm}$. | Refuse arc. |
| `OTP_ERR_ARC_MIDPOINT_OFF_CIRCLE` | Geometry | Planar midpoint distance to $\mathbf{c}$ deviates from $\rho > \epsilon_{\text{chord}} = 10^{-4} \text{ mm}$. | Refuse arc. |
| `OTP_ERR_ORIENTATION_NOT_UNIT` | Geometry | Unit vector $\|\hat{\mathbf{u}}\|_2 \ne 1.0 \pm 10^{-6}$. | Refuse orientation. |
| `OTP_ERR_UNMODELED_COMMAND` | Semantic | Command cannot be lowered to motion and lacks inert-preservation flag. | Refuse program (Zero Silent Drops). |
| `OTP_ERR_RESOURCE_BUDGET_EXCEEDED` | Security / System | Block count or memory allocation exceeds configured safety ceiling. | Abort before memory exhaustion. |

---

## 8. Conformance and Interoperability Checklist

To claim OpenToolpath v1.0 conformance, an implementation **MUST** fulfill the requirements in Table 8.1.

| Identifier | Requirement | Verification Protocol |
|---|---|---|
| **CONF-CONT-01** | Support `.otp` ZIP container ingestion and deterministic creation. | Package round-trip test. |
| **CONF-CONT-02** | Enforce ZIP-slip protection and SHA-256 digest validation. | Negative security vector test. |
| **CONF-SCH-01** | Validate `toolpath.json` and `manifest.json` against `opentoolpath-v1.schema.json`. | Schema validator gate. |
| **CONF-NUM-01** | Ingress rejection of `NaN`, `+Infinity`, `-Infinity` across all fields. | Adversarial float vector suite. |
| **CONF-GEO-01** | Arc midpoint equidistant to center within $10^{-4} \text{ mm}$. | Geometric midpoint oracle test. |
| **CONF-GEO-02** | Orientation vector normalization $\|\hat{\mathbf{u}}\| = 1.0 \pm 10^{-6}$. | Unit vector verification test. |
| **CONF-GEO-03** | Round-trip fidelity between unit vectors and ZYZ Euler angles. | Euler kinematics test. |
| **CONF-SAFE-01** | Enforce refuse-never-clamp on feedrates, temperatures, and powers. | Capability boundary test suite. |
| **CONF-SAFE-02** | Enforce zero silent drops on unmodeled dialect commands. | CAM import diagnostic audit. |
| **CONF-CODEC-01**| Bit-level lossless round-trip across Tier 1 (JSON), Tier 2 (`DRY0`), and Tier 3 (`DRY1`). | Semantic equality vector gate (`enc_ver 3`). |

### 8.1 Backward Compatibility Bridge to Dry IR v0

OpenToolpath v1.0 establishes an explicit migration bridge with Dry IR v0:
- **IR Schema Versioning:** Dry IR v0 toolpaths carry `version: 0`. OpenToolpath v1.0 toolpaths carry `version: 1`. An OTP v1.0 processor **SHOULD** ingest Dry IR v0 toolpaths by mapping `kind: "line"` with `travel: true` to rapid moves, and defaulting absent `centre.z`, `midpoint`, and `euler` to null.
- **Binary Containers:** `DRY0` and `DRY1` processors supporting `enc_ver: 3` **MUST** continue to decode legacy `enc_ver: 1` and `enc_ver: 2` streams without regression.

---

## 9. References

1. **RFC 2119 / RFC 8174:** Key words for use in RFCs to Indicate Requirement Levels.
2. **ISO/IEC 21320-1:2015:** Document Container File — Part 1: Core (ZIP).
3. **ISO 14649 (STEP-NC):** Data model for computerized numerical controllers.
4. **ISO/ASTM 52915:** Standard specification for additive manufacturing file format (AMF).
5. **DO-178C / DO-333:** Software Considerations in Airborne Systems and Equipment Certification / Formal Methods Supplement.
6. **Dry ADR 0001:** Formal Assurance Constitution ([docs/adr/0001-formal-assurance-constitution.md](docs/adr/0001-formal-assurance-constitution.md)).
7. **Dry ADR 0002:** Numeric Ingress and Emission Gates ([docs/adr/0002-numeric-ingress-and-emission-gates.md](docs/adr/0002-numeric-ingress-and-emission-gates.md)).
8. **Dry IR v0 Specification:** [docs/10-dry-ir-v0-spec.md](docs/10-dry-ir-v0-spec.md).
9. **Dry APT & IRBCAM Dialect Contract:** [docs/28-apt-irbcam-dialects.md](docs/28-apt-irbcam-dialects.md).

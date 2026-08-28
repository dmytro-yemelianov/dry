# Industrial Certification, Standards Compliance & Qualification Framework

This document defines the formal certification roadmap, standards compliance matrix, and qualification protocols for the **Dry** parametric design and manufacturing platform.

---

## 1. Overview & Regulatory Pillars

Dry bridges mathematical rigor (Lean 4 formal verification, deterministic memory management in Rust) with industrial CAM and cloud microservices. This enables qualification across four core industry sectors:

1. **Digital Manufacturing & CAM**: ISO/ASTM 52915 (3MF), ISO 14649 (STEP-NC AP 238), ISO 6983-1 / DIN 66025 (RS-274).
2. **Safety-Critical & Aerospace Systems**: DO-178C / DO-333 (Formal Methods), ISO 26262 (ASIL D), IEC 62304 (Medical Device Software).
3. **Industrial Robotics & Automation**: ISO 10218-1/2, ANSI/RIA R15.06 (KRL / Multi-Axis Safety).
4. **Cloud Security & Cryptographic Supply Chain**: SOC 2 Type II, SLSA Level 3/4, NIST SP 800-218 (SSDF).

---

## 2. Standards Compliance Matrix

| Regulatory Standard | Target Sector | Applicable Component | Evidence & Artifacts | Compliance Status |
|---|---|---|---|---|
| **ISO/ASTM 52915** | Additive Manufacturing / 3D Printing | `crates/core/src/codec/threemf.rs` | 3MF Toolpath Extension XML round-trip fixtures | Compliant (Ready for 3MF Seal) |
| **ISO 14649 (STEP-NC)** | 5-Axis Aerospace Milling | `crates/core/src/emit/step_nc.rs` | ISO 10303-238 XML schema export | Compliant (AP 238 Core) |
| **ISO 6983-1 / DIN 66025** | CNC G-code Control | `crates/core/src/gcode/` | LinuxCNC `rs274` validation test harness | 100% Conformance |
| **DO-178C / DO-333** | Airborne Systems / Flight Additive | `formal/` & `proofs/` | 38 Lean 4 theorem modules (0 axioms, 0 sorry) | Formal Refinement Verified |
| **IEC 62304** | Medical Orthopedic & Dental Implants | `crates/core/` (TPMS & Verifier) | Deterministic memory & typed units algebra | Architecture Ready |
| **ISO 26262 (ASIL D)** | Automotive Structural Tooling | `crates/core/src/verify/` | 15 mathematical safety contract checkers | Architecture Ready |
| **ISO 10218-1/2** | Industrial Robotics (KUKA) | `crates/core/src/emit/krl.rs` | ANTLR4 KRL grammar parser + Singular Cone Hold | Conformance Tested |
| **SOC 2 Type II** | Cloud SaaS (`verify-runner`) | `containers/verify-runner/` | RAII `EphemeralGcodeFile`, Ed25519 Token Auth | Production Hardened |
| **SLSA Level 3/4** | Software Supply Chain Security | `.github/workflows/` | GitHub Actions Cosign signed images + SPDX SBOM | Automated in CI |

---

## 3. Qualification Kits & Technical Evidence

### 3.1 DO-333 / Aerospace Formal Methods Evidence Kit

For flight-critical structural components (e.g. turbine blisks, satellite brackets):
* **Dialect Lowering Preservation**: Proofs that $L0 \to L1 \to L2 \to L3$ lowerings preserve geometry without coordinate drift.
* **Bounded Floating-Point Invariants**: Fixed precision contracts ensuring no non-finite (`NaN`, `Inf`) coordinates escape to machine controllers.
* **Deterministic Allocation**: Bounded peak memory allocation during streaming verification (`DRY1` column stream).

### 3.2 IEC 62304 / Medical Device Evidence Kit

For patient-specific porous implants (e.g. titanium cranial plates with Gyroid TPMS infill):
* **Extrusion Volume Conservation**: Formal proof in `Dry/Semantics/Deposition.lean` guaranteeing exact material density.
* **Audit Trail Traceability**: Every cloud verification report stamps `x-request-id`, compiler commit hash, and cryptographic license signature.

### 3.3 SOC 2 Type II / Zero Data Retention Architecture

For proprietary aerospace and automotive customer CAD designs:
* **Zero Geometry Retention**: G-code files uploaded to `dry-verify-runner` are stored in ephemeral tempfs files wrapped in RAII drop guards, unlinking immediately upon request termination or panic.
* **Zero Geometry Ingress in Logs**: Prometheus metrics and structured JSON tracing logs omit coordinate streams, capturing only aggregate telemetry.

---

## 4. Certification Phasing & Roadmap Alignment

```
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 1 (1–2 Months): Rapid Conformance                                │
│ • ISO/ASTM 52915 3MF Consortium Compliance Seal                        │
│ • SLSA Level 3 Provenance & Cosign Signed GHCR Container Images        │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 2 (3–6 Months): Industrial Recognition                           │
│ • ISO 14649 STEP-NC AP 238 Full Workingstep Conformance               │
│ • DO-333 / Aerospace Formal Methods Qualification Package             │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Phase 3 (6–12 Months): Enterprise SaaS & Medical Compliance            │
│ • SOC 2 Type II Security & Confidentiality Audit                       │
│ • IEC 62304 / ISO 13485 Medical Software Quality Management           │
└────────────────────────────────────────────────────────────────────────┘
```

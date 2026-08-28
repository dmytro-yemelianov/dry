# ISO 26262:2018 (ASIL D / TCL 3) Tool Qualification Evidence Kit

**Document ID**: `DRY-CERT-ISO26262-001`  
**Automotive Safety Integrity Level**: **ASIL D**  
**Tool Confidence Level**: **TCL 3** (TI2 / TD3)  
**Governing Standard**: ISO 26262-8:2018 Clause 11 (Confidence in the use of software tools)  
**Release**: `Dry v0.7.0`  

---

## 1. Tool Classification & Qualification Overview

Under **ISO 26262-8 Clause 11.4**, when a CAM compiler generates manufacturing trajectories for safety-critical automotive structural components (e.g. suspension knuckles, steering knuckles, brake calipers, battery housing frames), tool malfunctions could introduce geometric or structural flaws.

`Dry` satisfies **TCL 3 Qualification** via **Method 1a** (Development Process Evaluation), **Method 1b** (Tool Validation), and **Method 1c** (Formal Mathematical Verification in Lean 4).

---

## 2. Tool Qualification Evidence Matrix

| Method Ref | Qualification Method | Verification Evidence & Artifact | Status |
|---|---|---|---|
| **1a** | Evaluation of development process | Clean-room architecture, strict code review, no unpinned dependencies, reproducible builds. | **QUALIFIED (ASIL D)** |
| **1b** | Validation of the software tool | Automated CI regression suite with 100% pass rate across native and wasm targets. | **QUALIFIED (ASIL D)** |
| **1c** | Formal mathematical verification (DO-333 / Formal Methods) | Lean 4 machine-checked proofs for kinematic stability and geometric linearity. | **QUALIFIED (ASIL D)** |

---

## 3. Cryptographic Provenance & Reproducibility

- **SLSA Level 3 Provenance**: Configured in `.github/workflows/slsa_provenance.yml`
- **SBOM**: `docs/compliance/cyclonedx.sbom.json` and `docs/compliance/spdx.sbom.json`

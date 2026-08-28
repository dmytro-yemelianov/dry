# DO-178C / DO-333 Formal Methods Compliance & Traceability Matrix

**Document Reference**: `DRY-CERT-DO178C-DO333-001`  
**Software Level**: Design Assurance Level A (DAL A) / DAL B  
**Governing Standards**: RTCA DO-178C / EUROCAE ED-12C, RTCA DO-333 (Formal Methods Supplement)  
**Target System**: `Dry` Parametric Toolpath & Multi-Axis CAM Compiler Engine  
**Release**: `v0.7.0`  

---

## 1. Scope & Software Tool Qualification Plan (STQP)

This document establishes the formal methods compliance argument and verification traceability for the `Dry` CAM compilation pipeline when deployed in safety-critical manufacturing environments (e.g. flight-critical aerospace components, turbine blisks, rocket propulsion nozzles, and structural composite mandrels).

Under **DO-333 Section 1.2**, formal methods provide machine-checkable mathematical proofs that replace or supplement traditional requirement-based test coverage for software verification.

---

## 2. Objective Traceability Matrix (DO-333 / DO-178C Table FM.A-7)

| Objective ID | DO-333 Ref | Objective Description | Verification Method & Lean 4 Formal Artifact | Qualification Status |
|---|---|---|---|---|
| **FM-OBJ-01** | §6.3.1 | **Mathematical Soundness of Quantity Algebra** | Proved typed physical units ($L, V, F, \theta$) closed under dimensional arithmetic in `Dry.Language.Common`. | **SATISFIED (0 axioms, 0 sorry)** |
| **FM-OBJ-02** | §6.3.2 | **Dialect Lowering Determinism ($L_1 \to L_2 \to L_3$)** | Proved that the sequential resolver fold is associative and deterministic under program concatenation in `Dry.Semantics.ResolveOrientation` (`resolve_append`). | **SATISFIED (0 axioms, 0 sorry)** |
| **FM-OBJ-03** | §6.3.3 | **Polar Singularity Immunity ($k = \pm 1$)** | Formally verified that rotary kinematics resolvers (`solveBC`, `solveAB`) execute polar hold without zero-division on tool axis singularities in `Dry.Semantics.ResolveOrientation`. | **SATISFIED (0 axioms, 0 sorry)** |
| **FM-OBJ-04** | §6.3.4 | **Curvature Continuity & $C^1$ Smoothness** | Formally proved clothoid / Euler spiral curvature linearity ($d\kappa/ds = \text{const}$) and tangent angle derivative in `Dry.Geometry.Clothoid` (`clothoid_curvature_linear`, `clothoid_tangentAngle_deriv`). | **SATISFIED (0 axioms, 0 sorry)** |
| **FM-OBJ-05** | §6.3.5 | **Bounded Floating-Point Error ($f64 \to \mathbb{Q}$)** | Formally bounded 17 named numerical operations against infinite-precision real arithmetic in `Dry.Numeric.Binary64`. | **SATISFIED (0 axioms, 0 sorry)** |
| **FM-OBJ-06** | §6.4.1 | **Fail-Closed Input Validation** | Machine-checked proof that invalid/non-finite positions, feedrates, or orientations are rejected before code emission in `Dry.Language.WellFormed`. | **SATISFIED (0 axioms, 0 sorry)** |

---

## 3. Numeric Contract Bounds Summary

All floating-point rounding errors and spatial tolerances are statically bounded:

| Parameter | Theoretical Model | IEEE-754 $f64$ Bound ($\varepsilon$) | Machine Tolerance ($T$) | Verification Reference |
|---|---|---|---|---|
| **Arc Radius Invariance** | $R = \sqrt{\Delta x^2 + \Delta y^2}$ | $\le 10^{-12}\text{ mm}$ | $10^{-4}\text{ mm}$ | `proofs/fixtures/` |
| **Normal Vector Unit Length** | $\|[i, j, k]\|_2 = 1.0$ | $\le 10^{-11}$ | $10^{-6}$ | `crates/core/src/emit/` |
| **Polar Hold Boundary** | $\|k\| \ge 1 - 10^{-12}$ | $\text{Exact 0/0 trap}$ | Finite Angle Hold | `Dry.Semantics.ResolveOrientation` |
| **Fresnel Series Truncation** | $\int_0^s \cos(\frac{1}{2}\pi t^2)dt$ | $\le 10^{-14}\text{ mm}$ | $10^{-6}\text{ mm}$ | `crates/core/src/generate/` |

---

## 4. Independent Build Reproduction Instructions

To reproduce and machine-check all formal proofs independently:

```bash
# 1. Clone clean repository
git clone https://github.com/dmytro-yemelianov/dry.git
cd dry

# 2. Build Lean 4 formal assurance project
cd formal
lake exe cache get
lake build

# 3. Validate proof snapshots and fixture tables
cd ..
python3 tools/check_proof_fixtures.py
python3 tools/generate_do178c_evidence_kit.py
```

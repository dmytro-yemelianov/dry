# `dry-core` — The Deterministic Toolpath Compiler Engine

[![License: FSL-1.1-MIT](https://img.shields.io/badge/License-FSL--1.1--MIT-blue.svg)](../../LICENSE)
[![Rust: 1.88+](https://img.shields.io/badge/rust-1.88+-blue.svg)](https://www.rust-lang.org/)

`dry-core` is the dependency-light Rust kernel that powers Dry. It implements the multi-level Dry IR (L0 $\rightarrow$ L1 $\rightarrow$ L2 $\rightarrow$ L3), formal numeric contracts, physics simulation, kinematics, optimization, and multi-dialect emission.

---

## 1. Architectural Lowering Pipeline

```
L0 Feature Dialect (Parametric repeat, transform, TPMS fields, B-Rep slicing)
       │
       ▼ [expand / generate]
L1 Path Dialect (Ordered ops: Move, Arc, Spline, Clothoid, Process Channels)
       │
       ▼ [resolve / kinematics]
L2 Toolpath Motion Dialect (Discrete 5-axis/6-axis spatial trajectory segments)
       │
       ▼ [optimize / verify]
L3 Target Code (G-code [Marlin/Klipper/GRBL/RS274], KUKA KRL, ABB RAPID, STEP-NC)
```

---

## 2. Core Modules

| Module | Scope & Responsibility |
|---|---|
| `ir/` | AST definitions for L0, L1, L2, L3 dialects and typed units (`Length`, `Speed`, `Volume`, `Flow`, `Time`, `Angle`). |
| `resolve/` | Deterministic resolution of L1 ops into L2 spatial toolpaths with material accumulation state. |
| `simulate.rs` | Cycle-accurate kinematics and physical extrusion/machining metrics calculation. |
| `verify/` | Safety contract enforcement, bounding volume verification, acceleration bounds, and tool holder collision detection. |
| `optimize/` | S-curve 7-phase trajectory profiling, Euler clothoid corner transitions, collinear segment merging, and chip thinning compensation. |
| `generate/` | High-level CAM generators: TPMS minimal surfaces, CNC pocketing, stepped pockets, lathe facing/turning, and 5-axis heightfield draping. |
| `emit/` | Target machine code emission: RS-274, GRBL, Marlin, Klipper, Duet, KUKA KRL, ABB RAPID, and ISO 14649 STEP-NC. |
| `codec/` | Ultra-compact DRY0/DRY1 binary columnar encoding and decoding. |

---

## 3. Testing & Verification

```bash
# Run unit & integration tests
cargo test -p dry-core

# Verify clippy lints (-D warnings)
cargo clippy -p dry-core --all-targets -- -D warnings
```

---

## License

Licensed under the **Functional Source License, Version 1.1, MIT Future License (FSL-1.1-MIT)**. See [LICENSE](../../LICENSE).

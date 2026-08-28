# The Dry Book: Algorithmic CAM & Parametric Manufacturing Engine

> **Version**: 0.7.0  
> **Status**: Production-Grade / Active Standard  
> **Ecosystem**: Rust Native Core, WASM Web Engine, Python SDK (PyO3), TypeScript SDK, Production Verify Runner

---

## Welcome to Dry

**Dry** is a high-performance, mathematically proven, clean-room parametric design and Computer-Aided Manufacturing (CAM) engine written in Rust. It eliminates reliance on legacy monolithic slicing software by introducing an open, typed Intermediate Representation (**Dry IR**) that resolves feature-based algorithmic designs directly into machine motion, physical simulations, and machine-safe multi-dialect programs.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          L0: Feature Graph                              │
│         (FeatureProgram, Feature@pose, Group, Repeat, Splines)          │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │ expand_features
┌────────────────────────────────────▼────────────────────────────────────┐
│                         L1: Path Dialect                                │
│       (Design, Op::Move, Op::Arc, Op::Clothoid, Power, Channels)        │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │ resolve
┌────────────────────────────────────▼────────────────────────────────────┐
│                        L2: Motion Dialect                               │
│        (Toolpath, Segment, Columnar DRY0/DRY1, Typed Units)             │
├────────────────────────────────────┼────────────────────────────────────┤
│           Analysis                 │            Lowering                │
│    simulate() / verify()           │              emit()                │
└────────────────────────────────────┴────────────────────────────────────┘
                                     │
    ┌────────────────────────────────┼────────────────────────────────┐
    │                                │                                │
┌───▼──────────────┐       ┌─────────▼────────┐             ┌─────────▼────────┐
│ FFF G-code       │       │ CNC Machining    │             │ Robotics & Laser │
│ (Marlin/Klipper) │       │ (RS-274/STEP-NC) │             │ (KUKA KRL/GRBL)  │
└──────────────────┘       └──────────────────┘             └──────────────────┘
```

---

## Chapters

1. [Chapter 1: System Specification & Formal Contracts](01_system_specification.md)
   * The 4-Tier Dialect Hierarchy ($L0 \to L1 \to L2 \to L3$)
   * Dry IR v0 Wire Specifications (JSON, Columnar `DRY0`, Streaming `DRY1`)
   * Dimensional Physics & Compile-Time Units System
   * 15 Automated Machine-Safety Contracts

2. [Chapter 2: Algorithmic Authoring & Computational Geometry](02_algorithmic_authoring.md)
   * Python and TypeScript SDK Architectures
   * Continuous-Z Mathematical Vases and Smooth Manifolds
   * Bio-mimetic Cellular Infill: Triply Periodic Minimal Surfaces (TPMS)
   * Metamaterials: Auxetic Star-Polygon Lattices

3. [Chapter 3: Multi-Axis CAM & Subtractive Manufacturing](03_multi_axis_and_subtractive.md)
   * 5-Axis Toolframe Orientation & Kinematic Solvers (AB / BC Head-Table)
   * 2.5D Pocketing and Profile Milling (RS-274 / LinuxCNC / STEP-NC)
   * Industrial Robotics: Generating Valid KUKA Robot Language (KRL)
   * Laser & Dynamic Spindle Power Modulation (GRBL $M3$/$M5$)

4. [Chapter 4: Production Architecture, Cloud & Deployment](04_production_and_cloud_operations.md)
   * The `dry-verify-runner` Microservice
   * Cryptographic Token Licensing (Ed25519) & Rate Limiting
   * Structured Telemetry, Prometheus Metrics, and Request Tracing
   * Containerization, Security Guarantees, and SLO Enforcement

5. [Chapter 5: Executable End-to-End Examples](05_executable_examples.md)
   * Annotated Walkthroughs of `examples/python/` and `examples/typescript/`
   * Pre-Flight Hardware Audits against the Machine Catalog

# Dry — vision & scope

> Project: **Dry** — the toolpath compiler. Interchange format: the **Dry IR**.


## The thesis

Algorithmic toolpath generation is a **compiler problem**, not a library problem. A design is a
*program* that produces machine motion + process intent; that intent should be lowered, optimised,
verified, simulated and emitted the way a compiler lowers a program — through a typed intermediate
representation, with many front-ends and many back-ends.

We are not building "a better FullControl." We are building **toolpath compiler infrastructure** — the
"LLVM/MLIR for machine motion." The product is the **IR + engine**; authoring *languages* and target
*machines* are interchangeable front-ends and back-ends hanging off it.

This is justified by direct evidence: the FullControl fork (the FullControl fork) already pushed the hot path into
a Rust kernel, hardened a serialized IR, added a second authoring front-end (TypeScript), a g-code
verify/optimise engine, and a wasm runtime — *without a rewrite*. The clean-slate system is the same
architecture taken to its logical end, freed from the FC API and byte-compat constraints. The prior-art
survey (`docs/ir_prior_art.md`) confirms the niche — a typed, units-aware, multi-language IR for
**algorithmic, arc-native, non-planar, variable-width** toolpaths — is genuinely unoccupied.

## Why now / why a clean break

The fork proved the layering works but is held back by FC's legacy: pydantic step objects, a stateful
`resolve` entangled with authoring, an XYZ-centric `Segment` that fights non-planar/5-axis, units as
convention, and a Python-as-implementation core. A no-back-compat rewrite lets the **IR and the Rust
engine become the product**, with Python demoted to one binding among several. We reach it by *finishing
the strangler-fig and cutting the cord* (see `02-roadmap.md`), not from a blank page.

## In scope

- **Dry IR** — a typed, units-aware, multi-level IR (design → path → motion → target dialects), with a
  general **toolframe** (position + orientation), per-point typed **channels** (extrusion / speed /
  temperature / flow / tool / width / height), **provenance** and declared **invariants**. JSON + a
  compact binary/columnar encoding. A published, versioned **standard**.
- **The engine** (Rust → native + wasm, one codebase): `lower`, `simulate`, `verify`, `optimise`,
  `emit`, `parse` (machine-code → IR), `reverse-engineer` (toolpath → parametric design).
- **Authoring SDKs** (thin, logic-free, emit IR): Python, TypeScript, Rust-native.
- **Targets** (back-end dialects): FFF g-code (Marlin/Klipper/Duet…) first; then CNC (RS-274 / STEP-NC
  intent), laser (GRBL), robot. Interchange import/export: g-code, 3MF Toolpath, mesh-in (STL/3MF),
  STEP-NC.
- **Tooling**: CLI (verify/optimise/inspect/convert), a web playground + realistic viewer (wasm),
  reverse-engineering.

## Out of scope (non-goals)

- **Not a CAD / B-rep / mesh kernel.** Dry IR is *downstream* of geometry; meshes are an *import*, not the
  representation. (Use OCCT/Manifold upstream if you need solids.)
- **Not a slicer.** The system is for *algorithmic* toolpaths. A mesh→toolpath slicer could be one
  front-end, but it is not the core mission.
- **Not real-time motion control / firmware.** The IR is design-time; execution is the machine's job.
- **Not backward-compatible with FullControl.** A clean API, clean names, units everywhere.

## Success criteria

1. **Parity, then beyond.** Reach FFF-3-axis output parity with the fork (gated by ported conformance
   fixtures — see `03-conformance.md`), *then* do what FC can't: native non-planar + 5-axis, units-safe
   by construction, splines/clothoids, streaming million-segment prints.
2. **Multi-front-end.** ≥2 authoring SDKs (Python + TypeScript) producing identical IR for the same
   design, proven by conformance.
3. **Runs everywhere.** One engine: native (CLI/server) and wasm (browser), bit-comparable.
4. **The IR is a standard.** Versioned spec, JSON + binary, ≥1 external tool importing/exporting Dry IR.
5. **Verifiable.** Designs declare contracts the compiler enforces; arbitrary machine code can be
   parsed, verified, optimised.

## Relationship to the FullControl fork

The fork is the **bootstrap source**, not a dependency to preserve:
- **Reuse the substance:** `rust_kernel/` (walk/metrics/gcode/parser), the IR (`fullcontrol/ir/`),
  serialize, the optimisation passes, the columnar fast-path, the ~695 device profiles, and the ~906
  tests + golden g-code — ported as **conformance fixtures** so the rewrite is fed by accumulated
  correctness, not starting blind.
- **Drop the surface:** the FC Python API, pydantic step objects, the stateful resolve, the XYZ-centric
  Segment, the `lab/` split.
- **Migration:** the new Python SDK keeps FC-flavored ergonomics so the existing community (Colab,
  fullcontrol.xyz) can move with a thin compat shim; the old FC API is cut **last**, only after parity.

## The honest risk (stated up front)

A clean slate re-pays the correctness tax (profiles, flavor edge cases, byte-identity), ships nothing
during the rewrite, and has a far larger surface than FFF-3-axis. This only makes sense **because the
goal is the platform/standard**, and is only survivable **because we bootstrap from the fork's tests and
profiles and gate every phase on conformance**. See `02-roadmap.md` for the risk register and the
strangler-fig sequencing that de-risks it.

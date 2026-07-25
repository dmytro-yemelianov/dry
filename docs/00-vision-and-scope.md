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

This is justified by direct evidence: the FullControl fork already pushed the hot path into
a Rust kernel, hardened a serialized IR, added a second authoring front-end (TypeScript), a g-code
verify/optimise engine, and a wasm runtime — *without a rewrite*. The clean-slate system is the same
architecture taken to its logical end, freed from the FC API and byte-compat constraints. The prior-art
survey (`docs/ir_prior_art.md`) confirms the niche — a typed, units-aware, multi-language IR for
**algorithmic, arc-native, non-planar, variable-width** toolpaths — is genuinely unoccupied.

## Why now / why a clean break

The fork proved the layering works but is held back by FC's legacy: pydantic step objects, a stateful
`resolve` entangled with authoring, an XYZ-centric `Segment` that fights non-planar/5-axis, units as
convention, and a Python-as-implementation core. A no-back-compat rewrite lets the **IR and the Rust
engine become the product**, with Python demoted to one binding among several. We reach it by *a
clean-room reimplementation against a behavioural oracle* (see `02-roadmap.md` + `CLEANROOM.md`) —
independent and commercially licensable, not a blank-page gamble.

## In scope

- **Dry IR** — a typed, units-aware, multi-level IR (design → path → motion → target dialects), with a
  general **toolframe** (position + orientation), per-point typed **channels** (extrusion / speed /
  temperature / flow / tool / width / height), **provenance** and declared **invariants**. JSON + a
  compact binary/columnar encoding. A publicly documented, versioned **contract**.
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

1. **Parity, then beyond.** Reach FFF-3-axis output parity with the fork (gated by oracle-generated conformance
   fixtures — see `03-conformance.md`), *then* do what FC can't: native non-planar + 5-axis, units-safe
   by construction, splines/clothoids, streaming million-segment prints.
2. **Multi-front-end.** ≥2 authoring SDKs (Python + TypeScript) producing identical IR for the same
   design, proven by conformance.
3. **Runs everywhere.** One engine: native (CLI/server) and wasm (browser), bit-comparable.
4. **The IR is a stable contract.** Public versioned spec, JSON + binary, and ≥1 licensed external tool
   importing/exporting Dry IR.
5. **Verifiable.** Designs declare contracts the compiler enforces; arbitrary machine code can be
   parsed, verified, optimised.

## Relationship to FullControl (inspiration + oracle, not code)

Dry is **independent and proprietary**; FullControl (and its fork) are GPLv3. Dry is therefore a
**clean-room** implementation—FullControl is used only as a reference, never as code. See
`CLEANROOM.md` for the full discipline.
- **Inspiration:** the design ideas — the multi-level IR, passes, flavors, the gallery of demos. Ideas
  and architecture are not copyrightable; Dry reimplements them from this spec and first principles.
- **A behavioural oracle (dev/CI only):** FullControl is *run* to generate the expected outputs (g-code,
  metrics) that Dry's conformance tests target — matching functional output is interoperability, not
  copying. The oracle lives under `conformance/oracle/`, is **never shipped or linked into Dry**, and is
  retired once Dry is self-consistent (FullControl's role asymptotes to zero — a fading scent).
- **No reuse of code, tests, or profile files.** Every line of Dry is written fresh; device profiles are
  regenerated from primary sources; conformance corpora are *generated* by the oracle, not vendored.
- **Drop, don't port:** the FC Python API, pydantic step objects, the stateful resolve, the XYZ-centric
  Segment, the `lab/` split — these are simply not carried over.
- **Migration:** the new Python SDK offers FC-flavored ergonomics so the existing community (Colab,
  fullcontrol.xyz) can move easily — as an independent, behaviourally-compatible reimplementation.

## The honest risk (stated up front)

Clean-room means Dry is **reimplemented, not lifted** — you forgo a copy-paste shortcut and re-establish
the correctness (profiles, flavor edge cases, byte-identity) yourself; it ships nothing during the
build-up and has a far larger surface than FFF-3-axis. This is survivable because **the oracle makes
clean-room cheap**: FullControl hands you the exact target output, so you implement until the diff is
zero rather than flying blind, and every phase is gated on that conformance. It is worth it because the
goal is a **commercial platform with a public integration contract**. Independence preserves ownership,
keeps GPL code out of customer artifacts, and supports private licensing. See `02-roadmap.md` for the
risk register and sequencing.

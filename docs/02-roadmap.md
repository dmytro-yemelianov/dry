# Dry — roadmap

The route is **clean-room, oracle-gated, then retire the oracle**: build the independent Rust core + Dry
IR from scratch, gate every phase on conformance **generated from the FullControl oracle** (run at
dev/CI, never shipped — see `CLEANROOM.md`), grow the SDKs and targets, and drop the oracle dependency
once Dry is self-consistent. Each phase has a goal, deliverables, and a hard **exit gate** (you don't
proceed until it's green). Phases overlap where dependencies allow; the critical path is P0 → P1 → P2 → P6.

## Phase 0 — Foundations & conformance harness
**Goal:** a standalone Rust `core` crate (no PyO3/numpy coupling) + the Dry IR v0 spec + a conformance
harness seeded from the fork.
**Deliverables (clean-room — see `CLEANROOM.md`):**
- A dependency-free `core` crate **written from scratch** (no PyO3/numpy); bindings (`pyo3`,
  `wasm-bindgen`) are thin adapter crates. (Reimplemented, not extracted from FullControl.)
- Dry IR v0: the L2 motion dialect + columnar storage + JSON & binary encodings + typed units, written as
  a versioned spec doc + Rust types.
- **Conformance harness**: a `conformance/oracle/` step that **runs FullControl** (dev/CI only, never
  shipped) to *generate* the golden g-code, the gallery, and device-profile targets; a runner that diffs
  Dry's output against them. Corpora are generated, not vendored.
**Exit gate:** the from-scratch `core` reproduces the oracle's `simulate` + Marlin `emit` **byte-identical**
on every generated golden, native and wasm.

## Phase 1 — The typed core: simulate / verify / emit at parity (FFF Marlin)
**Goal:** the L2 dialect + engine analyses, units-safe, at FFF-3-axis parity.
**Deliverables:** `simulate`, `verify` (reimplement the validate + invariant rules (oracle-gated)), `emit` (Marlin first, then
Klipper/Duet flavors), the Python binding (PyO3) + a minimal CLI (`inspect`/`verify`/`emit`). Typed
quantities throughout; provenance + declared invariants in the IR.
**Exit gate:** byte-identical g-code across Marlin/Klipper/Duet vs the fork on the generated goldens; verify
reproduces the fork's validation messages; CLI usable.

## Phase 2 — Authoring: L1 path dialect + lowering + the Python SDK (gallery parity)
**Goal:** author designs in the new Python SDK; lower L0/L1 → L2; reach **gallery parity**.
**Deliverables:** the L1 path dialect (Line/Arc/Spline + channels), `expand_features` + `resolve`
lowering passes, the Python authoring SDK (FC-flavored), and the 27 fork gallery designs reimplemented
(oracle-gated; 26 are exported today, with the remaining gap recorded in the source audit) as the
authoring conformance suite.
**Exit gate:** every gallery design, authored in the new Python SDK, lowers to L2 and emits g-code
that matches the fork's output for that design (within the documented tolerance), and passes its declared
invariants.

## Phase 3 — Optimise, parse, reverse, and the web runtime
**Goal:** the full engine surface + the browser story.
**Deliverables:** optimisation passes (arc_fit/travel_reorder/adaptive_speed/simplify/coasting/z_hop) on
L2 with invariant tests; `parse` (g-code → L2, byte-identical round-trip); `reverse` (toolpath → design);
the **wasm build** + a web playground/realistic viewer (reimplemented; oracle `web/`).
**Exit gate:** `emit(parse(g)) == g` byte-identical on the goldens; each opt pass conserves its invariant;
the wasm playground renders + simulates + emits a gallery design client-side.

## Phase 4 — Multi-front-end + the IR as a published standard
**Goal:** prove "many front-ends, one IR" and publish Dry IR.
**Deliverables:** the **TypeScript SDK** (reimplemented; oracle `ts/`) and a **Rust authoring SDK**, both producing
identical IR for the same design (cross-SDK conformance); the Dry IR spec published as a versioned standard
(JSON + binary, semver, conformance test vectors); a reference importer/exporter (3MF Toolpath).
**Exit gate:** a fixed design authored in Python, TS, and Rust produces byte-equal Dry IR; an external tool
(or a second implementation) round-trips a Dry IR test vector.

## Phase 5 — Generalise: non-planar, 5-axis, more targets
**Goal:** do what FC can't.
**Deliverables:** the **toolframe** generalisation in L2 (orientation channel), non-planar authoring
helpers, 5-axis IK lowering, and target dialects beyond FFF: CNC (RS-274 / STEP-NC intent), laser (GRBL),
robot (one vendor). Splines/clothoids in L1; streaming for >1M segments.
**Status (2026-08-02): all five items merged.** The gate is met with one asterisk that should not be
lost: "emits **valid** programs" is earned for CNC — LinuxCNC `rs274` is a genuine independent
interpreter and gates CI — and *not* for the robot target, where an external ANTLR grammar proves the
KRL module parses but nothing has run it on a controller. The emitted banner says so.

**Exit gate:** a non-planar and a 5-axis design lower + simulate + emit correctly on a reference machine
model; a CNC and a laser target emit valid programs from the same IR.

## Deployment track — from a gated engine to an operable service

Runs **alongside** Phases 5 and 6, not after them: it is gated by product decisions and operational
capability rather than by engine work, so it does not queue behind the oracle retirement.

**Goal:** something a paying user can depend on. The engine is heavily gated; the product is not
deployed, and no CI gate in this repo has ever served a request.

**Deliverables:** one named service (today there are two divergent sketches — `containers/verify-runner`
and the `crates/cloud` spike); observability; authentication, quota and revocation; a deploy pipeline
with a rehearsed rollback; a measured capacity curve; signed artifacts with an SBOM; a runbook and a
data-handling policy for uploaded programs, which are customer IP.

**Exit gate:** see [`23-deployment-roadmap.md`](23-deployment-roadmap.md). Note that "no hosted
service" is a legitimate outcome — the CLI ships today — and choosing it removes most of this track.

## Phase 6 — Stand alone (retire the oracle)
**Goal:** Dry is the product; the FullControl oracle is no longer needed.
**Deliverables:** the Python SDK reaches *feature* parity (not just output) with FC's authoring API; a
migration guide ([`migration-from-fullcontrol.md`](migration-from-fullcontrol.md)) + the `dry.compat.fullcontrol`
shim ease the move for FC users (Colab, fullcontrol.xyz); Dry's **own** golden outputs serve as the
reference.
**Status (2026-08-18): Delivered.** All 28 gallery designs and 14 conformance vectors pass independently;
`dry.compat.fullcontrol` drop-in shim is verified by unit tests; migration documentation is published.
**Exit gate:** the **entire** conformance suite passes against Dry's own references (goldens + gallery +
profiles + cross-SDK); Dry IR is the public contract.

## Deferred strategic initiative — the Dry IR language and ecosystem

**Status:** intentionally unscheduled; preserve for a later planning pick after the current FFF roadmap
and the P5 target experiments establish the right boundaries. The implementation sequence, compatibility
strategy and merge-sized work packets are defined in
[`20-dry-ir-ecosystem-implementation-plan.md`](20-dry-ir-ecosystem-implementation-plan.md).

**Goal:** evolve Dry from a strong FFF toolpath compiler into a machine-independent, verified
manufacturing-language layer with a usable surrounding ecosystem: versioned dialect contracts,
multi-language SDKs, capability/profile schemas, validators, importers, target backends, conformance
kits and reference workflows. G-code and other controller languages remain compatibility backends; Dry
does **not** become real-time motion-control firmware.

**Current baseline:** Dry already has the correct compiler shape — authoring ops → resolved absolute L2
IR → simulate/verify/optimise → Marlin/Klipper/Duet output — but Dry IR v0 is only the resolved L2
contract. Authoring still contains inherited state and bare numeric conventions; profiles and
verification are FFF-centred; coordinate frames, a general capability model, high-level manufacturing
intent, collision/process validation and production non-FFF backends are not complete.

**Candidate scope when activated:**

- A real **L0 manufacturing-intent dialect** for features/operations such as regions, deposition
  strategies, pockets, profiles, drilling and tool/process plans, while keeping CAD/B-rep and slicing
  kernels outside the core.
- A **non-modal L1 path dialect** with explicit state, named coordinate frames and transforms
  (design/workpiece/fixture/tool/machine); lowering may use state internally, but the interchange
  contract must not depend on hidden prior commands.
- **End-to-end dimensional types** at public SDK, profile and wire boundaries, with canonical-unit
  normalization and explicit dimensions instead of undocumented bare-number conventions.
- A versioned, extensible **typed channel registry** and **machine capability schema** covering supported
  primitives, axes/kinematics, tools, process limits and target features without hard-coding one firmware
  family into the generic IR.
- Target-aware verification beyond the current rule catalog: tool/fixture/build-volume collision,
  reachability and singularities, unsupported operations, thermal/process envelopes and explicit
  treatment of opaque/manual machine code. A clean report must retain a precisely documented meaning
  rather than imply general unattended safety.
- A pluggable L3 backend contract with production-grade targets selected from the P5 experiments
  (FFF plus CNC/RS-274 and/or laser/GRBL first; STEP-NC for intent interchange where it adds value).
- Bidirectional target contracts: parsers/lifters for FFF and the selected non-FFF workflow, with
  explicit lossiness, tolerance and opaque-command policies instead of treating emitted machine code as
  automatically recoverable intent.
- Conformance at every boundary: intent → path, path → absolute motion, motion → target, import/lift,
  and target-specific verification, including reference-machine and versioned controlled-hardware
  protocols. The public ecosystem includes schemas, fixtures, diagnostics and compatibility tooling,
  not only the Rust implementation.

**Non-goals:** replacing firmware motion planners, promising one identical program for fundamentally
different manufacturing processes, silently accepting unknown controller behavior, or claiming
certification from static verification alone.

**Entry gate:** P2.3 (feature expansion) and P4.3 (published IR contract) are complete; at least one
non-FFF target experiment, **P5.3 or P5.4**, has met its prototype acceptance; and an activation record
names the selected non-FFF workflow, reference machine/controller and evidence carried forward. This is
the objective replacement for an open-ended dependency on "relevant P5 experiments."

**Exit gate:** within at least one process, a machine-independent authored program lowers to two
different controller dialects through declared capability profiles with no hidden modal assumptions;
FFF plus one non-FFF workflow passes structural, process, kinematic and collision/reachability checks on
reference machines; emitted programs lift back to their declared semantic boundary with documented
losses; controlled hardware runs pass a versioned protocol with archived evidence; and an independent
implementation round-trips the public contracts.

## Mathematical assurance workstream

**Status:** planned as FM1 in
[`21-mathematical-assurance-plan.md`](21-mathematical-assurance-plan.md). The v0 quantities, L2 logical
model, planar feature expansion and codec semantics can start before D1. Proofs for full frames,
capabilities and targets follow the corresponding D1 contract freezes.

**Goal:** give the language a machine-checked semantic foundation and connect it to the Rust compiler
without confusing exact real-number theorems, bounded floating-point behavior, conformance tests or
hardware evidence.

**Scope:** typed quantity algebra; transforms and frames; L0/L1/L2 lowering; trace and observation
relations; simulation/verifier predicates; per-pass optimization contracts; codec/version round trips;
capability matching; target lowering and semantic lift; floating-point error budgets; and an executable
Rust refinement bridge.

**Non-goals:** proving firmware or hardware correct, treating sampled curves as analytically exact,
claiming static verification certifies a process, or labeling the whole compiler “formally verified”
when only a subset has met the published proof/refinement gates.

**Entry gate:** the published L2 v0 specification is sufficient for the FM1.1 tooling/claim constitution
and the v0 portion of FM1.2/FM1.7. Each later proof packet additionally requires its public language
contract to be frozen. D1-dependent proof work does not set D1 semantics.

**Exit gate:** every supported assurance claim maps to a checked theorem, explicit assumptions and
numeric domain; every semantic floating-point boundary appears in a versioned, source-drift-checked
inventory, every bounded theorem names an accepted numeric profile, and unresolved claims remain
marked empirical/pending; Rust passes independent refinement checks; FFF and the selected non-FFF
workflow satisfy the bounded conditional compiler theorem; and an independent reviewer reproduces the
proof build, numeric profiles, boundary inventory and claim registry.

## Track C — Industrial Standards & Certification

**Goal:** Qualify the Dry engine, IR, and container services against tier-1 manufacturing, aerospace, medical, and cybersecurity standards. See [`26-industrial-certification-and-standards.md`](26-industrial-certification-and-standards.md).

* **Additive & Subtractive CAM Standards**:
  * **ISO/ASTM 52915 (3MF Core & Toolpath Extension)**: Official 3MF Consortium Compliance Seal for lossless interchange with Siemens NX, Netfabb, and OEM slicing engines.
  * **ISO 14649 (STEP-NC AP 238)**: Full semantic workingstep compliance for high-precision 5-axis aerospace machining.
  * **ISO 6983-1 / DIN 66025 (RS-274)**: Modal block conflict auditing and independent LinuxCNC / Fanuc conformance.
* **Safety-Critical & Formal Verification Standards**:
  * **DO-178C / DO-333 (Level A/B/C)**: Formal Methods Evidence Kit based on Lean 4 machine-checked proofs for flight-critical additive manufacturing path planning.
  * **IEC 62304 (Class B/C)**: Medical device software life cycle qualification for patient-specific orthopedic and dental implants.
  * **ISO 26262 (ASIL D)**: Automotive functional safety Tool Confidence Level (TCL 2/3) qualification.
* **Cybersecurity, Cloud & Supply Chain**:
  * **SOC 2 Type II**: Security, Availability, and Zero-Retention confidentiality certification for `dry-verify-runner`.
  * **SLSA Level 3/4 & NIST SP 800-218 (SSDF)**: Verifiable cryptographic container provenance, Sigstore/Cosign container signing, and SPDX/CycloneDX SBOM generation.

## Track E — Advanced CAM & Computational Geometry Expansion

**Goal:** Expand the CAM kernel from 2.5D prismatic milling and analytic draping into adaptive high-speed machining, arbitrary mesh heightfields, and functionally graded metamaterials.

* **E1.1 (Radial Tool Engagement & Trochoidal Milling)**: Dynamically calculate radial cutter engagement angle $\theta_e(s)$ and automatically generate trochoidal peeling loops in $90^\circ+$ internal corners to maintain constant cutting load ($\text{MRR} = \text{const}$).
* **E1.2 (Helical Ramp Entry & Plunge Protection)**: Automatic helical spiral ramp-in before pocket clearing in hard materials (aluminum/steel/titanium).
* **E1.3 (Mesh Heightfield 5-Axis Drape)**: Ray-surface BVH (Bounding Volume Hierarchy) accelerated projection over imported STL/OBJ triangle meshes for conformal non-planar 3D printing and 5-axis subtractive milling.
* **E1.4 (Functionally Graded TPMS Metamaterials)**: Spatially varying scalar density field $c(x,y,z)$ modulating TPMS isovalue smoothly across parts.

## Track M — Deepened Mathematical Assurance (FM2)

**Goal:** Formally connect machine-checked Lean 4 semantics with IEEE-754 floating-point execution and kinematic singularities.

* **FM2.1 (IEEE 754 Floating-Point Refinement)**: Formally prove that the Rust `f64` implementation refines the exact rational $\mathbb{Q}$ semantics within a computable error bound $\varepsilon$: $|\text{Rust}_{\text{vol}}(f64) - \text{Lean}_{\text{vol}}(\mathbb{Q})| \le \varepsilon$.
* **FM2.2 (Euler Spiral / Fresnel Curvature Linearity)**: Formally prove bounded curvature derivative error ($d\kappa/ds = \text{const}$) in Lean 4 across full deflection intervals.
* **FM2.3 (5-Axis Polar Singularity Hold)**: Formally verify polar hold invariance ($k = \pm 1$) in BC kinematics proving zero-division immunity.

## Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| **Correctness tax** (profiles, flavor edge cases, byte-identity rediscovered as bugs) | high | high | Conformance-gate **every** phase; generate the goldens/profiles/gallery via the oracle *first* (P0). No phase proceeds without its gate green. |
| **Scope creep** (5-axis/CNC/splines/streaming balloon the surface) | high | high | Sequence them **last** (P5); P0–P3 are strictly FFF-3-axis at parity. Toolframe is *designed in* from P0 but only *exercised* in P5. |
| **Ship-nothing during rewrite** (momentum/funding loss) | medium | high | Ship the **wasm playground early** (P3) for a visible artifact; keep the fork live and maintained until P6. |
| **Ecosystem migration** (lose the FC community) | medium | high | New Python SDK keeps FC-flavored ergonomics + a compat shim; cut FC **last** (P6) with a migration guide. |
| **Second-system over-design** | medium | medium | Anchor every abstraction to an *oracle-validated behaviour* (architecture §10); if it isn't reused or conformance-tested, defer it. |
| **Two codebases to maintain** (fork + new) until P6 | certain | medium | Freeze the fork to maintenance-only once P1 starts; all new feature work goes to the new core. |
| **Parity gates ≠ robustness gates** (the conformance suite proves Dry matches the oracle on *well-formed* input; nothing in it exercises malformed, degenerate or hostile input) | certain | high | Confirmed by the 2026-07-31 core audit: every conformance corpus is oracle-generated and therefore well-formed by construction, so defects reachable only from hand-built IR, imported g-code, the binary codec or the SDKs' raw JSON were invisible to a green suite — including non-finite values printed verbatim into g-code. Mitigation: the **H1 hardening workstream** (`04-tasks.md`) validates at every ingress and at emit; add degenerate-input vectors to `conformance/` rather than relying on oracle-generated corpora alone. |
| **Compliance drift across regulatory targets** (aerospace vs medical vs cloud) | medium | high | Maintain standardized, machine-verifiable evidence kits in `docs/26-industrial-certification-and-standards.md` generated automatically by CI. |

## Sequencing & dependencies

```
P0 ──► P1 ──► P2 ──────────► P6 (cut: standalone Dry ecosystem)
         └──► P3 ──► P4 ──┘       │
                     └► P5 ───────┼──► Track E (Advanced CAM / Trochoidal / Mesh Drape)
                                  ├──► Track M (FM2 Deepened Lean 4 Refinements)
                                  └──► Track C (Industrial Standards & Certification)
```

Critical path: **P0 → P1 → P2 → P6** (Completed). Tracks C, E, and M build upon the stabilized v0.7.0 core.

See `03-conformance.md` for how the gates are defined, `04-tasks.md` for the actionable backlog, and `26-industrial-certification-and-standards.md` for certification criteria.

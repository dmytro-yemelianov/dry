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
**Exit gate:** a non-planar and a 5-axis design lower + simulate + emit correctly on a reference machine
model; a CNC and a laser target emit valid programs from the same IR.

## Phase 6 — Stand alone (retire the oracle)
**Goal:** Dry is the product; the FullControl oracle is no longer needed.
**Deliverables:** the Python SDK reaches *feature* parity (not just output) with FC's authoring API; a
migration guide + an optional FC-compatible shim ease the move for FC users (Colab, fullcontrol.xyz);
Dry's **own** golden outputs become the reference so the oracle can be dropped.
**Exit gate:** the **entire** conformance suite passes against Dry's own references (goldens + gallery +
profiles + cross-SDK); the FullControl oracle is removed from CI; the Dry IR is the public contract.

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

## Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| **Correctness tax** (profiles, flavor edge cases, byte-identity rediscovered as bugs) | high | high | Conformance-gate **every** phase; generate the goldens/profiles/gallery via the oracle *first* (P0). No phase proceeds without its gate green. |
| **Scope creep** (5-axis/CNC/splines/streaming balloon the surface) | high | high | Sequence them **last** (P5); P0–P3 are strictly FFF-3-axis at parity. Toolframe is *designed in* from P0 but only *exercised* in P5. |
| **Ship-nothing during rewrite** (momentum/funding loss) | medium | high | Ship the **wasm playground early** (P3) for a visible artifact; keep the fork live and maintained until P6. |
| **Ecosystem migration** (lose the FC community) | medium | high | New Python SDK keeps FC-flavored ergonomics + a compat shim; cut FC **last** (P6) with a migration guide. |
| **Second-system over-design** | medium | medium | Anchor every abstraction to an *oracle-validated behaviour* (architecture §10); if it isn't reused or conformance-tested, defer it. |
| **Two codebases to maintain** (fork + new) until P6 | certain | medium | Freeze the fork to maintenance-only once P1 starts; all new feature work goes to the new core. |

## Sequencing & dependencies

```
P0 ──► P1 ──► P2 ──────────► P6 (cut)
         └──► P3 ──► P4 ──┘
                     └► P5 (parallel, lands before/with P6)

P2.3 ─┐
P4.3 ─┼──► D1 activation ──► language foundations ──► target loop ──► ecosystem + hardware gates
P5.3/4┘
```

D1 remains outside the current critical path. Its activation consumes the published L2 contract,
feature-expansion experience and one accepted non-FFF prototype; it does not delay finishing P6.
Critical path: **P0 → P1 → P2 → P6**. P3 (engine surface + web) and P4 (multi-SDK + standard) branch off
P1/P2 and can run in parallel. P5 (generalisation) depends on P1's toolframe design and lands alongside
P6. The cut (P6) requires the full conformance suite green — i.e. P2 (gallery) + P1 (output) + P4
(cross-SDK) gates all passing.

See `03-conformance.md` for how the gates are defined and `04-tasks.md` for the actionable backlog.

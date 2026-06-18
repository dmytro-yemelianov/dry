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
lowering passes, the Python authoring SDK (FC-flavored), and the 27 gallery designs reimplemented (oracle-gated) as the
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
```
Critical path: **P0 → P1 → P2 → P6**. P3 (engine surface + web) and P4 (multi-SDK + standard) branch off
P1/P2 and can run in parallel. P5 (generalisation) depends on P1's toolframe design and lands alongside
P6. The cut (P6) requires the full conformance suite green — i.e. P2 (gallery) + P1 (output) + P4
(cross-SDK) gates all passing.

See `03-conformance.md` for how the gates are defined and `04-tasks.md` for the actionable backlog.

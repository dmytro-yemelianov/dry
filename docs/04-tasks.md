# Dry — task backlog

Actionable tasks per phase. **Size**: S ≈ ≤1 day, M ≈ a few days, L ≈ 1–2 weeks. **Dep**: prerequisite
task ids. Each task is "done" when its **acceptance** is green (and, where relevant, its conformance gate
from `03-conformance.md`). Front-loaded on P0–P2 (the critical path); later phases are sketched and
refined as they approach.

Legend: `[ ]` todo. IDs are stable references.

## Phase 0 — Foundations & conformance harness

- `[ ]` **P0.1** (M) Extract a dependency-free Rust `core` crate from `rust_kernel/src/{walk,metrics,gcode,parser}.rs`; move `pyo3`/`wasm-bindgen` into thin adapter crates. *Dep: —. Accept:* `core` builds with no pyo3/numpy; native + `wasm32-unknown-unknown` both compile; `cargo fmt`+`clippy -D warnings` clean (no `#[allow]`).
- `[x]` **P0.2** (M) **Unit-typed Dry IR v0**: the L2 motion dialect carries typed quantities — `Length`/`Area`/`Volume`/`Feedrate`/`Time`/`Flow`/`Angle` (`crates/core/src/units.rs`), each `#[serde(transparent)]`. The cross-dimensional physics is defined once in the operators (`Length×Length=Area`, `Volume÷Area=Length`, `Length÷Feedrate=Time`, …) so a unit confusion in resolve/simulate/emit is a *compile* error. *Accept (met):* types compile; mixed-unit arithmetic does not compile; round-trips through serde byte-identically (the whole conformance suite + all three bindings stay green). *Deferred to P2.1:* the general toolframe orientation + the channel registry (they belong to the L1 path dialect).
- `[x]` **P0.3** (M) JSON + binary (columnar) encodings of Dry IR v0 with a versioned header. The binary form (`crates/core/src/codec.rs`) is an Arrow-style struct-of-arrays body (each field a contiguous column — validity bitmaps for nullables, a kind dictionary) under DEFLATE (pure-Rust `miniz_oxide`); `Toolpath::to_bytes`/`from_bytes`, plus `dry pack`/`unpack`. *Accept (met):* `from(to(ir)) == ir` for every design (and empty/edge cases); on spiral_vase the binary is **25.3× smaller** than JSON (1380 B vs 34841 B), well past the 3× gate. *Header reserves room for provenance/invariants once the IR carries them.*
- `[ ]` **P0.4** (M) **Conformance export script**: fork → `conformance/{golden,gcode,gallery,profiles,roundtrip}/`. *Dep: —. Accept:* corpora generated + frozen; counts recorded.
- `[ ]` **P0.5** (M) Conformance **runner** + CI matrix (native + wasm; fmt/clippy; SDK lint). *Dep: P0.4. Accept:* runner diffs engine output vs each corpus; CI green on an empty engine (all corpora "pending").
- `[ ]` **P0.6** (S) Pin the **math backend** (platform libm bindings) used by native + wasm; ban ambient clock/RNG. *Dep: P0.1. Accept:* a `sin/cos/atan2/hypot` conformance test is bit-identical native vs wasm vs the fork.
- **P0 exit gate:** extracted `core` reproduces fork `simulate` + Marlin `emit` byte-identical on the golden corpus, native + wasm.

## Phase 1 — Typed core: simulate / verify / emit at parity

- `[ ]` **P1.1** (M) `simulate(ir@L2) -> Metrics` (time/distance/material/peak-flow), columnar fold. *Dep: P0.3. Accept:* metric parity with the fork's `simulate` on the corpus (≤1e-12).
- `[ ]` **P1.2** (L) `emit(ir@L2, Marlin) -> gcode` (reimplemented; oracle = `gcode.rs` + flavor vocab) + the F/E/arc/retraction state machine. *Dep: P0.3. Accept:* byte-identical Marlin on corpora 1,2.
- `[ ]` **P1.3** (M) Klipper + Duet flavors. *Dep: P1.2. Accept:* byte-identical on the flavor corpus.
- `[ ]` **P1.4** (M) Device-profile model + start/end procedures (regenerated from primary sources; cross-checked vs `devices/`). *Dep: P1.2. Accept:* sampled profiles emit identical preamble/postamble; full set nightly.
- `[~]` **P1.5** (M) `verify(ir, contracts) -> Report` (`crates/core/src/verify.rs`): Dry's own clean-room safety contracts + structural invariants, each a located `Finding` (rule id, severity, segment, message), `Serialize`-able, plus CLI `dry verify` (exit 1 on errors). **Landed:** `bounds`, `max-flow`, `speed`, `monotonic-z`, and always-on `finite` / `travel-extrudes` / `bead`. *Accept (met):* constructed violations are flagged at the right segment; a clean toolpath passes; the report serialises. *Deferred (need IR channels from P2.1):* `cold-extrusion`/`temp`/`retraction`/`first-layer` (the IR carries no temperature/fan/tool channel yet).
- `[ ]` **P1.6** (M) PyO3 binding + a minimal **CLI** (`inspect`/`verify`/`emit`). *Dep: P1.1–P1.5. Accept:* `cli emit design.tpir` produces the gated g-code; exit codes per verify.
- **P1 exit gate:** byte-identical g-code Marlin/Klipper/Duet + identical verify messages on corpora 1,2,4.

## Phase 2 — Authoring: L1 dialect + lowering + Python SDK

- `[ ]` **P2.1** (M) **L1 path dialect**: Line/Arc/Spline/Dwell/ToolChange + the channel registry (extrusion/speed/width/height/temp/fan/flow/tool). *Dep: P0.2. Accept:* L1 verifier passes; arcs carry centre/clockwise; channels typed + defaulted.
- `[ ]` **P2.2** (L) `resolve: L1 -> L2` lowering (deposition math + state propagation: running toolframe, extruder e-ratio, channel propagation) as a **pure pass**. *Dep: P2.1, P1.1. Accept:* a hand-written L1 design resolves to L2 that simulates/emits to the gated output.
- `[ ]` **P2.3** (M) `expand_features: L0 -> L1` (Repeat/Group/Feature@pose). *Dep: P2.1. Accept:* a feature graph expands to the same L1 as the hand-written equivalent.
- `[ ]` **P2.4** (L) **Python authoring SDK** (FC-flavored builders emitting L0/L1). *Dep: P2.1. Accept:* authoring a square/vase produces L1 that lowers to the gated g-code.
- `[ ]` **P2.5** (L) Reimplement (clean-room) the **27 gallery designs** to the new SDK as the authoring conformance suite (geometry-level comparison, not just metrics — per `03`). *Dep: P2.4, P0.4. Accept:* each design lowers+emits to match the fork within the documented tolerance and passes its declared invariants.
- **P2 exit gate:** every gallery design authored in the new Python SDK reproduces the fork's output + invariants.

## Phase 3 — Optimise, parse, reverse, web

- `[ ]` **P3.1** (L) Optimisation passes on L2 (arc_fit/travel_reorder/adaptive_speed/simplify/coasting/z_hop) with per-pass invariant tests (reimplemented; oracle the fork passes). *Dep: P1.1. Accept:* each pass conserves its invariant; material ≤1e-6; arc_fit emits G2/G3.
- `[ ]` **P3.2** (M) `parse(gcode, flavor) -> L2` (oracle: `parser.rs`), byte-identical round-trip. *Dep: P1.2. Accept:* `emit(parse(g)) == g` on the round-trip corpus.
- `[ ]` **P3.3** (M) `reverse(toolpath) -> design` (oracle: `reverse_engineer`). *Dep: P3.2. Accept:* recovers lobes/waves/profile on the fork's reverse-engineering fixtures.
- `[ ]` **P3.4** (M) **wasm build** + adapter; web playground + realistic viewer (oracle: `web/`). *Dep: P0.1, P2.4. Accept:* a gallery design renders+simulates+emits client-side; `application/wasm` MIME; deployable.
- **P3 exit gate:** round-trip byte-identical; opt invariants hold; wasm playground works end-to-end.

## Phase 4 — Multi-front-end + the IR standard

- `[ ]` **P4.1** (L) **TypeScript SDK** (oracle: `ts/`) emitting Dry IR. *Dep: P2.1. Accept:* a fixed design in TS == the Python Dry IR (≤1e-12).
- `[ ]` **P4.2** (M) **Rust authoring SDK**. *Dep: P2.1. Accept:* same design == Python/TS Dry IR.
- `[ ]` **P4.3** (M) Publish **Dry IR spec** (versioned, JSON+binary, semver) + conformance **test vectors**. *Dep: P0.3. Accept:* an external/second implementation round-trips the vectors.
- `[ ]` **P4.4** (M) **3MF Toolpath** import/export reference (oracle: `ir/threemf.py`); mesh-in (STL/3MF) importer stub. *Dep: P0.3. Accept:* extruding path round-trips within tolerance; documented lossiness.
- **P4 exit gate:** Python/TS/Rust produce byte-equal Dry IR; a Dry IR vector round-trips in a second impl.

## Phase 5 — Generalise: non-planar, 5-axis, more targets

- `[ ]` **P5.1** (L) Exercise the **toolframe orientation** channel end-to-end; non-planar authoring helpers. *Dep: P2.2. Accept:* a non-planar design lowers/simulates/emits correctly.
- `[ ]` **P5.2** (L) 5-axis **IK lowering** to B/C + a reference machine model. *Dep: P5.1. Accept:* a tilted-toolframe design emits valid 5-axis g-code on the model.
- `[ ]` **P5.3** (L) **CNC** (RS-274) + optional **STEP-NC intent** export. *Dep: P1.2. Accept:* a pocket/profile emits a valid CNC program.
- `[ ]` **P5.4** (M) **Laser** (GRBL) + one **robot** vendor target. *Dep: P1.2, P5.1. Accept:* valid programs from the same IR.
- `[ ]` **P5.5** (M) **Splines/clothoids** in L1; **streaming** L2 for >1M segments. *Dep: P2.1. Accept:* a clothoid-cornered design emits; a 1M-segment print streams without materialising objects.

## Phase 6 — Cut the cord

- `[ ]` **P6.1** (M) New Python SDK reaches **feature** parity with the FC API; a deprecated FC compat shim. *Dep: P2.4. Accept:* FC-style scripts run via the shim with a deprecation notice.
- `[ ]` **P6.2** (M) Migrate docs/site/community (Colab, fullcontrol.xyz) to the new stack + a migration guide. *Dep: P6.1. Accept:* the published demos run on the new engine.
- `[ ]` **P6.3** (S) Remove the FC Python implementation; Dry IR is the public contract. *Dep: full suite green. Accept:* repo builds without the FC API; the entire conformance suite passes.
- **P6 exit gate:** entire conformance suite green; FC API removed with a migration guide.

## Immediate next 5 (if starting today)

1. **P0.4** — the oracle generates the corpora into `conformance/` (needed by everything).
2. **P0.1** — extract the dependency-free `core` crate.
3. **P0.5** — conformance runner + native/wasm CI matrix.
4. **P0.2 / P0.3** — Dry IR v0 types + JSON/binary encodings.
5. **P0.6** — pin the shared math backend (bit-stability native↔wasm↔fork).
Then P0 exit gate = the first real proof the rewrite reproduces the fork.

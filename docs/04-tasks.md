# Dry — task backlog

Actionable tasks per phase. **Size**: S ≈ ≤1 day, M ≈ a few days, L ≈ 1–2 weeks. **Dep**: prerequisite
task ids. Each task is "done" when its **acceptance** is green (and, where relevant, its conformance gate
from `03-conformance.md`). Front-loaded on P0–P2 (the critical path); later phases are sketched and
refined as they approach.

Legend: `[ ]` todo, `[~]` partially landed, `[x]` landed for the current v0 scope. IDs are stable references.

## Phase 0 — Foundations & conformance harness

- `[x]` **P0.1** (M) Dependency-free Rust `core` crate with PyO3/wasm-bindgen isolated in adapter crates. *Accept (met):* `dry-core` builds without binding dependencies; workspace, wasm and PyO3 crates check separately.
- `[x]` **P0.2** (M) **Unit-typed Dry IR v0**: the L2 motion dialect carries typed quantities — `Length`/`Area`/`Volume`/`Feedrate`/`Time`/`Flow`/`Angle` (`crates/core/src/units.rs`), each `#[serde(transparent)]`. The cross-dimensional physics is defined once in the operators (`Length×Length=Area`, `Volume÷Area=Length`, `Length÷Feedrate=Time`, …) so a unit confusion in resolve/simulate/emit is a *compile* error. *Accept (met):* types compile; mixed-unit arithmetic does not compile; round-trips through serde byte-identically (the whole conformance suite + all three bindings stay green). *Deferred to P2.1:* the general toolframe orientation + the channel registry (they belong to the L1 path dialect).
- `[x]` **P0.3** (M) JSON + binary encodings of Dry IR v0 with a versioned header. `DRY0` (`Toolpath::to_bytes`) is the compact Arrow-style columnar body under DEFLATE (validity bitmaps for nullables, a kind dictionary); `DRY1` (`Toolpath::to_streaming_bytes`, now used by `dry pack`) stores compressed row chunks so CLI simulate/verify/emit can decode bounded blocks. `Toolpath::from_bytes` and `dry unpack` read both forms. *Accept (met):* `from(to(ir)) == ir` for every design (and empty/edge cases); `DRY0` on spiral_vase is **25.3× smaller** than JSON (1380 B vs 34841 B), well past the 3× gate; `DRY1` round-trips every conformance design and streams through the shared binary iterator.
- `[x]` **P0.4** (M) **Conformance export script**: fork → `conformance/{golden,gcode,gallery,profiles,roundtrip}/`. *Accept (met):* corpora generated + frozen; counts recorded.
- `[~]` **P0.5** (M) Conformance **runner** + CI matrix (native + wasm; fmt/clippy; SDK lint). **Landed:** Rust conformance tests, wasm smoke runner, TS/Python tests. *Remaining:* formal CI matrix definition.
- `[x]` **P0.6** (S) Pin the **math backend** (platform libm bindings) used by native + wasm; ban ambient clock/RNG. *Accept (met):* native↔wasm math parity test is green.
- **P0 exit gate:** extracted `core` reproduces fork `simulate` + Marlin `emit` byte-identical on the golden corpus, native + wasm.

## Phase 1 — Typed core: simulate / verify / emit at parity

- `[x]` **P1.1** (M) `simulate(ir@L2) -> Metrics` (time/distance/material/peak-flow). *Accept (met):* metric parity with the current conformance corpus.
- `[x]` **P1.2** (L) `emit(ir@L2, Marlin) -> gcode` (reimplemented; oracle = `gcode.rs` + flavor vocab). *Accept (met):* byte-identical Marlin on the current corpus, including arcs and splines lowered to lines.
- `[x]` **P1.3** (M) Klipper + Duet flavors. **Landed:** Marlin, Klipper, and Duet (RepRapFirmware) G-code dialects supported in emitter. Klipper translates `Dwell` to `G4 P<ms>`, while Marlin/Duet use `G4 S<sec>`. Firmware flavor from profile maps directly to emitter settings, fully tested.
- `[x]` **P1.4** (M) Device-profile model + start/end procedures (regenerated from primary sources; cross-checked vs `devices/`). *Accept (met):* a versioned profile schema in `dry-core` supporting start/end procedures, firmware flavor, build volume, feedrate range, filament diameter, max volumetric flow, minimum nozzle temperature, line width/layer height, retraction limits, and first-layer parameters.
- `[x]` **P1.5** (M) `verify(ir, contracts) -> Report` (`crates/core/src/verify.rs`): Dry's own clean-room safety contracts + structural invariants, each a located `Finding` (rule id, severity, segment, message), `Serialize`-able, plus CLI `dry verify` (exit 1 on errors). *Accept (met):* all core contracts implemented (bounds, flow, speed, monotonic-z, cold-extrusion, orientation, finite, travel-extrudes, bead, retraction speed/distance, travel without retraction, and first layer speed/height).
- `[x]` **P1.6** (M) PyO3 binding + CLI (`inspect`/`simulate`/`verify`/`emit`/`optimize`/`pack`/`unpack`). *Accept (met):* CLI emits gated g-code; verify exit codes are tested.
- **P1 exit gate:** byte-identical g-code Marlin/Klipper/Duet + identical verify messages on corpora 1,2,4.

## Phase 2 — Authoring: L1 dialect + lowering + Python SDK

- `[~]` **P2.1** (M) **L1 path dialect + channel registry**. **Landed:** the typed process channels — `temperature`, `fan`, `flow` (scales deposited volume), `tool` — authored as L1 ops, defaulted + propagated through `resolve`, riding each L2 segment (serde-omitted when unset, so motion-only IR stays byte-identical); plus the `Dwell` op (`G4` emit + simulated time) and a `cold-extrusion` verify rule. The codec round-trips the channels; arcs already carry centre/clockwise. *Accept (met):* channels typed + defaulted; conformance byte-identity preserved; channel-bearing IR round-trips through the binary codec. The channels are exposed in all front-ends (Rust `Op`, Python `.temperature()/.fan()/.flow()/.tool()/.orient()/.dwell()`, TS the same). The **toolframe orientation** also landed: `Segment.orientation` (tool-direction unit vector, `None` ⇒ +Z), the `Orient` L1 op, codec round-trip, an `orientation-not-unit` verify rule, and `.orient(i,j,k)` in every SDK — so non-planar / 5-axis is now a first-class IR property. The **5-axis target lowering** also landed: `emit` with `five_axis` derives rotary `A`/`B` words (degrees) from the toolframe orientation by a documented AB-head convention (`B = atan2(i, k)`, `A = atan2(j, hypot(i, k))`), emitted only when they change; CLI `dry emit --five-axis`. Default emit stays 3-axis (orientation dropped → byte-identical). *Deferred to later P2.x:* `Spline`/`Clothoid`/`ToolChange` path nodes and alternate rotary kinematics (AC/BC, table-vs-head). (L2 optimisation: `merge_collinear` has landed — see P3.1.)
- `[x]` **P2.2** (L) `resolve: L1 -> L2` lowering (deposition math + state propagation: running toolframe, extruder e-ratio, channel propagation) as a **pure pass**. *Accept (met):* hand-written L1 designs resolve to L2 that simulate/emit to the gated output; checked resolve validates physical inputs at binding boundaries.
- `[ ]` **P2.3** (M) `expand_features: L0 -> L1` (Repeat/Group/Feature@pose). *Dep: P2.1. Accept:* a feature graph expands to the same L1 as the hand-written equivalent.
- `[x]` **P2.4** (L) **Python authoring SDK** (FC-flavored builders emitting L1). *Accept (met):* authoring square/arc/channel/spline designs lowers to gated g-code and tested metrics.
- `[ ]` **P2.5** (L) Reimplement (clean-room) the **27 fork gallery designs** in the new SDK as the authoring conformance suite (geometry-level comparison, not just metrics — per `03`). **Current: 26 exported fixtures; `gyroid_infill` is missing, and the website's Overhang Challenge Plus still needs a distinct case.** *Dep: P2.4, P0.4. Accept:* each design lowers+emits to match the fork within the documented tolerance and passes its declared invariants.
- **P2 exit gate:** every gallery design authored in the new Python SDK reproduces the fork's output + invariants.

## Phase 3 — Optimise, parse, reverse, web

- `[x]` **P3.1** (L) Optimisation passes on L2 — IR→IR transforms with invariant tests. **Landed:** `merge_collinear`, `arc_fit`, `travel_reorder`, `adaptive_speed`, `coasting`, `z_hop`, and the shared `optimize_pipeline`/`optimize_aggressive_pipeline` used by CLI/wasm/TS. Complete integration tests verify volume, geometry, and connectivity conservation.
- `[x]` **P3.2** (M) `parse(gcode, flavor) -> L2` (oracle: `parser.rs`), byte-identical round-trip. *Accept (met):* G-code parser foundation parses modal state and lifts segments, with a G-code roundtrip parser gate (`emit(parse(g)) == g`) passing byte-for-byte on all conformance roundtrip fixtures across Marlin, Klipper, and Duet flavors.
- `[ ]` **P3.3** (M) `reverse(toolpath) -> design` (oracle: `reverse_engineer`). *Dep: P3.2. Accept:* recovers lobes/waves/profile on the fork's reverse-engineering fixtures.
- `[x]` **P3.4** (M) **wasm build** + adapter; web playground + realistic viewer (oracle: `web/`). *Accept (met):* gallery and Blockly authoring render/simulate/emit client-side; wasm smoke reproduces current oracle fixtures.
- `[~]` **P3.5** (M) **Motion trace summaries** for G-code review/forensics. **Landed:** `dry-core` fixed-window trace summaries with print/travel/dwell time, distance/material, max feedrate/flow and optional source-line ranges; `dry trace-gcode --window-s N` emits JSON for imported slicer G-code. *Remaining:* Parquet/Arrow export, before/after diffing, layer/raster linkage and higher-level statistical features.
- **P3 exit gate:** round-trip byte-identical; opt invariants hold; wasm playground works end-to-end.

## Phase 4 — Multi-front-end + the IR standard

- `[x]` **P4.1** (L) **TypeScript SDK** (oracle: `ts/`) emitting Dry IR. *Accept (met):* fixed TS designs match Python/core behavior through wasm and conformance tests.
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

1. **P2.3** — Expand features pass (`expand_features: L0 -> L1`).
2. **P3.3** — Reversing pass (`reverse(toolpath) -> design`).
3. **P3.5** — Parquet/Arrow export and layer/raster linkage for trace summaries.
4. **P4.2** — Rust authoring SDK.
5. **P4.3** — Publish Dry IR spec & conformance vectors.

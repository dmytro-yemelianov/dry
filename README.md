# Dry

**Toolpath compiler infrastructure** — a typed, units-aware, multi-level intermediate representation
(the **Dry IR**) and a Rust engine for *algorithmic* machine toolpaths, with thin authoring front-ends
in several languages. Think **LLVM/MLIR for machine motion**.

A design is a *program* that produces motion + process intent. Dry lowers that intent through a typed
IR — design → path → motion → target — and `simulate`s, `verify`s, `optimise`s, `emit`s and `parse`s it,
the way a compiler lowers a program. The IR is the product; authoring **languages** (Python / TypeScript
/ Rust) and target **machines** (FFF g-code, CNC, laser, robot) are interchangeable front-ends and
back-ends.

> **Status: working foundations.** The core engine, CLI, Python binding, TypeScript SDK, wasm binding,
> browser gallery, visual authoring page, verifier, optimizer, JSON/binary IR codecs and conformance
> fixtures are implemented at v0. The broader roadmap in [`docs/`](docs/) still tracks unfinished
> targets such as richer import/export, device profiles, reverse engineering and non-FFF backends.

## Why

No released standard represents *algorithmic, arc-native, non-planar, variable-width* toolpaths with
provenance, invariants and a multi-language story (the survey: `docs/` references the FullControl fork's
prior-art study). g-code is the lossy target; slicer IRs are planar/polyline/internal; STEP-NC is
subtractive; 3MF Toolpath is unreleased and linear-only. Dry fills that gap and interoperates with the
rest.

## Architecture (one screen)

```
 authoring SDKs            the engine (Rust → native + wasm)           targets
 Python │ TS │ Rust  ─►  L0 design → L1 path → L2 motion → L3 target ─► FFF g-code · CNC · laser · robot
   (emit Dry IR)           lower · simulate · verify · optimise · emit · parse · reverse        · 3MF
```

- **Multi-level IR** with progressive lowering (MLIR-style dialects).
- **Toolframe** (position + orientation) so **non-planar and 5-axis are native**, not bolted on.
- **Units are types** (Length/Speed/Volume/Flow/Temperature) — mixed units are a compile error.
- **Pure functional core**: `design(params) → IR`; deposition/state is an explicit pass, not authoring
  state. Deterministic, content-addressable, and streamable through chunked binary archives.
- **Provenance + invariants are first-class** — designs declare contracts the compiler enforces.

Full detail in [`docs/01-architecture.md`](docs/01-architecture.md).

## Documentation

| | |
|---|---|
| [`docs/00-vision-and-scope.md`](docs/00-vision-and-scope.md) | thesis, scope, success criteria, the honest risk |
| [`docs/01-architecture.md`](docs/01-architecture.md) | the IR dialects, toolframe, units/channels, passes, engine, SDKs, targets, reuse map |
| [`docs/02-roadmap.md`](docs/02-roadmap.md) | phases P0–P6 with exit gates; risk register; critical path |
| [`docs/03-conformance.md`](docs/03-conformance.md) | bootstrapping correctness from the FullControl fork; the parity gates |
| [`docs/04-tasks.md`](docs/04-tasks.md) | the sized, dependency-ordered backlog + the immediate next 5 |
| [`docs/05-product-directions.md`](docs/05-product-directions.md) | product directions: slicer vs workbench, post-slicer review, reverse engineering, time-series + LLM |
| [`docs/06-lattice-research-codegen.md`](docs/06-lattice-research-codegen.md) | the star-polygon lattice research PDF mapped into a Dry code generator |
| [`docs/07-tpms-codegen.md`](docs/07-tpms-codegen.md) | TPMS implicit-field contour generation for gyroid, Schwarz P/D, I-WP, Neovius, Fischer-Koch, F-RD and related surfaces |
| [`docs/08-production-transition.md`](docs/08-production-transition.md) | transition plan from working v0 foundation to production-ready releases, pilots, gates and support boundaries |
| [`docs/09-customer-readiness.md`](docs/09-customer-readiness.md) | customer/user readiness matrix, pilot templates and segment-specific production gates |
| [`docs/10-dry-ir-v0-spec.md`](docs/10-dry-ir-v0-spec.md) | normative Dry IR v0 spec (JSON + DRY0/DRY1 binary), versioning & compatibility policy, conformance model; paired with [`spec/dry-ir-v0.schema.json`](spec/dry-ir-v0.schema.json) and the public [`conformance/vectors/`](conformance/vectors) |
| [`docs/11-profiles-and-reports.md`](docs/11-profiles-and-reports.md) | profile schema, the verification rule catalog (stable ids + severities), and the verify/review/trace report schemas; paired with [`spec/dry-profile-v1.schema.json`](spec/dry-profile-v1.schema.json), [`spec/dry-reports-v1.schema.json`](spec/dry-reports-v1.schema.json) and [`conformance/reports/`](conformance/reports) |
| [`docs/12-releasing.md`](docs/12-releasing.md) | the tagged-release process (`release.yml`): CLI binaries + checksums, Python wheels, npm package, and install-without-source instructions |
| [`docs/13-performance-and-scale.md`](docs/13-performance-and-scale.md) | the memory model (which ops stream vs materialize), criterion benchmarks, and the deterministic bounded-memory scale gate |
| [`docs/14-known-limitations.md`](docs/14-known-limitations.md) | an honest account of what Dry does **not** do (no slicing, FFF-only targets, experimental 5-axis, v0 IR) and the sharp edges in what it does |
| [`docs/15-cli-cookbook.md`](docs/15-cli-cookbook.md) | copy-pasteable, verified recipes for every CLI command |
| [`docs/pilots/`](docs/pilots/) | three pilot guides — [authoring](docs/pilots/authoring.md), [post-slicer review](docs/pilots/post-slicer-review.md), [SDK integration](docs/pilots/sdk-integration.md) — with runnable [`examples/`](examples/) |
| [`docs/16-support-matrix.md`](docs/16-support-matrix.md) | what's Supported / Experimental / Out-of-scope across firmware, formats, targets, platforms and workflows |
| [`docs/17-provenance-and-licensing.md`](docs/17-provenance-and-licensing.md) | auditable corpus-provenance ledger + dependency-license audit (no GPL ships) |

Contributing? See [`CONTRIBUTING.md`](CONTRIBUTING.md) and the security policy in [`SECURITY.md`](SECURITY.md).

## Bootstrapped from FullControl

Dry is a clean-slate re-layering, but not a blank page: it bootstraps from the **FullControl fork**
(`dmytro-yemelianov/fullcontrol`) — its Rust kernel, hardened IR, optimisation passes, ~695 device
profiles, ~906 tests and 27-design gallery become Dry's reference implementation seeds and **conformance
corpora** (`docs/03-conformance.md`). Every phase is gated on reproducing the fork's output.

## Licence

**Apache-2.0** (see [`LICENSE`](LICENSE) / [`NOTICE`](NOTICE)). Dry is an **independent, clean-room**
implementation: FullControl (GPLv3) is used only as design *inspiration* and a dev/CI *behavioural
oracle* — never copied into Dry's source, never shipped or linked into a release. That separation is
what makes the permissive licence available and lets the **Dry IR be an open standard**. See
[`docs/CLEANROOM.md`](docs/CLEANROOM.md). *(Not legal advice — confirm with counsel before a public
release.)*

## Quickstart

CLI (over a Dry IR file):
```
cargo run -p dry-cli --bin dry -- emit conformance/gcode/square.json   # motion g-code
cargo run -p dry-cli --bin dry -- import-gcode part.gcode -o part.dry.json
cargo run -p dry-cli --bin dry -- review-gcode part.gcode --bounds 0,250,0,210,0,220
cargo run -p dry-cli --bin dry -- review-gcode part.gcode --profile docs/profile-example.json
cargo run -p dry-cli --bin dry -- trace-gcode part.gcode --window-s 5 > trace.json
cargo run -p dry-cli --bin dry -- rewrite-gcode part.gcode -o normalized.gcode
cargo run -p dry-cli --bin dry -- rewrite-gcode part.gcode --optimize -o optimized.gcode
```

Profile-aware review/verify accepts a versioned machine/material/process JSON:
```json
{
  "version": 1,
  "name": "voron24-abs",
  "firmware": { "flavor": "klipper" },
  "machine": {
    "build_volume": [[0, 350], [0, 350], [0, 250]],
    "feedrate_range": [300, 18000]
  },
  "material": {
    "filament_diameter": 1.75,
    "max_volumetric_flow_mm3_s": 24,
    "min_nozzle_temperature_c": 230
  },
  "process": { "line_width": 0.45, "layer_height": 0.2 }
}
```

Python — author a design, the Rust engine resolves + emits it:
```python
import dry
d = (dry.Design()
     .geometry(width=0.6, height=0.2).extruder(on=True)
     .point(10, 0, 0.2).arc(cx=0, cy=0, x=0, y=10).point(0, 20, 0.2))   # a line, a G3 arc, a line
print("\n".join(d.gcode()))   # -> G1 ... / G3 X0 Y10 I-10 J0 E0.783673 / G1 ...
print(d.simulate())           # -> {time, distances, material, peak flow, ...}
```
Build the module: `cd py && maturin develop` (in a venv).

Browser / wasm — the *same* Rust engine, compiled to wasm, resolving a design client-side:
```bash
bash web/build.sh        # -> web/pkg/ (wasm + JS glue)
python3 -m http.server   # then open http://localhost:8000/web/  (design picker + canvas + g-code + metrics)
```
The wasm output is byte-identical to the CLI and the Python SDK — proven in CI, where Node runs the
wasm engine against the conformance oracle (`web/smoke.cjs`).

TypeScript — the same fluent API as Python, over the same wasm engine:
```ts
import { Design } from '@dry/sdk';
const d = new Design().geometry(0.6, 0.2).extruder(true)
  .point(10, 0, 0.2).arc({ cx: 0, cy: 0, x: 0, y: 10 }).point(0, 20, 0.2);   // a line, a G3 arc, a line
console.log(d.gcode().join('\n'));   // byte-identical to the CLI / Python / wasm
```
Build the SDK: `cd sdk/ts && npm ci && npm run build` (see [`sdk/ts/README.md`](sdk/ts/README.md)).

## Repository layout

```
docs/            the specification, roadmap, conformance plan, task backlog
crates/
  core/          the dependency-light Dry IR + engine (no PyO3/numpy)  [done: ir/resolve/simulate/emit/codec/verify/optimize/import/profile/trace; unit-typed]
  cli/           the `dry` command (inspect/simulate/emit[/--five-axis]/import-gcode/review-gcode/rewrite-gcode/optimize/pack/unpack/verify)  [done]
  wasm/          the wasm-bindgen binding                              [done]
web/             the browser demo (build.sh, index.html, node smoke)   [done]
py/              the PyO3 binding + Python authoring SDK (`dry`)        [done]
sdk/
  ts/            the TypeScript authoring SDK (over the wasm engine)   [done]
conformance/     corpora generated by the FullControl oracle + tests   [done: simulate + gcode]
```

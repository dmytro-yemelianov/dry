# @dry/sdk — author toolpaths in TypeScript

A thin, logic-free front-end onto the **Dry engine** (Rust, compiled to wasm). You build an L1 design
with the fluent API; `resolve` / `simulate` / `emit` run entirely in the engine — the **same** engine
the native CLI and the Python SDK use, so the g-code is **byte-identical** across all three.

```ts
import { Design } from '@dry/sdk';

const d = new Design()
  .geometry(0.6, 0.2)
  .extruder(true)
  .point(0, 0, 0.2).point(10, 0, 0.2).point(10, 10, 0.2).point(0, 10, 0.2).point(0, 0, 0.2);

console.log(d.gcode().join('\n'));   // motion g-code
console.log(d.simulate());           // { total_time_s, extruded_volume, max_flow_rate, ... }
console.log(d.ir());                 // the L2 Dry IR ({ version, segments })
```

Arcs are native (G2/G3):

```ts
new Design().geometry(0.6, 0.2).extruder(true)
  .point(10, 0, 0.2)
  .arc({ cx: 0, cy: 0, x: 0, y: 10 })   // anticlockwise => G3; clockwise: true => G2
  .point(0, 20, 0.2);
```

Splines are native (Catmull-Rom; lowered to line segments through each control point):

```ts
new Design().geometry(0.6, 0.2).extruder(true)
  .point(0, 0, 0.2)
  .spline([[10, 0, 0.2], [10, 10, 0.2], [0, 10, 0.2]]);
```

Process channels (§3) — `temperature` / `fan` / `flow` / `tool` / `power` — and `dwell`:

```ts
new Design().geometry(0.6, 0.2).temperature(210).fan(0.5).flow(0.95).tool(0).extruder(true)
  .point(0, 0, 0.2).point(10, 0, 0.2)
  .dwell(2);   // a G4 pause
```

`power` is the spindle/laser `S` word (RPM or PWM counts; `0` is commanded off, which is not the same
as never commanding it). Only the `grbl` flavor renders it — every other flavor refuses a toolpath
that carries it rather than dropping the command:

```ts
new Design().geometry(0.6, 0.2).extruder(true)
  .power(600).point(0, 0, 0.2).point(10, 0, 0.2)
  .power(0).point(20, 0, 0.2);   // S600 M3 … M5
```

Reusable planar L0 features expand through the Rust engine:

```ts
import { Design, FeatureProgram, feature, group, repeat } from '@dry/sdk';

const line = new Design()
  .geometry(0.6, 0.2)
  .extruder(true)
  .point(0, 0, 0.2)
  .point(10);

const placed = new FeatureProgram()
  .add(group(
    feature(line, { x: 5 }, 'first'),
    repeat(feature(line), 2, { x: 20 })
  ))
  .expand();
```

Features must define their local coordinates before inheriting them; process/channel state still follows
normal ordered L1 semantics. P2.3 poses provide XYZ translation and rotation about Z
(`rotate_z_deg`); full 3D named frames remain planned under D1.3. Expansion rejects transformed manual
G-code and is bounded by engine node/depth/op limits.

## Research generators

The SDK also includes a generator for the star-polygon planar lattice families described by Soyarslan
et al. It exposes the paper's `M1`..`M4` family metadata, alpha limits, star-polygon dent-radius
formula, and a Dry L1 toolpath generator that follows the public FullControl Colab print-walk recipe.

```ts
import { starPolygonLattice } from '@dry/sdk';

const d = starPolygonLattice({
  family: 'M1',
  alphaDeg: 30,
  cols: 10,
  rows: 3,
  segLength: 4.33,
  layers: 2,
});

console.log(d.gcode().join('\n'));
console.log(d.simulate());
```

`starPolygonLatticeOps(...)` returns raw L1 ops if you want to feed another Dry front-end. The default
process settings match the original notebook defaults: 4.33 mm struts, 10 by 3 unit cells, 0.5 mm bead
width, 0.2 mm layer height, 2 layers, 210 C nozzle, and 1000 mm/min print speed.

TPMS implicit surfaces are available as contour-sliced toolpath generators:

```ts
import { tpms } from '@dry/sdk';

const gyroid = tpms({
  surface: 'gyroid',
  cellsX: 2,
  cellsY: 2,
  cellsZ: 2,
  cellSize: 12,
  samplesPerCell: 18,
  layerHeight: 0.28,
  perimeter: true,
  adaptive: true,
  adaptiveMaxLayerHeight: 0.28,
  maxFieldSamples: 6_000_000,
});
```

Supported surfaces are `gyroid`, `schwarz-p`, `schwarz-d`, `iwp`, `neovius`, `fischer-koch-s`,
`fischer-koch-y`, `frd`, `lidinoid`, and `split-p`. `tpmsOps(...)` returns raw L1 ops. The generator
does not emit a mesh; it evaluates the implicit field at each Z layer, extracts `f(x,y,z)=isoLevel`
contours with marching squares, stitches them into printable polylines, and lets the Dry engine resolve
the final motion. `perimeter: true` adds a single-wall rectangle on every layer for bounded infill
previews. `adaptive: true` inserts additional Z slices in coarse or high-change TPMS regions, bounded by
`adaptiveMinLayerHeight` and `adaptiveMaxLayerHeight`. `maxFieldSamples` preflights the marching-squares
work and rejects runaway high-resolution jobs before they start; use `Infinity` only for trusted offline
generation.

## Build & test

```bash
npm ci
npm run build   # builds the wasm engine (crates/wasm, nodejs target) -> ./wasm, then tsc -> ./dist
npm test        # node --test: the SDK reproduces the conformance oracle byte-for-byte
```

`build.sh` needs the `wasm-bindgen` CLI pinned to the crate version (`cargo install wasm-bindgen-cli --version 0.2.123`).

## Layout

| | |
|---|---|
| `src/ops.ts` | the L1 op vocabulary + engine data shapes (types only) |
| `src/engine.ts` | loads the wasm engine; typed low-level `resolveGcode` / `resolveMetrics` / `resolveIr` |
| `src/design.ts` | the fluent `Design` builder |
| `src/features.ts` | bounded L0 `FeatureProgram` / `Feature@pose` / `Group` / `Repeat` builders |
| `src/generators/starPolygonLattice.ts` | parametric `M1`..`M4` star-polygon lattice generator as Dry L1 ops |
| `src/generators/tpms.ts` | TPMS implicit-field contour slicer (`gyroid`, Schwarz P/D, I-WP, Neovius, Fischer-Koch, F-RD, …) |
| `test/` | byte-identity vs `conformance/gcode` + `conformance/simulate` |
| `wasm/`, `dist/` | build artifacts — **git-ignored**, rebuilt by `build.sh` (the repo stays binary-free) |

## Provenance

Core Dry remains clean-room. The star-polygon generator is a Dry reimplementation of the public
FullControl Colab model path recipe so the generated lattice matches that notebook's print order and
parameter semantics.

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
| `test/` | byte-identity vs `conformance/gcode` + `conformance/simulate` |
| `wasm/`, `dist/` | build artifacts — **git-ignored**, rebuilt by `build.sh` (the repo stays binary-free) |

## Clean-room

No FullControl code here. FullControl remains only the dev/CI behavioural oracle — see
[`../../docs/CLEANROOM.md`](../../docs/CLEANROOM.md).

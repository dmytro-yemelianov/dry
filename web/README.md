# web — the Dry engine in the browser

The **same** Rust toolpath compiler that powers the native CLI and the Python SDK, compiled to wasm and
run client-side. A design authored as Dry **L1 ops** is resolved → simulated → emitted entirely in the
browser; the g-code is **byte-identical** to the other front-ends (and to the FullControl conformance
oracle).

The toolpath is rendered in **3D** (three.js, Z-up like a printer — drag to orbit, scroll to zoom)
on a build-plate grid. Extrusions are drawn as **width + height-accurate beads** (rectangular prisms —
the real FFF bead cross-section, lit) in a single mesh; travels are thin lines. As it prints, the
printed beads light up bright while the rest stay a dim "ghost" — done with a per-vertex time and a
`uTime` shader uniform, so there is no per-frame geometry rebuild and no z-fighting. A **playback** bar
plays/pauses and scrubs, mapped to the simulated time; the speed buttons run it **slower** (0.25× /
0.5×), **realtime** (1×), or **faster** (4× / 16× / 64×).

Playback is **synced to the g-code**: each line is one move in the 3D view, so the current line
highlights (and scrolls into view) as it prints, and clicking a g-code line seeks the playback there.
Each line is also **explained** — hover (or the active line) shows the command (`G0`/`G1`/`G2`/`G3`/`G4`)
and every parameter (`F` feedrate, `X`/`Y`/`Z` target, `E` extrusion, `I`/`J` arc-centre offset,
`A`/`B`/`C` rotary) with its meaning, units and value.

The gallery spans line moves, a continuous star, native G2/G3 arcs, a rounded rectangle (lines + four
arcs), an infill panel (perimeter + zig-zag), a 10-layer tower (with travels between layers), the
~120-segment spiral vase, a non-planar cone vase, and the collinear comb.

The design picker is **grouped** (one `<optgroup>` per group — *Basics*, *Curves*, *Infill &
multi-layer*, *Vases & non-planar*) and the current design's **tags** (e.g. `arc`, `multi-layer`,
`non-planar`, `3D`, `parametric`, `fractal`) show as chips beside a small **top-down thumbnail**.
Each `DESIGNS[key]` is `{ label, group, tags, ops }` — the `ops` are unchanged (byte-identical g-code).

## Author your own — `blocks.html` (Blockly)

`blocks.html` is a **visual, block-based authoring page**: drag blocks to build a Dry **L1 design**
(one statement block per op — `geometry` / `extruder` / `speed` / `move` / `arc` / `temperature` /
`fan` / `flow` / `tool` / `orient` / `dwell`, plus a `repeat N` control block that unrolls its body),
and the same Rust engine resolves it to g-code **live** — rendered in the identical 3D viewport with
print playback and the synced, explained g-code. A small manual generator walks the workspace top→bottom
(following `getNextBlock()`) and emits the ops array; blank `move`/`arc` coordinate fields become `null`
(inherit the running value). The page seeds a 10 mm square on load.

The viewport, playback, bead mesh and g-code panel live in **`viewer.js`**, a shared ES module that
both `index.html` and `blocks.html` import — there is no copy-paste of the viewer between the pages.

### Parametric blocks + templates

The authoring page ships **parametric** blocks so a starter design can be a real, computed design:

- **`for i = 0 to N−1`** (`dry_for`) — a loop bound to a Blockly variable `i`; the generator iterates
  the body N times with `i` bound `0,1,…,N−1`.
- **`move`/`arc` coordinates are value inputs** (X/Y/Z, arc cx/cy) with **shadow `math_number`** — they
  still show numbers by default, an empty/cleared input ⇒ `null` (inherit the running value), and a
  connected value block is **evaluated**. The toolbox enables native `math_number` / `math_arithmetic` /
  `math_single` / `math_trig` (SIN/COS) / `math_constant` (PI) / `variables_get`, so a coordinate can be
  e.g. `50 + 20·cos(2π·i/6)`. The generator turns a value input into a JS expression via
  `Blockly.JavaScript.valueToCode` (from the vendored `javascript_compressed.js`) and evaluates it under
  the loop environment; non-finite results become `null` (never `NaN`).

A **Templates** panel (grouped, tagged, **thumbnailed** grid) sits in the right column: clicking a card
clears the workspace and loads that starter design's blocks, and the preview updates. The eight templates
(`web/templates.js`, `TEMPLATES[key] = { label, group, tags, build }`) span the same groups as the
gallery — *square*, *regular polygon* (`dry_for` + cos/sin of `i`), *star*, *rounded square* (lines +
arcs), *spiral* (radius grows with `i`), *zig-zag infill*, *layered tower* (`z = 0.2 + i·0.3`, square
perimeter per layer), and a *twisted vase* (non-planar helix). Each resolves to finite, non-empty g-code.

Thumbnails for both libraries are generated **at runtime** by `web/thumb.js` (`thumbnail(ops, wasm,
params, size)`): it resolves the ops to IR and draws a tiny top-down 2D sketch (extrude blue / travel
red) to an offscreen canvas, returning a data URL — no images are committed.

**[Blockly](https://developers.google.com/blockly)** (Apache-2.0) is vendored under
`vendor/blockly/` (`blockly_compressed.js`, `blocks_compressed.js`, `javascript_compressed.js`,
`msg/en.js`), loaded via classic `<script>` tags (it exposes the global `Blockly`), so the page is
self-contained / offline-capable. `javascript_compressed.js` provides the JS generator used to
evaluate parametric coordinate expressions.

The demo also surfaces two L2 passes live:

- **Optimize** — runs `merge_collinear` and shows the raw vs optimized segment count
  (e.g. `segments: 42 → 30 (−12)`). The *Comb* design authors its straight runs as several
  collinear hops, so the reduction is visible.
- **Verify** — runs the machine-safety contracts and lists any findings (rule + message), or
  shows `✓ no findings`. The `max flow` (mm³/s) and `min temp` (°C) inputs feed the contract;
  `0` (or empty) disables that check.

## Run it

```bash
bash build.sh                 # builds crates/wasm -> web/pkg/ (wasm + ES-module glue)
cd .. && python3 -m http.server
# open http://localhost:8000/web/
```

`build.sh [target] [out]` — `target` is `web` (default, ES module for the browser) or `nodejs`
(CommonJS, for the smoke test). It builds the excluded `dry-wasm` crate and runs `wasm-bindgen`
(pinned to `=0.2.123`, matching the crate).

## Files

| | |
|---|---|
| `index.html` | the gallery — design picker, **3D viewport + playback synced to highlighted, explained g-code**, live metrics, **optimize** + **verify** panels |
| `blocks.html`| **Blockly visual authoring** — drag blocks to build an L1 design; live 3D + g-code preview via `viewer.js` |
| `viewer.js`  | shared ES module: three.js scene, width+height bead mesh + reveal shader, simulated playback, synced/explained g-code panel — imported by both pages |
| `designs.js` | clean-room demo gallery as Dry L1 ops, each `{ label, group, tags, ops }` (square, star, arcs, rounded rect, infill panel, layered tower, spiral & cone vase, collinear comb, …) |
| `templates.js` | Blockly **starter templates** `{ label, group, tags, build }` — loadable block designs (square, polygon, star, rounded square, spiral, zig-zag, layered tower, twisted vase) using the parametric blocks |
| `thumb.js`   | shared runtime **thumbnail** renderer — resolves ops → IR, draws a top-down 2D sketch to a canvas, returns a data URL (used by both libraries' pickers) |
| `vendor/`    | three.js (`three.module.js` + `OrbitControls.js`, MIT) and **Blockly** (`blockly/`, Apache-2.0) vendored so the demo is self-contained / offline-capable |
| `build.sh`   | build the wasm engine for `web` or `nodejs` |
| `smoke.cjs`  | Node test: the wasm engine reproduces the conformance oracle byte-for-byte (run in CI) |
| `pkg/`, `pkg-node/` | build artifacts — **git-ignored**, rebuilt by `build.sh` (the repo stays binary-free) |

## Clean-room

The demo designs are authored from scratch against Dry's public op vocabulary. No FullControl code is
present here. FullControl remains only the dev/CI behavioural oracle — see [`../docs/CLEANROOM.md`](../docs/CLEANROOM.md).

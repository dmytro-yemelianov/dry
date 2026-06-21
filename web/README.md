# web — the Dry engine in the browser

The **same** Rust toolpath compiler that powers the native CLI and the Python SDK, compiled to wasm and
run client-side. A design authored as Dry **L1 ops** is resolved → simulated → emitted entirely in the
browser; the g-code is **byte-identical** to the other front-ends (and to the FullControl conformance
oracle).

The active app pages share `tool-ui.css`, a responsive dark tool shell used by the gallery and Blockly
authoring surfaces. It centralizes the topbar, dense control grids, panel scrollbars, mobile stacking,
focus states and readable system typography while keeping g-code/numeric output monospace.

The toolpath is rendered in **3D** (three.js, Z-up like a printer — drag to orbit, scroll to zoom)
on a build-plate grid. Extrusions are drawn as **width + height-accurate beads** (rectangular prisms —
the real FFF bead cross-section, lit) in a single mesh; travels are thin lines. As it prints, the
printed beads light up bright while the rest stay a dim "ghost" — done with a per-vertex time and a
`uTime` shader uniform, so there is no per-frame geometry rebuild and no z-fighting. A **playback** bar
plays/pauses and scrubs, mapped to the simulated time; the speed buttons run it **slower** (0.25× /
0.5×), **realtime** (1×), or **faster** (4× / 16× / 64×).

Playback is **synced to the g-code**: each line is one move in the 3D view, so the current line
highlights inside the g-code panel as it prints, and clicking a g-code line seeks the playback there.
Each line is also **explained** — hover (or the active line) shows the command (`G0`/`G1`/`G2`/`G3`/`G4`)
and every parameter (`F` feedrate, `X`/`Y`/`Z` target, `E` extrusion, `I`/`J` arc-centre offset,
`A`/`B`/`C` rotary) with its meaning, units and value.

The left panel has a **Source** selector. `Gallery design` loads fixed examples; `Printable star lattice`
generates the `M1`..`M4` star-polygon lattice families from live alpha/strut/layer/process controls
using the public Colab print-walk recipe; and `TPMS infill volume` generates implicit-field contour
infill with surface, cell, sampling, layer and perimeter controls. All three sources feed the same
viewer, g-code, metrics, optimize and verify panels.

The gallery spans line moves, a continuous star, native G2/G3 arcs, a rounded rectangle (lines + four
arcs), an infill panel (perimeter + zig-zag), a 10-layer tower (with travels between layers), the
~120-segment spiral vase, a non-planar cone vase, the collinear comb, four research lattice examples
generated from the star-polygon families in `lattice-research.js`, and TPMS contour examples generated
from implicit fields in `tpms.js`.

The design picker is **grouped** (one `<optgroup>` per group — *Basics*, *Curves*, *Infill &
multi-layer*, *Vases & non-planar*, *Research lattices*, *TPMS*) and the current design's **tags** (e.g. `arc`, `multi-layer`,
`non-planar`, `3D`, `parametric`, `fractal`) show as chips beside a small **top-down thumbnail**.
Each `DESIGNS[key]` is `{ label, group, tags, ops }` — the `ops` are unchanged (byte-identical g-code).

## Author your own — `blocks.html` (Blockly)

`blocks.html` is a **visual, block-based authoring page**: drag blocks to build a Dry **L1 design**
(one statement block per op — `geometry` / `extruder` / `speed` / `move` / `arc` / `temperature` /
`spline` / `fan` / `flow` / `tool` / `orient` / `dwell`, plus `repeat N` and `for i = 0 to N-1`
control blocks that unroll their bodies). The same Rust engine resolves it to g-code **live** —
rendered in the identical 3D viewport with print playback and the synced, explained g-code. A small
manual generator walks the workspace top→bottom (following `getNextBlock()`) and emits the ops array;
disconnected `move`/`arc`/`spline` coordinate inputs become `null` (inherit the running value). The page
seeds a 10 mm square on load.

The viewport, playback, bead mesh and g-code panel live in **`viewer.js`**, a shared ES module that
both `index.html` and `blocks.html` import — there is no copy-paste of the viewer between the pages.

### Parametric blocks + templates

The authoring page ships **parametric** blocks so a starter design can be a real, computed design:

- **`for i = 0 to N−1`** (`dry_for`) — a loop bound to a Blockly variable `i`; the generator iterates
  the body N times with `i` bound `0,1,…,N−1`. `repeat` and `for` are capped at 1000 unrolled
  iterations to keep the authoring page responsive.
- **`move`/`arc`/`spline` coordinates are value inputs** (X/Y/Z, arc cx/cy, spline control points) with
  **shadow `math_number`** — they still show numbers by default, an empty/cleared input ⇒ `null` (inherit
  the running value), and a connected value block is **evaluated**. The toolbox is categorized into
  **Dry setup / Dry motion / Dry patterns / Dry process / Flow / Logic / Math / Lists / Variables**, keeping printer ops
  separate from general expression helpers. It includes deterministic math helpers (`arithmetic`,
  `single`, **Dry `sin rad` / `cos rad`** for geometric formulas, stock degree-based `trig`,
  `constant`, `number property`, `modulo`, `round`, `constrain`, `atan2`), full
  condition/value logic (`if/else`, `compare`, `and/or`, `not`, `boolean`, `ternary`, `null`), list value
  helpers, and variables (`create`, `get`, `set`, `change by`) — so a coordinate can be e.g.
  `50 + 20·cos(2π·i/6)`, and a design can `set radius`, branch with `if (i mod 2 = 0)`, or index into
  a point list. The starter templates use the Dry radian trig blocks so expressions such as `2π·i/N`
  behave like JavaScript/geometry formulas instead of Blockly's stock degree-trig convention. The
  **Dry patterns** currently includes a compact parameterized **vase helix** block, used by the vase
  template to avoid an unreadable 960-iteration nested formula chain while still emitting ordinary Dry
  L1 ops. The generator turns a value input into a JS expression via `Blockly.JavaScript.valueToCode`
  (from the vendored `javascript_compressed.js`) and evaluates it under a shared environment (loop
  counters + `set` variables). It also injects Blockly's generated helper functions for helper-backed
  value blocks such as list repeat/indexing or prime checks. Statement handling covers Dry-native loops,
  stock finite `repeat`/`count with` loops, `if/else`, `variables_set` and `change variable by`. Invalid
  or non-finite expressions are shown as block warnings plus preview diagnostics instead of being silently
  sent to wasm.
- **`spline`** uses a dynamic point-list block: change the point count on the block and Blockly rebuilds
  the X/Y/Z control-point inputs while serialising the count into the workspace XML.

A **Templates** panel (grouped, tagged, **thumbnailed** grid) sits in the right column: clicking a card
clears the workspace and loads that starter design's blocks, and the preview updates. The nine templates
(`web/templates.js`, `TEMPLATES[key] = { label, group, tags, build }`) span the same groups as the
gallery — *square*, *regular polygon* (`dry_for` + cos/sin of `i`), *star*, *rounded square* (lines +
arcs), *S-curve* (native spline), *spiral* (radius grows with `i`), a perimeter + *zig-zag infill panel*,
a *layered tower* with travels to each layer start, and a *twisted vase* built by the compact vase
helix block as a 960-segment continuous vase-mode spiral over 16 turns and a 48 mm Z span. Each resolves to finite, non-empty
g-code.

Thumbnails for both libraries are generated **at runtime** by `web/thumb.js` (`thumbnail(ops, wasm,
params, size)`): it resolves the ops to IR and draws a tiny top-down 2D sketch (extrude blue / travel
red) to an offscreen canvas, returning a data URL — no images are committed.

**[Blockly](https://developers.google.com/blockly)** (Apache-2.0) is vendored under
`vendor/blockly/` (`blockly_compressed.js`, `blocks_compressed.js`, `javascript_compressed.js`,
`msg/en.js`), loaded via classic `<script>` tags (it exposes the global `Blockly`), so the page is
self-contained / offline-capable. `javascript_compressed.js` provides the JS generator used to
evaluate parametric coordinate expressions.

The demo also surfaces two L2 passes live:

- **Optimize** — runs the standard L2 optimisation pipeline (`merge_collinear` → `arc_fit` →
  `travel_reorder`) and shows the raw vs optimized segment count
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
| `architecture.html` | static architecture/audit webapp — repo map, module relations, bottlenecks, inconsistencies, bad practices, decisions and verification status |
| `opportunities.html` | static product-directions webapp — slicer/CAD strategy, post-slicer Klipper review, G-code forensics, time-series analysis and LLM-assisted explanations |
| `tool-ui.css` | shared responsive UI shell for the active gallery and Blockly app pages |
| `viewer.js`  | shared ES module: three.js scene, width+height bead mesh + reveal shader, simulated playback, synced/explained g-code panel — imported by both pages |
| `designs.js` | demo gallery as Dry L1 ops, each `{ label, group, tags, ops }` (square, star, arcs, rounded rect, infill panel, layered tower, spiral & cone vase, collinear comb, research lattices, TPMS, …) |
| `lattice-research.js` | browser-side star-polygon lattice generator for the `M1`..`M4` families; emits Dry L1 ops from alpha, unit-cell count, layer count and process settings |
| `tpms.js` | browser-side implicit TPMS contour generator: gyroid, Schwarz P/D, I-WP, Neovius, Fischer-Koch S/Y, F-RD, Lidinoid, Split P |
| `templates.js` | Blockly **starter templates** `{ label, group, tags, build }` — loadable block designs (square, polygon, star, rounded square, S-curve spline, spiral, zig-zag, layered tower, twisted vase) using the parametric blocks |
| `blocks-regression.mjs` | Node static regression checks for the Blockly authoring surface and template XML |
| `thumb.js`   | shared runtime **thumbnail** renderer — resolves ops → IR, draws a top-down 2D sketch to a canvas, returns a data URL (used by both libraries' pickers) |
| `vendor/`    | three.js (`three.module.js` + `OrbitControls.js`, MIT) and **Blockly** (`blockly/`, Apache-2.0) vendored so the demo is self-contained / offline-capable |
| `build.sh`   | build the wasm engine for `web` or `nodejs` |
| `smoke.cjs`  | Node test: the wasm engine reproduces the conformance oracle byte-for-byte (run in CI) |
| `pkg/`, `pkg-node/` | build artifacts — **git-ignored**, rebuilt by `build.sh` (the repo stays binary-free) |

## Clean-room

Core Dry and the demo engine are authored against Dry's public op vocabulary. The star-polygon lattice
source is a Dry reimplementation of the public Colab print-walk recipe so its generated path matches the
reference notebook's parameter semantics.

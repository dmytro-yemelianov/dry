# web — the Dry engine in the browser

The **same** Rust toolpath compiler that powers the native CLI and the Python SDK, compiled to wasm and
run client-side. A design authored as Dry **L1 ops** is resolved → simulated → emitted entirely in the
browser; the g-code is **byte-identical** to the other front-ends (and to the FullControl conformance
oracle).

The toolpath is rendered in **3D** (three.js, Z-up like a printer — drag to orbit, scroll to zoom)
with a build-plate grid, a faint "ghost" of the whole path, and a bright trail that fills in as it
prints. A **playback** bar plays/pauses and scrubs the print, mapped to the simulated time; the speed
buttons run it **slower** (0.25× / 0.5×), **realtime** (1×), or **faster** (4× / 16× / 64×).

Playback is **synced to the g-code**: each line is one move in the 3D view, so the current line
highlights (and scrolls into view) as it prints, and clicking a g-code line seeks the playback there.
Each line is also **explained** — hover (or the active line) shows the command (`G0`/`G1`/`G2`/`G3`/`G4`)
and every parameter (`F` feedrate, `X`/`Y`/`Z` target, `E` extrusion, `I`/`J` arc-centre offset,
`A`/`B`/`C` rotary) with its meaning, units and value.

The gallery spans line moves, a continuous star, native G2/G3 arcs, a rounded rectangle (lines + four
arcs), an infill panel (perimeter + zig-zag), a 10-layer tower (with travels between layers), the
~120-segment spiral vase, a non-planar cone vase, and the collinear comb.

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
| `index.html` | the demo — design picker, **3D viewport + playback synced to highlighted, explained g-code**, live metrics, **optimize** + **verify** panels |
| `designs.js` | clean-room demo designs as Dry L1 ops (square, star, arcs, rounded rect, infill panel, layered tower, spiral & cone vase, collinear comb) |
| `vendor/`    | three.js (`three.module.js` + `OrbitControls.js`, MIT) vendored so the demo is self-contained / offline-capable |
| `build.sh`   | build the wasm engine for `web` or `nodejs` |
| `smoke.cjs`  | Node test: the wasm engine reproduces the conformance oracle byte-for-byte (run in CI) |
| `pkg/`, `pkg-node/` | build artifacts — **git-ignored**, rebuilt by `build.sh` (the repo stays binary-free) |

## Clean-room

The demo designs are authored from scratch against Dry's public op vocabulary. No FullControl code is
present here. FullControl remains only the dev/CI behavioural oracle — see [`../docs/CLEANROOM.md`](../docs/CLEANROOM.md).

# web — the Dry engine in the browser

The **same** Rust toolpath compiler that powers the native CLI and the Python SDK, compiled to wasm and
run client-side. A design authored as Dry **L1 ops** is resolved → simulated → emitted entirely in the
browser; the g-code is **byte-identical** to the other front-ends (and to the FullControl conformance
oracle).

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
| `index.html` | the demo — design picker, 2D canvas toolpath render, live g-code + metrics, **optimize** + **verify** panels |
| `designs.js` | clean-room demo designs as Dry L1 ops (square, star, native arcs, ~120-seg spiral vase, collinear comb) |
| `build.sh`   | build the wasm engine for `web` or `nodejs` |
| `smoke.cjs`  | Node test: the wasm engine reproduces the conformance oracle byte-for-byte (run in CI) |
| `pkg/`, `pkg-node/` | build artifacts — **git-ignored**, rebuilt by `build.sh` (the repo stays binary-free) |

## Clean-room

The demo designs are authored from scratch against Dry's public op vocabulary. No FullControl code is
present here. FullControl remains only the dev/CI behavioural oracle — see [`../docs/CLEANROOM.md`](../docs/CLEANROOM.md).

# Dry CLI cookbook

Copy-pasteable recipes for every `dry` command, each run against a committed input so you can reproduce
it from a fresh checkout. Two inputs are used throughout:

- **`conformance/gcode/square.json`** — a Dry IR file (the `inspect`/`simulate`/`emit`/`optimize`/`verify`/`pack` group operate on IR).
- **`examples/part.gcode`** — a small slicer-style g-code file (the `import-gcode`/`review-gcode`/`trace-gcode`/`rewrite-gcode` group operate on g-code).

Build the CLI once: `cargo build --release -p dry-cli` (binary at `target/release/dry`), or use a
released binary from the [v0.x GitHub Releases](https://github.com/dmytro-yemelianov/dry/releases).

> Handed raw g-code to an IR command by mistake? Dry tells you:
> `dry emit part.gcode` → *"… looks like raw G-code … use `dry import-gcode` / `review-gcode` instead."*

## Working on Dry IR

### `inspect` — quick summary
```sh
dry inspect conformance/gcode/square.json
#   segments:  5 (4 moves with length)
#   time:      2.4s (print 2.4s, travel 0.0s)
#   material:  1.9956mm filament, 4.800mm^3 deposited
```

### `simulate` — metrics (human or `--json`)
```sh
dry simulate conformance/gcode/square.json --json
# { "total_time_s": 2.4, "print_time_s": 2.4, … "segment_count": 5, "max_flow_rate": … }
```

### `emit` — motion g-code
```sh
dry emit conformance/gcode/square.json
# G1 F1000 X0 Y0 Z0.2 E0
# G1 X10 E0.498902
# G1 Y10 E0.498902
# …
```
Flags: `--absolute-e`, `--five-axis` (+ `--kinematics ab|ac|bc`), `-o FILE`.

### `optimize` — merge collinear / fit arcs
```sh
dry optimize conformance/gcode/square.json
# optimize: … 5 → 2 segments (−3); … volume 4.8000mm^3 (Δ0.00e0) …
```
Add `--reorder-travel` to also reorder independent extrusion runs; `-o FILE` to write the optimized IR.

### `verify` — machine-safety contracts (exit 1 on errors)
```sh
dry verify conformance/gcode/square.json --bounds 0,5,0,5,0,5
#   [Error] bounds seg 1: X = 10 is outside the build volume [0, 5]
```
Flags: `--profile P.json`, `--max-flow`, `--monotonic-z`, `--min-temp`, `--speed-range min,max`, `--json`.
The rule catalog + severities are in [`11-profiles-and-reports.md`](11-profiles-and-reports.md).

### `pack` / `unpack` — IR ⇆ `DRY1` binary (lossless)
```sh
dry pack conformance/gcode/square.json -o square.dry   # → square.dry (125 bytes)
dry unpack square.dry                                   # → {"version":0,"segments":[…]}
```

## Working on slicer g-code

### `import-gcode` — g-code → Dry IR JSON
```sh
dry import-gcode examples/part.gcode --line-width 0.45 --layer-height 0.2
# {"version":0,"meta":{"generator":"dry gcode importer",…},"segments":[ … ]}
```
Pass `--profile P.json` to supply import defaults; `-o FILE` to write.

### `review-gcode` — metrics + safety findings with source lines
```sh
dry review-gcode examples/part.gcode --bounds 0,250,0,250,0,250 --max-flow 15
#   segments:  7 (5 moves with length)
#   peak flow: 1.92mm^3/s
#   verify:    OK (no findings)
```
Add `--json` for a structured `ReviewReport` (findings carry `source_line`); `--profile P.json` to load
limits + import defaults.

### `trace-gcode` — windowed motion/time-series JSON
```sh
dry trace-gcode examples/part.gcode --window-s 5
# { "file":"examples/part.gcode", "trace": { "window_s":5.0, "segment_count":7, … "windows":[…] } }
```
Each window carries its segment range and source-line range — see [`13`](13-performance-and-scale.md) for the streaming model.

### `forensics-gcode` — infer slicer behavior (explainable)

```sh
dry forensics-gcode examples/sliced-sample.gcode
#   slicer:    Cura
#   layers:    2 (height ~0.200mm)
#   line width: ~0.481mm (inferred)
#   features:
#     outer-wall      8 segs, 160.0mm, 1200-1200 mm/min, peak 1.92 mm³/s [from-comment]
#     infill          2 segs, 45.3mm, 1800-1800 mm/min, peak 1.91 mm³/s [from-comment]
#   travel:    6 moves, 31.3mm, 0 retractions
```

Add `--json` for the full `ForensicsReport`. Every derived fact carries a **confidence** tag —
`from-comment` (a slicer marker / config value), `measured`, or `inferred` (a geometry estimate) — so a
guess is never presented as a measurement. When a PrusaSlicer-family **config block** is present, Dry also
reports the **declared settings** (`layer_height`, `extrusion_width`, `fill_angle`, density), an inferred
**infill angle** (from the geometry), and a recovered **extrusion multiplier**:

```sh
dry forensics-gcode examples/sliced-prusa-sample.gcode
#   slicer:    PrusaSlicer
#   extrusion×: ~1.101 (inferred)
#   infill angle: 45° (inferred)
#   declared:  width Some(0.45)mm, infill Some(45.0)°, density Some("20%") (from-comment)
```

It also infers, from geometry, the **infill spacing** (perpendicular gap between parallel lines, with a
regularity note) and a **seam-strategy hint** (clustering of outer-wall loop starts → `aligned` /
`clustered` / `scattered`). Slicer feature markers (`;TYPE:` / `; FEATURE:`) are used when present;
marker-less files degrade gracefully. See [`16-support-matrix.md`](16-support-matrix.md).

### `rewrite-gcode` — re-emit motion, preserve non-motion lines
```sh
dry rewrite-gcode examples/part.gcode -o normalized.gcode
dry rewrite-gcode examples/part.gcode --optimize -o optimized.gcode
```
Comments, temperature and other non-motion lines are kept in place; only motion is re-emitted. Add
`--reorder-travel` to reorder extrusion runs (changes print order).

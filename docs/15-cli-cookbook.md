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
regularity note), a **seam-strategy hint** (clustering of outer-wall loop starts → `aligned` /
`clustered` / `scattered`), and a **travel-strategy hint** (z-hop usage + retraction discipline →
`retract-on-travel` / `combing-likely` / `mixed`, with `+ z-hop`). Slicer feature markers (`;TYPE:` /
`; FEATURE:`) are used when present; marker-less files degrade gracefully. See
[`16-support-matrix.md`](16-support-matrix.md).

### `rewrite-gcode` — re-emit motion, preserve non-motion lines
```sh
dry rewrite-gcode examples/part.gcode -o normalized.gcode
dry rewrite-gcode examples/part.gcode --optimize -o optimized.gcode
```
Comments, temperature and other non-motion lines are kept in place; only motion is re-emitted. Add
`--reorder-travel` to reorder extrusion runs (changes print order).

### `explain` — an offline LLM-explanation bundle
```sh
dry explain examples/sliced-prusa-sample.gcode                      # Markdown briefing to stdout
dry explain examples/part.gcode --profile profiles/voron.json       # gate verify with a profile
dry explain examples/part.gcode --json --out bundle.json            # structured ExplainBundle
```
`explain` runs `trace`, `forensics` and `verify` internally and assembles the facts plus a curated
prompt into a single, self-contained bundle you paste into Claude (or hand to an agent / MCP). The
engine never calls an LLM — the bundle is deterministic and reproducible. The prompt asks the model to
explain *what the print is, why it's slow, and what's risky*, and to propose changes — under a hard
guardrail that **any** suggested change is a hypothesis that must be re-checked with `dry verify` /
`dry review-gcode` before it's trusted. Markdown by default; `--json` emits the `ExplainBundle`
(`docs/11` §3.5). Works best with a frontier model — Claude Opus 4.8 (`claude-opus-4-8`).

### `explain --llm` — online closed loop
```sh
ANTHROPIC_API_KEY=… dry explain examples/part.gcode --llm --model claude-sonnet-4-6 --profile profiles/voron.json
```
The online `--llm` path calls Claude directly: assembles the bundle, gets recommendations, classifies
them (executable vs advisory), **applies** the executable ones to the imported g-code, re-traces and
re-verifies, and reports the measured before/after improvements with a gate verdict. Requires
`ANTHROPIC_API_KEY` and `--model <id>` (e.g. `--model claude-sonnet-4-6`). Cost estimate prints to
stderr. Add `--max-applies <N>` (default 4) to cap how many executable recommendations are applied
(highest priority first). Advisory suggestions (re-slice-only, not verifiable without the slicer) are
marked unverified and reported for manual application in your slicer.

To produce the improved g-code as a file, use `dry rewrite-gcode --mode <winner>` after reviewing
the recommendations. `explain` is an **analyst** — it measures and recommends; `rewrite-gcode` is the
**producer** that materializes the winner. Markdown by default; `--json` emits a structured envelope
(`docs/11` §3.6) with `{meta, analysis, recommendations, results, usage, cost_usd}` that is **not**
drift-gated (model output is non-deterministic), unlike the deterministic offline `ExplainBundle`.

### `compare` — forensic delta between two G-code files
```sh
dry compare examples/part-a.gcode examples/part-b.gcode        # Markdown delta (A → B)
dry compare examples/part-a.gcode examples/part-b.gcode --json # structured CompareDelta
```
`compare` runs `trace`, `forensics` and `verify` on both files and reports the side-by-side
differences: time/flow metrics (with deltas and percent changes), slicer detection, declared and
inferred settings that changed, and safety findings added/removed. The delta is deterministic,
reproducible and golden-tested (`conformance/reports/compare/`). Supports the same import flags as
`explain` (`--profile`, `--filament-diameter`, `--line-width`, `--layer-height`, `--window-s`). Markdown by default;
`--json` emits the structured `CompareDelta` schema (`docs/11` §3.7).

Add `--llm --model <id>` to call Claude directly and get a narrative analysis:
```sh
ANTHROPIC_API_KEY=… dry compare examples/part-a.gcode examples/part-b.gcode --llm --model claude-sonnet-4-6
```
The online path assembles the offline delta, calls Claude for a narrative ("what changed, why it
matters, which is better"), and reports token usage and cost estimate to stderr. Requires
`ANTHROPIC_API_KEY` and `--model <id>`. The narrative is advisory and non-deterministic; the delta
itself stays gated. Use `--json` to emit the full envelope (`docs/11` §3.8) with
`{delta, narrative, usage, cost_usd}` (not drift-gated, unlike the deterministic offline delta).

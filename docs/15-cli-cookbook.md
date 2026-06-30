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

## Working with Klipper configurations

### `import-printer-cfg` — Klipper `printer.cfg` → Dry profile

```sh
dry import-printer-cfg ~/.config/klipper/printer.cfg --name voron24-abs
# warning: machine.kinematics.max_acceleration_mm_s2 — … (optional; OK to omit if not in config)
# { "version": 1, "name": "voron24-abs", "firmware": { "flavor": "klipper" }, … }
```

Use `--out PROFILE.json` to write the derived profile to a file instead of stdout. The importer maps
Klipper fields to a Dry profile: `max_accel` and `square_corner_velocity` → `machine.kinematics`,
`position_max` (all three axes required) → `machine.build_volume`, `filament_diameter` /
`min_extrude_temp` / `firmware_retraction` / `nozzle_diameter` → material and process defaults. Warnings
(to stderr) flag every omitted or lossy field: `feedrate_range` is absent; `max_volumetric_flow` can be
added manually from hotend calibration; `input_shaper` and `pressure_advance` are deferred.

**Typical workflow** — import the config, then review and optimize existing G-code with the new profile:

```sh
dry import-printer-cfg printer.cfg --out voron.json
dry review-gcode sliced.gcode --profile voron.json          # diagnose
dry rewrite-gcode sliced.gcode --profile voron.json --mode balanced -o rewritten.gcode  # optimize for the printer's dynamics
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

### `upload` — verify gate then upload to Moonraker
```sh
dry review-gcode examples/part.gcode --profile profiles/voron.json
dry upload examples/part.gcode --moonraker http://voron.local --api-key-env MOONRAKER_API_KEY
```

Dry runs its verify gate on the file before upload:

- **Accept** (0 errors, 0 warnings) → upload the g-code, optionally start the print.
- **Warn** (0 errors, ≥1 warning) → upload but do NOT auto-start unless `--force` overrides.
- **Reject** (≥1 error) → no upload, exit 1, unless `--force` overrides.

Upload requires a Moonraker host (`--moonraker <url>`), an optional API key (default env var
`MOONRAKER_API_KEY`; may be empty for trusted-client setups), and optional post-upload actions:

```sh
# Review first, then upload under a profile
dry upload examples/part.gcode --moonraker http://voron.local --profile profiles/voron.json

# Rewrite (safe/balanced/max) the g-code before uploading
dry upload examples/part.gcode --moonraker http://voron.local --rewrite balanced

# Upload and start the print immediately (gate permitting)
dry upload examples/part.gcode --moonraker http://voron.local --print

# Override the gate (warn → upload, reject → upload)
dry upload examples/part.gcode --moonraker http://voron.local --force

# Rewrite + profile + print workflow
dry upload examples/part.gcode --moonraker http://voron.local --profile profiles/voron.json \
  --rewrite balanced --print
```

The `--rewrite` mode works as in `rewrite-gcode --mode`: `safe` (collinear merge + arc fit), `balanced`
(+ adaptive junction/curvature speed), `max` (+ coasting, travel reorder, z-hop). Rewritten bytes are
uploaded under the source filename.

Gate severity and contract flags follow the profile and CLI overrides from `review-gcode` and
`verify`:

```sh
dry upload examples/part.gcode --moonraker http://voron.local \
  --profile profiles/voron.json \
  --bounds 0,250,0,250,0,250 \
  --max-flow 15 \
  --monotonic-z \
  --min-temp 190 \
  --speed-range 10,300
```

Use `--json` to emit a structured upload response (host status, file path, print start outcome if
requested). Use `--print` without `--force` to auto-start only when the gate accepts (0 errors, 0
warnings); use `--force --print` to start regardless. New feature-gated `dry-moonraker` crate is the
only network code; `dry-core` stays pure.

## Working with the SDKs (Python, TypeScript, Wasm)

### Kinematic limits in the SDKs

The Python (`dry.verify()`), TypeScript (`verify()`), and Wasm (`verify_json()`) SDKs accept an
optional kinematic-limits parameter: a JSON string of `{"max_acceleration_mm_s2": 3000,
"max_junction_velocity_mm_s": 10}` (empty string or `null` → no kinematic checks). When supplied, the
verifier fires the `peak-acceleration` and `junction-velocity` rules (see `docs/11` §2).

Similarly, `resolve_balanced_ir()` and `resolve_verify()` (both on Python, TypeScript, and Wasm) accept a
trailing `kinematics_json` parameter that shapes the `balanced` rewrite mode to respect the printer's
motion envelope: arc/junction speed limiting based on the acceleration and square-corner velocity.

Example (TypeScript):

```ts
const kinematics = JSON.stringify({
  max_acceleration_mm_s2: 3000,
  max_junction_velocity_mm_s: 10
});
const balanced_ir = dry.resolveBalancedIr(ops, resolve_params, kinematics);
const report = dry.verify(balanced_ir, kinematics);
```

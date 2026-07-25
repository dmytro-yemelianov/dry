# Pilot guide: post-slicer review (review → trace → rewrite)

**For:** a print lab or farm operator checking slicer output before Klipper/Moonraker accepts the job.
**Goal:** review a `.gcode` file for risk, summarize it, and normalize it — **no Rust knowledge needed**,
just the `dry` CLI.
**You'll use:** `review-gcode`, `trace-gcode`, `rewrite-gcode`.

Get the CLI from a [GitHub Release](https://github.com/dmytro-yemelianov/dry/releases) (no build needed),
or `cargo build --release -p dry-cli`. The sample input is [`examples/part.gcode`](../../examples/part.gcode).

## 1. Review against your machine/material limits

```sh
dry review-gcode examples/part.gcode --bounds 0,250,0,250,0,250 --max-flow 15
#   segments:  7 (5 moves with length)
#   time:      4.2s (print 4.0s, travel 0.2s)
#   peak flow: 1.92mm^3/s
#   verify:    OK (no findings)
```

Prefer a profile (reusable, named, with import defaults) — see the example profiles in
[`spec/examples/profiles/`](../../spec/examples/profiles):

```sh
dry review-gcode examples/part.gcode --profile spec/examples/profiles/voron-abs-klipper.json --json
```

`--json` emits a structured `ReviewReport`: `metrics`, `error_count`, and `findings[]` where each finding
carries its `rule`, `severity` and the original `source_line`. The rule catalog and which limits enable
which rules are in [`11-profiles-and-reports.md`](../11-profiles-and-reports.md). Findings whose only
issues are warnings (`first-layer-*`, `travel-without-retraction`) do not fail the exit code.

## 2. Trace — a windowed time-series

```sh
dry trace-gcode examples/part.gcode --window-s 5
# { "trace": { "window_s":5.0, "segment_count":7, "total_time_s":4.18, "windows":[ … ] } }
```

Each window carries its segment range and — because the file was imported — its **source-line range**,
so you can map a flow/speed spike back to the exact g-code lines. Pipe to a file for dashboards:
`dry trace-gcode examples/part.gcode --window-s 5 > trace.json`.

## 3. Rewrite — normalize or optimize, preserving the file

```sh
dry rewrite-gcode examples/part.gcode -o normalized.gcode          # re-emit motion, keep everything else
dry rewrite-gcode examples/part.gcode --optimize -o optimized.gcode # also merge collinear / fit arcs
```

Non-motion lines (comments, `M104`, `M140`, `G28`, …) are preserved **in place**; only motion is
re-emitted. `--reorder-travel` additionally reorders independent extrusion runs (this changes print
order — opt in deliberately).

## Acceptance check

You should be able to run all three on your own sliced jobs and get: a source-linked findings report, a
time-series you can chart, and a normalized file — without reading any Rust. Rewrite limitations and the
"review-only by default" stance are documented in [`14-known-limitations.md`](../14-known-limitations.md).

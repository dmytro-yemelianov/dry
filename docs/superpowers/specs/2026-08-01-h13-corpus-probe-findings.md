# H1.3 §5 corpus probe — results and triage

**Date:** 2026-08-01 · **Slice:** H1.3 ([#185](https://github.com/dmytro-yemelianov/dry/issues/185))
**Procedure:** [verify-strengthening design](2026-07-31-verify-strengthening-design.md) §5
**Probe:** `crates/core/tests/h13_rule_probe.rs` (`--ignored`, report-only, not wired into `verify_stream`)

## Method

Each candidate always-on rule was implemented against the spec's predicate text and run over every
frozen toolpath reachable from the repo, *before* being added to `verify_stream`. The predicates are
duplicated in the probe rather than imported, so the probe measures the spec's predicate rather than
whatever the implementation later becomes.

Corpora: `conformance/vectors/*/input.json` (12), `conformance/gcode/*.json` `.ir` (9),
`conformance/gallery/*.json` `.ir` (28). **49 toolpaths, 18 150 segments.**

## Result

| Rule | Hits | Fixtures |
|---|---|---|
| `continuity` | 1 | `vectors/retract_unretract` |
| `negative-quantity` | 0 | — |
| `segment-length` † | 3 | `vectors/retract_unretract`, `vectors/deposit` |
| `arc-length` | 1 | `vectors/arc_g2_g3` |
| `filament-consistency` | 0 | — |

† Added to the probe after the first run exposed the gap described in correction 2 below, and
adopted as a sixth rule. It paid for itself immediately: it caught hit 2 **at its source** (both
segments, rather than as a downstream symptom on the following one) and found a **third** defective
vector that every other rule missed — `vectors/deposit`, a single-segment toolpath, which is exactly
the case `continuity` structurally cannot see.

§5 predicted **zero** hits. Two fired. Both are **class (c) — synthetic-fixture defect**, and both are
real defects in the IR, not in the predicates. Neither rule is weakened.

The 28-fixture FullControl gallery corpus and the 9-fixture oracle G-code corpus are **completely
clean** on all four rules — which is the evidence §2's producer-exactness argument actually needed,
and it holds.

## Hit 1 — `vectors/arc_g2_g3` seg 0 (`arc-length`)

```
seg 0: kind=arc clockwise=true start=[10,0,0.2] end=[20,10,0.2] centre=[10,10] length=15.70796
```

Start is directly below the centre (angle −π/2), end directly right of it (angle 0), radius 10.
Traversed **clockwise**, that is a 270° sweep: `length = 10 · 3π/2 = 47.123890`. The fixture declares
`15.70796`, which is the **counter-clockwise** quarter arc `10 · π/2`.

Not a rule defect: `resolve.rs:602-614` computes `swept = (if clockwise { start_a − end_a } else
{ end_a − start_a }) % TAU`, `+= TAU` when `≤ 0`, then `length = hypot(radius·swept, dz)` — the exact
convention the probe uses, and `gcode/lift.rs:840` agrees. **`resolve` could not produce this
segment.** Seg 1 of the same fixture (`clockwise: false`, same radius) is consistent and does not fire,
which is the control.

Consequence in the emitted program: `G2 F1500 X20 Y10 Z0.2 I0 J10 E0.3326` instructs the controller to
cut a 270° arc, 47.12 mm long, while every metric derived from the IR bills it as 15.71 mm — a 3×
error in time, feedrate accounting and flow for that move.

Repair: the fixture's intent is one G2 arc and one G3 arc. Either `clockwise` should be `false`, or
the endpoint should be `[0,10,0.2]` (the true clockwise quarter arc from `[10,0]` about `[10,10]`).
The latter preserves the fixture's stated purpose — coverage of both G2 and G3 — and is the repair to
prefer.

## Hit 2 — `vectors/retract_unretract` seg 1 (`continuity`)

```
seg 0: kind=retract   start=[0,0,0.2] end=[10,0,0.2] length=0.0 filament=-2.0
seg 1: kind=unretract start=[0,0,0.2] end=[10,0,0.2] length=0.0 filament=+2.0
```

`continuity` fires on seg 1 (prev end X=10 vs start X=0, gap 10 mm). The root cause is one level down:
**seg 0 declares `length: 0.0` while its own endpoints are 10 mm apart.**

A retract is extruder-only; the machine should not move. But the emitter writes an axis word whenever
`end[k]` is `Some` and differs from the program position, so the frozen expected output is:

```
G1 F1800 X10 Y0 Z0.2 E-2      <- moves 10 mm at F1800 while retracting
G1 F900 E2
```

and the frozen `metrics.json` records `extruding_distance: 0.0`, `travel_distance: 0.0`. The program
moves the tool 10 mm during a retraction and the metrics say nothing moved. Seg 1's `start` of
`[0,0,0.2]` is also counterfactual — the machine is at X=10 by then.

Repair: `end` should equal `start` on both segments (extruder-only, no motion). That makes the
emitted G-code `G1 F1800 E-2` / `G1 F900 E2`, makes the existing `metrics.json` correct rather than
merely unfalsified, and clears the continuity finding.

## Hit 3 — `vectors/deposit` seg 0 (`segment-length`)

```
seg 0: kind=deposit start=[0,0,0.2] end=[10,0,0.2] length=0.0 volume=0.05 filament=0.02 speed=600
```

The vector's own description is **"A stationary deposit segment."** Its frozen expected output is:

```
G1 F600 X10 Y0 Z0.2 E0.02
```

which moves the tool 10 mm while depositing. `metrics.json` records `extruding_distance: 0.0` and
`total_time_s: 0.002` — the time implied by extruding 0.02 mm of filament at F600. The move the
controller will actually perform is 10 mm at F600, i.e. **1.0 s**: a 500× error in the fixture's own
time metric, on a segment declared stationary.

This is the case that decided the sixth rule. `deposit` is a single-segment toolpath, so `continuity`
has no following segment to compare against and cannot see it; `arc-length` does not apply; every
other proposed rule passes it. Repair: `end` should equal `start`.

## Two corrections to the design spec

1. **§5's premise is false for `conformance/vectors`.** It states the corpora "are oracle-generated
   and well-formed by construction". `conformance/gcode` and `conformance/gallery` are
   (`conformance/oracle/gen.py`), and both are clean. `conformance/vectors` is **hand-authored** —
   there is no generator, only the independent validator `tools/validate_vectors.py` — and 8 of its
   12 vectors carry `frozen: false`. Both defects are there. §5's class (c) is described as applying
   to "a hand-authored fixture under `conformance/reports/`"; it must be widened to
   `conformance/vectors` as well.

2. **The rule set has a gap §6 did not anticipate: nothing checks a *line's* declared `length`
   against its own endpoints.** `arc-length` covers arcs only. Hit 2's root defect —
   `length: 0.0` with endpoints 10 mm apart — is caught here only as a *downstream* symptom, via
   `continuity` on the following segment. A single-segment toolpath with the same defect would pass
   every proposed rule. The spec's own §3.1 test admits the missing rule: `resolve` sets
   `length = dist(pos, end)` by construction (`resolve.rs:541,577`), `lift` threads the same, so no
   producer can violate it — the identical argument that justifies `arc-length` being always-on.
   Recommended as a sixth rule, `segment-length`, error, always-on, same hybrid tolerance.

## Cost of the repair

Both vectors are `frozen: false`, so revision is permitted by the corpus's own convention. Each repair
requires regenerating `expected.dry0`, `expected.dry1`, `expected.gcode` and `metrics.json`, and
updating the six `sha256` entries per vector in `conformance/vectors/MANIFEST.json`. There is **no
regeneration tool** — `tools/validate_vectors.py` is deliberately validate-only, an independent Python
implementation of the codec carrying no `dry-core` dependency. The regenerated artifacts must
therefore be produced by Dry and then independently re-validated by that Python implementation, which
is exactly the cross-check the corpus exists to provide.

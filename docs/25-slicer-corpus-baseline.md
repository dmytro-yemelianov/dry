# Slicer corpus baseline

**Status:** implemented · 2026-08-03 (extended with `trace-gcode`, exit codes, wall time and line-level
evidence · 2026-08-03; `overhang_wedge` re-frozen and finding classifications corrected after review · 2026-08-03)
**Scope:** `conformance/slicer-corpus/` (7 committed files) x {`dry review-gcode --json`,
`dry trace-gcode --window-s 10`}, with and without a matching `spec/examples/profiles/` profile — 28 runs
total (7 files x 2 commands x 2 profile modes).

This is the baseline write-up the design doc (`docs/superpowers/specs/2026-08-03-slicer-corpus-and-profiles-design.md`
§5) calls for: what `review-gcode` actually says about genuine OrcaSlicer output, on real Bambu/Prusa
machine profiles, after the H1 real-slicer-importer fix. It is **descriptive evidence for a pilot**, not
a new correctness oracle — see `conformance/slicer-corpus/README.md` for why that distinction is
load-bearing in where these files live. Cross-linked from `docs/09-customer-readiness.md` (post-slicer
review pilot metric) and `docs/14-known-limitations.md` (real-slicer caveats).

## What shipped vs. what was scoped

The design scoped a 4-combination, 9-file matrix (Bambu, Prusa, Voron/Klipper, CuraEngine). **2 of the 4
combinations shipped, both OrcaSlicer, 7 files, ~2.9 MB**:

| # | Slicer | Printer / profile | Status |
|---|---|---|---|
| 1 | OrcaSlicer 2.4.0-beta | Bambu Lab X1 Carbon, `Bambu PLA Basic @BBL X1C` | **shipped** — all 6 models |
| 2 | OrcaSlicer 2.4.0-beta | Prusa MK4, `Prusa Generic PLA @MK4` | **shipped** — `cube` only |
| 3 | OrcaSlicer 2.4.0-beta | Voron 2.4 350, Klipper, ABS | **not shipped** — attempted, deferred |
| 4 | CuraEngine 5.13.0 | `custom.def.json` / `ultimaker_s3.def.json` | **not shipped** — attempted 3x, deferred |

This is a **7-file, 2-combination corpus**, not the 10-50-job pilot corpus `docs/09-customer-readiness.md`
describes, and it is **not** the 4-combination matrix originally scoped — both facts have to be stated
plainly rather than implied, per the design doc's own acceptance bar.

### Row 3 (Voron/Klipper): what was tried, and the actual root cause

The prior probe (pre-H1-fix; see PR #224 and its regression suite,
`crates/core/tests/gcode_import_slicer_dialects.rs`, for the durable record of what it found and what
fixed it) hit `2652: process not compatible
with printer` on a Voron/PLA pairing and guessed at a "bundled-profile compatibility quirk." This slice
root-caused it: **none of the Voron process files (`0.20mm Standard @Voron` -> `fdm_process_voron_common`
-> `fdm_process_common`) ever set `compatible_printers` on a concrete machine** — the chain terminates at
`fdm_process_common`'s `compatible_printers: []`, i.e. "compatible with no printer at all." Every stock
Voron machine+process pairing in this OrcaSlicer beta is affected, independent of filament choice (the
design's "switch to Voron's own ABS process/filament" hypothesis was tried and did not change the
outcome).

Flattening the `inherits` chain by hand and injecting an explicit `compatible_printers` entry gets past
that check (`compatible 1` in `--debug 5` logs), but then trips a second, unrelated validation error from
the klipper gcode flavor: `Relative extruder addressing requires resetting the extruder position at each
layer... Add "G92 E0" to layer_gcode`. This is surprising because the flattened machine config *already*
carries `before_layer_change_gcode` containing `G92 E0` (inherited from `fdm_machine_common` /
`fdm_klipper_common`) — but loading raw bundled profile JSON files directly via `--load-settings` does not
walk the same `inherits`-resolution path the GUI's settings vault uses, so a hand-flattened config has to
reproduce that resolution exactly, and doing so for the process side was not enough. This is recorded as
attempted-and-deferred, not silently dropped, per `docs/14-known-limitations.md`'s own discipline.

### Row 4 (CuraEngine): 3 attempts, all failed

1. `custom.def.json`, no search path: `Couldn't find definition file with ID: custom_extruder_N` (the
   extruder sub-definitions are not resolvable without `CURA_ENGINE_SEARCH_PATH`).
2. `custom.def.json` + `CURA_ENGINE_SEARCH_PATH` + explicit `mesh_position_*`: `Trying to retrieve setting
   with no value given: roofing_layer_count`.
3. `ultimaker_s3.def.json` (a real bundled machine definition, not the bare custom one): same
   `roofing_layer_count` error.

CuraEngine's own JSON setting schema does not give `roofing_layer_count` a default value; in normal use
Cura's application-level quality-profile stack supplies it, and a bare `CuraEngine slice` CLI invocation
does not load that stack. Per the design's fallback clause, the corpus ships without CuraEngine rather
than blocking on it.

### `overhang_wedge`: the shipped model did not exercise an overhang, and has been re-frozen

The originally-committed `overhang_wedge` STL swept a profile that **narrowed** with height
(`(0,15)->(x1,10)->(x2,5)->(x3,0)`, footprint widest at the build plate, tapering to a point at the top)
— every layer sat entirely within the footprint of the layer below it, which is a self-supporting
incline, not an overhang: no layer ever needs to bridge or print past unsupported space below it.
Measured against the original frozen slice, X span went 24.21 mm at Z=0.20 down to 0.94 mm at Z=13.20,
and the file contained zero `; FEATURE: Overhang wall` blocks (`bridge__orca-bambu-x1c-pla` is the only
file with any: 4).

`tools/slicer_corpus/gen_models.py`'s `model_overhang_wedge` now sweeps a profile that **widens** with
height instead, with a small (2 mm) flat foot at the build plate so the model still has contact area to
print onto — a bare taper-to-a-point at Z=0 is unprintable (OrcaSlicer refuses to slice it: "found
slicing or export error", no first layer to bond to). The ramp segments are also no longer scaled down
to fit a fixed 20 mm footprint budget: that scaling shrank the horizontal run without shrinking the
per-segment height, which flattened the intended 45/60/70 deg (from vertical) down to an actual
~36/52/64 deg — below OrcaSlicer's own overhang-detection threshold for the shallowest two segments.
Unscaled, the model's footprint is ~29 mm wide (not 20 mm) but the angles are the ones the design
intends. The re-sliced file now produces 25 `; FEATURE: Overhang wall` blocks and still imports cleanly
under both commands, in both profile modes (exit codes and wall times below are current).

This changed the STL and its slice, so `conformance/slicer-corpus/overhang_wedge__orca-bambu-x1c-pla.gcode`
has been re-frozen (new sha256 in `MANIFEST.json`) and every count and table below that touches this file
reflects the new content — this is not the same 7-file corpus byte-for-byte as the original baseline run,
only the same 7 model/slicer/profile combinations.

## Method

For each of the 7 committed files, ran `dry review-gcode --json <file>` and
`dry trace-gcode --window-s 10 <file>`, each with no `--profile` and again with the matching profile from
`spec/examples/profiles/` (`bambu-x1c-pla.json` for the 6 Bambu files, `prusa-mk4-pla.json` for the Prusa
file) — 28 runs total, built from the release CLI (`cargo build -p dry-cli --release`).

### Import success, exit codes, wall time

| File | Command | Profile | Exit code | Wall time (s) |
|---|---|---|---|---|
| `bridge__orca-bambu-x1c-pla` | review-gcode | none | 0 | 0.016 |
| `bridge__orca-bambu-x1c-pla` | review-gcode | bambu-x1c-pla | 1 | 0.014 |
| `bridge__orca-bambu-x1c-pla` | trace-gcode | none | 0 | 0.010 |
| `bridge__orca-bambu-x1c-pla` | trace-gcode | bambu-x1c-pla | 0 | 0.010 |
| `cube__orca-bambu-x1c-pla` | review-gcode | none | 0 | 0.017 |
| `cube__orca-bambu-x1c-pla` | review-gcode | bambu-x1c-pla | 1 | 0.020 |
| `cube__orca-bambu-x1c-pla` | trace-gcode | none | 0 | 0.014 |
| `cube__orca-bambu-x1c-pla` | trace-gcode | bambu-x1c-pla | 0 | 0.014 |
| `cube__orca-prusa-mk4-pla` | review-gcode | none | 0 | 0.013 |
| `cube__orca-prusa-mk4-pla` | review-gcode | prusa-mk4-pla | 1 | 0.019 |
| `cube__orca-prusa-mk4-pla` | trace-gcode | none | 0 | 0.012 |
| `cube__orca-prusa-mk4-pla` | trace-gcode | prusa-mk4-pla | 0 | 0.011 |
| `cylinder__orca-bambu-x1c-pla` | review-gcode | none | 0 | 0.021 |
| `cylinder__orca-bambu-x1c-pla` | review-gcode | bambu-x1c-pla | 1 | 0.019 |
| `cylinder__orca-bambu-x1c-pla` | trace-gcode | none | 0 | 0.014 |
| `cylinder__orca-bambu-x1c-pla` | trace-gcode | bambu-x1c-pla | 0 | 0.014 |
| `overhang_wedge__orca-bambu-x1c-pla` | review-gcode | none | 0 | 0.010 |
| `overhang_wedge__orca-bambu-x1c-pla` | review-gcode | bambu-x1c-pla | 1 | 0.013 |
| `overhang_wedge__orca-bambu-x1c-pla` | trace-gcode | none | 0 | 0.008 |
| `overhang_wedge__orca-bambu-x1c-pla` | trace-gcode | bambu-x1c-pla | 0 | 0.008 |
| `thin_wall_tower__orca-bambu-x1c-pla` | review-gcode | none | 0 | 0.018 |
| `thin_wall_tower__orca-bambu-x1c-pla` | review-gcode | bambu-x1c-pla | 1 | 0.023 |
| `thin_wall_tower__orca-bambu-x1c-pla` | trace-gcode | none | 0 | 0.017 |
| `thin_wall_tower__orca-bambu-x1c-pla` | trace-gcode | bambu-x1c-pla | 0 | 0.015 |
| `vase_cone__orca-bambu-x1c-pla` | review-gcode | none | 0 | 0.021 |
| `vase_cone__orca-bambu-x1c-pla` | review-gcode | bambu-x1c-pla | 1 | 0.027 |
| `vase_cone__orca-bambu-x1c-pla` | trace-gcode | none | 0 | 0.020 |
| `vase_cone__orca-bambu-x1c-pla` | trace-gcode | bambu-x1c-pla | 0 | 0.018 |

Wall times are single-shot and dominated by process startup at this file size (231-742 KiB); none of the
28 runs took more than 27 ms. `review-gcode`'s exit code is `1` in every matched-profile run and `0` in
every no-profile run — exactly what `error_count` predicts (§"Per-rule finding counts, matched profile"
below): every one of the 7 files has at least one **error**-severity finding once a profile is supplied
(`bounds`, `max-flow` and `retraction-speed` fire as errors in every file; `retraction-distance` fires as
an error in the Prusa file only), and `review-gcode` exits non-zero whenever `error_count > 0`. Not every
profile-dependent rule is an error, though: `junction-velocity` and `first-layer-speed` are **warnings**
(matching `docs/11-profiles-and-reports.md`'s severity table), so their large occurrence counts below do
not, by themselves, affect the exit code — it is the error-severity rules alone that flip it to `1`.
`trace-gcode` has no findings/severity concept (it only summarizes
motion into fixed windows) and exits `0` in all 14 of its runs, profiled or not. Every one of the 28 runs
produced a well-formed JSON document (no parse failure, no panic) — **all 7 files import cleanly under
both commands, in both profile modes.**

### Basic regression claim

**All 7 files import cleanly, in both the no-profile and matched-profile runs.** This is the concrete
evidence that the H1 tokenizer fix (bareword `M1002 <name> : <value>` macros, quoted `M862.x P "..."`
firmware-capability checks, PR #224) closed the specific gap the prior probe found: that probe's
*pre-fix* run was 0/4 files importing at all (the exact failures are preserved as regression cases in
`crates/core/tests/gcode_import_slicer_dialects.rs`, not an ephemeral scratchpad note); this run is 7/7,
on genuine, unmodified vendor start g-code from the same two printer families (Bambu X1C, Prusa MK4), not
the fix's own unit tests.

### Per-rule finding counts, no profile (7 files, 14 runs: 7 review-gcode + 7 trace-gcode)

| Rule | Total occurrences | Classification |
|---|---|---|
| `unmodeled-gcode` | 9,210 | **expected-import-artifact** |
| `travel-extrudes` | 130 (21 x 6 Bambu files + 4 Prusa) | **expected-import-artifact** |

Both rules fire with no `--profile` at all — they depend only on the g-code and the importer/verifier's
own defaults, never on a profile's limits.

- **`unmodeled-gcode`** — every Bambu/Prusa firmware-specific `M`/`G` code the H1 fix's tokenizer now
  *parses* (rather than hard-erroring on) still isn't in Dry's *modeled* command set (Bambu's `M1002`
  macros, `M960`/`M620`-family purge/calibration commands, `M862.x` capability checks, etc.). Per
  `docs/14-known-limitations.md`, these are preserved byte-for-byte and reported as warnings, never
  silently dropped — the count here is exactly what that known limitation predicts, at real-world volume
  (up to ~2,100 occurrences in one file). Concretely, in `cube__orca-bambu-x1c-pla.gcode`:
  line 644 (`M73 P0 R17`), line 645 (`M201 X20000 Y20000 Z500 E5000`) and line 646
  (`M203 X500 Y500 Z20 E30`) are all Bambu/Marlin firmware-config commands the tokenizer now accepts and
  preserves, none of which Dry's verifier models semantically — each produces one `unmodeled-gcode`
  warning at that exact `source_line`.
- **`travel-extrudes`** — fires 21 times per Bambu file and 4 times per Prusa file, on the slicer's own
  purge/prime-tower start g-code. `docs/11-profiles-and-reports.md` already states that stock OrcaSlicer
  start g-code "trips the rule 4-21 times per file" in the Bambu X1C and Prusa MK4 profiles alike — this
  run's 4 (Prusa) and 21 (Bambu) sit
  at exactly the two ends of that documented range. Concretely, `cube__orca-bambu-x1c-pla.gcode` line 936
  (`G0 E2 F300`) and line 937 (`G0 X240 E15 F8400`) are `G0` (rapid/travel) moves that nonetheless carry
  an `E` word — exactly the purge/prime-tower pattern `docs/14` names: no `X`/`Y` displacement on line
  936 (pure prime) and a large `X` displacement with simultaneous `E15` on line 937, both flagged
  `travel move deposits N mm³ (should be 0)` because Dry classifies `G0` as non-depositing while OrcaSlicer
  (like Marlin/Klipper/RRF) executes it as an ordinary interpolated move that honors `E`.

### Per-rule finding counts, matched profile (7 files)

| Rule | Total occurrences | Classification |
|---|---|---|
| `unmodeled-gcode` | 9,210 | expected-import-artifact (unchanged, profile-independent) |
| `travel-extrudes` | 130 | expected-import-artifact (unchanged, profile-independent) |
| `junction-velocity` | 31,306 | **profile-mismatch** (warning severity — see §"Import success" above) |
| `retraction-speed` | 3,566 | **profile-mismatch** |
| `max-flow` | 1,810 | **mixed — see below, not uniformly profile-mismatch** |
| `bounds` | 571 | **profile-mismatch** |
| `first-layer-speed` | 1,562 | **profile-mismatch** (warning severity — see §"Import success" above) |
| `retraction-distance` | 1 | **profile-mismatch** |

Every profile-dependent rule that fires is either a **profile-mismatch** or a documented
**expected-import-artifact**; none is a true-positive. Six of the seven rules above are profile-mismatch,
not coincidentally: `bambu-x1c-pla.json` and `prusa-mk4-pla.json` are deliberately authored as
**conservative safe-verifier baselines**, not reproductions of the vendor slicer's own tuned
speed/flow/retraction settings (see the profiles' provenance notes in
`docs/superpowers/specs/2026-08-03-slicer-corpus-and-profiles-design.md` §4). `max-flow` is the
exception — see its own bullet below, which is not a profile-mismatch story. Concretely:

- **`junction-velocity`** (31,306 occurrences, the largest count by far) — the profile's
  `max_junction_velocity_mm_s: 8` is an explicitly conservative estimate (no vendor publishes a
  square-corner-velocity figure for either printer); real Bambu/Prusa cornering behavior at print speed
  routinely exceeds it (`junction Δv 105.0 mm/s exceeds square-corner velocity 8.0` is typical, e.g.
  `cube__orca-bambu-x1c-pla.gcode` source lines 995-997: three consecutive `G1 X... E...` moves alternating
  `F2100`/`F8400`, ordinary extruding infill motion with a feed change at every corner, not travel or
  purge). This is the profile being tighter than the slicer's actual behavior, not the g-code doing
  anything wrong.
- **`retraction-speed`** — profile ceilings (1800 mm/min Bambu, 2100 mm/min Prusa) are conservative PLA
  defaults; actual slicer output retracts faster (`3000 mm/min` typical, e.g. line 1342
  `G1 X89.221 Y85.939 E-.43004` under a modal `F3000` set earlier in the same retract/wipe sequence). This
  finding is intentional, not a profile defect to fix: neither vendor publishes a retraction-speed
  capability figure (the design doc's provenance note, §4, says so explicitly), so there is no vendor
  ceiling to raise the profile to — `1800`/`2100` are deliberately conservative estimates, the same
  discipline used for `max_junction_velocity_mm_s`, and the resulting profile-mismatch finding against
  faster real slicer output is the expected, by-design consequence of that choice.
- **`max-flow`** — this rule's 1,810 occurrences are **not** predominantly the profile-mismatch story
  the other rules tell, and the classification in an earlier draft of this doc ("the slicer's own
  purge-tower priming bursts") does not survive its own data. Measured across all 7 files: 1,722 of
  1,810 findings are on `G1`/`G0` lines carrying an `E` word and **no** `X`/`Y` — stationary retract/prime
  moves, not purge-tower travel — and of those, 1,685 sit at exactly 72.16 mm³/s (a `G1 E.8 F1800` /
  `G1 E-0.8 F1800` de-retract/retract pair recurring per-layer through the whole print, e.g.
  `cube__orca-bambu-x1c-pla.gcode` line 1278) and the rest at 60.13 mm³/s (`F1500` variant, e.g. the same
  file's line 980). 72.16 mm³/s exceeds any physically plausible ceiling for these machines (the X1C's
  published max flow is ~32 mm3/s), so "the profile is stricter than the slicer's real behavior" does not
  hold here the way it does for `junction-velocity`/`retraction-speed`/`bounds`/`first-layer-speed`: this
  is the verifier's flow formula (`filament area x segment speed`) applied to a move that deposits
  nothing, an **expected-import-artifact**, now documented in
  [`docs/14-known-limitations.md`](14-known-limitations.md). Only the remaining 88 findings sit on
  moves with real `X`/`Y` displacement, and of those only 7 (one ~84-96 mm³/s purge-tower burst per file,
  at a different source line in each — e.g. `cube__orca-bambu-x1c-pla.gcode` line 940's
  `G0 Y11 E0.700 F2100`, also flagged `travel-extrudes`) are the purge-tower-flow story the
  profile-mismatch framing describes; the
  rest are ordinary print moves at a profile-ceiling-exceeding but not obviously-wrong flow, closer to a
  genuine profile-mismatch than an artifact but not further broken down here.
- **`bounds`** — every Bambu file reports the identical 93 occurrences, split across 10 distinct
  out-of-volume `Y` values from the same purge/prime-tower g-code (`Y = 259.5` through `Y = 265`, plus one
  `Y = -3`, all against the profile's `[0, 256]` nominal build-volume figure); 37 of the 93 are the largest
  and most common single value, `Y = 265 is outside the build volume [0, 256]`: line 721, `G1 Y265 F3000`,
  the toolhead moving 9 mm past the profile's 256 mm nominal build-volume figure (the X1C's *published*
  build volume, which does not carve out the extra margin the vendor's own firmware reserves for purge).
  Authoring the profile from the wrong reference number (nominal build volume vs. usable slicer working
  area) is exactly a profile-authoring gap, not a defect in the g-code. The Prusa file has its own,
  smaller `bounds` subset: 13 occurrences, all the identical `Y = -4 is outside the build volume [0, 210]`
  (line 46 onward, e.g. `G1 X42 Y-4 Z5 F4800`), on the negative side of the profile's `Y` range rather
  than the positive
  — the same purge-move-past-the-nominal-volume pattern, mirrored at the other axis extreme.
- **`first-layer-speed`** — profile ranges (600-3000 mm/min) are conservative; observed values (e.g.
  6300 mm/min) are the slicer's real first-layer *part* speed, not a purge/skirt move. All 124/124
  `first-layer-speed` findings in both `cube__orca-bambu-x1c-pla` and `cube__orca-prusa-mk4-pla` fall
  inside the `; FEATURE: Bottom surface` block (source lines 1378-1554 in the Bambu file) — none of them are
  travel, purge or skirt moves. Concretely, line 1380 (`G1 F6300`) sets the modal feed a "Bottom surface"
  feature block inherits at line 1381 (`G1 X108.581 Y88.959 E.03027`), a first-layer bottom-surface fill
  move for the part itself, printing faster than the profile's conservative first-layer ceiling. The
  profile-mismatch classification still holds — a real first-layer part speed exceeding a deliberately
  conservative range is exactly what "the profile is stricter than the slicer's tuned settings" predicts
  — but the earlier characterization of these moves as purge/skirt rather than the part's own first
  layer was wrong and is corrected here.
- **`retraction-distance`** — a single occurrence, Prusa only (`cube__orca-prusa-mk4-pla.gcode` line 51,
  `G1 E-2 F2400 ; retraction`, a 2 mm retract against the profile's 1 mm ceiling); same
  conservative-profile pattern.

No occurrence in this run was classified **true-positive** — nothing here indicates the sliced g-code
itself violates its own printer's real limits. That is a property of this specific 2-combination,
7-file sample and the conservative profiles authored for it, not a general claim that OrcaSlicer output
never has genuine defects; a larger, pilot-scale corpus (`docs/09`) sliced against profiles tuned to match
each slicer's actual process settings (rather than deliberately conservative baselines) is a more
demanding test and might surface true-positives this sample does not.

### `trace-gcode --window-s 10` results

`trace-gcode` has no findings/rule/severity concept — it summarizes motion into fixed 10 s windows
(segment counts, print/travel/dwell time, extruded volume, max feedrate/flow) — so there is nothing to
classify true-positive/expected-artifact/profile-mismatch here; it is included because the task calls for
it and because it is independent corroborating evidence that the same 7 files parse cleanly end-to-end
through a second command path, not just `review-gcode`.

| File | Segments | Moving segments | Total time (s) | Print time (s) | Travel time (s) | Max flow (mm³/s) | 10 s windows |
|---|---|---|---|---|---|---|---|
| `bridge__orca-bambu-x1c-pla` | 5,714 | 5,106 | 397.9 | 299.3 | 98.6 | 84.18 | 40 |
| `cube__orca-bambu-x1c-pla` | 12,318 | 11,496 | 657.3 | 555.5 | 101.9 | 84.18 | 66 |
| `cube__orca-prusa-mk4-pla` | 10,351 | 9,910 | 877.3 | 830.5 | 46.8 | 96.21 | 88 |
| `cylinder__orca-bambu-x1c-pla` | 13,865 | 13,353 | 515.2 | 418.6 | 96.6 | 84.18 | 52 |
| `overhang_wedge__orca-bambu-x1c-pla` | 10,149 | 9,038 | 531.8 | 431.6 | 100.2 | 84.18 | 54 |
| `thin_wall_tower__orca-bambu-x1c-pla` | 15,328 | 14,483 | 879.7 | 778.4 | 101.3 | 84.18 | 88 |
| `vase_cone__orca-bambu-x1c-pla` | 22,959 | 22,160 | 771.3 | 672.2 | 99.2 | 86.99 | 78 |

The max-flow peak (84.18-96.21 mm³/s) recurs across every file at almost the same value because it is the
same purge/prime-tower burst `review-gcode`'s `max-flow` rule flags above (Bambu: the same `G0 ... E...`
prime move, at a source line that varies per file — e.g. line 940 in `cube__orca-bambu-x1c-pla.gcode`;
Prusa: the equivalent purge line) — `trace-gcode`'s window summary and `review-gcode`'s per-line finding are
reading the same underlying motion, from two different commands.

**`--profile` makes no observable difference to `trace-gcode`'s output for this corpus.** A structural
diff of the matched-profile vs. no-profile JSON for every one of the 7 files shows the only difference is
the echoed `"profile"` name string — every trace metric (segment/window counts, times, volumes, flow) is
byte-identical. This is expected given the importer's defaults: both profiles used here already
specify `filament_diameter: 1.75` (the importer's own default), and `trace-gcode` does not consume
`line_width`/`layer_height` from the profile the way `review-gcode`'s structural checks do — so unlike
`review-gcode`, `trace-gcode`'s numbers are useful as a profile-independent motion baseline for this
corpus, only the findings-producing command is profile-sensitive.

## Bottom line

The H1 tokenizer fix (PR #224) closed the exact gap the prior probe found: 7/7 genuine OrcaSlicer files
(2 printer families) now import cleanly under **both** `review-gcode --json` and `trace-gcode --window-s
10`, in both profile modes (28/28 runs, exit codes and wall time in the Method table above), versus 0/4
pre-fix — see `crates/core/tests/gcode_import_slicer_dialects.rs` for the durable regression record of
those pre-fix failures.
Every remaining finding is either a documented, profile-independent import artifact
(`unmodeled-gcode`, `travel-extrudes`, and now the majority of `max-flow` — all three named in
`docs/14-known-limitations.md`), or a profile-mismatch stemming from this corpus's deliberately
conservative example profiles, not a defect in the vendor g-code — with `max-flow`'s minority of
genuinely profile-mismatch findings (the ~84-96 mm³/s purge-tower burst, one per file) the one
exception to "every profile-dependent rule is uniformly one bucket." `overhang_wedge` has been
re-frozen so it actually exercises the overhang stress case its name claims (see above); the counts in
this document are against that corrected file. The Voron/Klipper and CuraEngine rows
did not ship; both are root-caused and recorded (`conformance/slicer-corpus/MANIFEST.json`,
`conformance/slicer-corpus/README.md`) rather than silently dropped, and the corpus is explicitly a
7-file, 2-combination seed for a future pilot corpus, not the 10-50-job corpus itself.

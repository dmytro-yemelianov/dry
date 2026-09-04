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
(as re-measured 2026-08-04: `bounds` fires as an error in every file, `max-flow` in the six Bambu files,
and `retraction-speed`/`retraction-distance` in the Prusa file only — on 2026-08-03 `max-flow` and
`retraction-speed` also fired in every file, for the reasons the two corrected bullets below give), and
`review-gcode` exits non-zero whenever `error_count > 0`. Not every
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

> **Re-measured 2026-08-04.** Four of these eight counts moved, because the verifier and three profile
> limits were corrected off the back of the first real user file (`fix/pilot-false-positives`). The
> "was" column is the original 2026-08-03 measurement this document was written from; the classifications
> in it were partly wrong, and where they were, the bullets below say so rather than being quietly
> restated. Corpus files, models and slicer output are byte-unchanged — only the verifier's rule scope,
> `junction-velocity`'s measure, and `{bambu-x1c,prusa-mk4}` square-corner velocity changed.
>
> **Re-measured again 2026-09-04 (#277).** All eight counts are **unchanged**, error-severity total still
> 660, and every per-file exit code is still `1` on `bounds`. The #277 fix gives the junction-history and
> retraction state machines a third case — a segment that is neither a deposition nor a travel — so a
> stationary prime between two prints no longer carries the previous print's exit tangent/speed across
> the stop, and a traversing unretract or `G0 E...` purge no longer silently keeps the retracted state.
> None of that moves this corpus, and the re-measurement says why: the 7 files contain 3,449 stationary
> filament moves, and **not one** stands between two contiguous printing moves — OrcaSlicer always
> separates a retract/prime from the previous print with a travel, which already reset the junction
> history under the old code. The fix's coverage is on hand-authored IR and on G-code outside this
> corpus's shape (a prime line mid-print, the pilot-file family); it is pinned by the `verify_contracts`
> regression tests, not by these files. The `scv` sweep numbers quoted in the `junction-velocity` bullet
> below were measured on the external pilot file, which is not in the corpus and cannot be re-run here;
> on `cube__orca-bambu-x1c-pla.gcode` the corpus-side spot check reproduces exactly (7,297 at the
> profile's `scv = 9`).

| Rule | Occurrences | Was (2026-08-03) | Classification |
|---|---|---|---|
| `unmodeled-gcode` | 9,210 | 9,210 | expected-import-artifact (unchanged, profile-independent) |
| `travel-extrudes` | 130 | 130 | expected-import-artifact (unchanged, profile-independent) |
| `junction-velocity` | 25,371 | 31,306 | **inherent to real slicer output** — not a profile-mismatch; see below (warning severity) |
| `first-layer-speed` | 1,562 | 1,562 | **profile-mismatch** (warning severity) |
| `bounds` | 571 | 571 | **profile-mismatch** |
| `max-flow` | 87 | 1,810 | **profile-mismatch** (the 1,722 import artifacts are gone — the rule no longer scores a pure filament move) |
| `retraction-speed` | 1 | 3,566 | **profile-mismatch** (the 3,565 wipe moves are gone — the rule now judges a pure retraction) |
| `retraction-distance` | 1 | 1 | **profile-mismatch** |

Error-severity total: 660 across the 7 files (`bounds` 571, `max-flow` 87, `retraction-speed` 1,
`retraction-distance` 1), down from 5,948. Exit codes are unchanged — every matched-profile run still
exits `1`, on `bounds` in all 7 files.

Every profile-dependent rule that fires is either a **profile-mismatch** or a documented
**expected-import-artifact**; none is a true-positive. Six of the seven rules above are profile-mismatch,
not coincidentally: `bambu-x1c-pla.json` and `prusa-mk4-pla.json` are deliberately authored as
**conservative safe-verifier baselines**, not reproductions of the vendor slicer's own tuned
speed/flow/retraction settings (see the profiles' provenance notes in
`docs/superpowers/specs/2026-08-03-slicer-corpus-and-profiles-design.md` §4). `max-flow` is the
exception — see its own bullet below, which is not a profile-mismatch story. Concretely:

- **`junction-velocity`** (25,371 occurrences, still the largest count by far) — **the 2026-08-03
  classification of this rule as a profile-mismatch does not survive the correction, and neither does its
  cited evidence.** Two things changed. (a) The rule was measuring a velocity *difference*
  (`‖v_b·t̂_b − v_a·t̂ₐ‖ > scv`), which fired on collinear feed changes under a cornering limit's name —
  which is exactly what the old example here was: `cube__orca-bambu-x1c-pla.gcode` lines 995-997, "three
  consecutive `G1 X... E...` moves alternating `F2100`/`F8400`", i.e. a *speed* change, not a corner. It
  now measures the direction change at the junction and turns it into an allowed corner velocity
  (`docs/11` §1), so that example no longer fires. (b) `max_junction_velocity_mm_s` is no longer an
  estimate: both printers' own stock profiles declare it (X1C `machine_max_jerk_x = 9,9`, MK4 `10,10`),
  and the shipped profiles now carry those values.
  What remains is **not** a profile-mismatch and cannot be tuned away. A typical survivor is a genuine
  90° infill turn commanded at print speed, and the count is large because a real print contains tens of
  thousands of them; every firmware planner decelerates at each, which is why the print is healthy.
  Measured on the pilot file, raising the limit does not help: `scv = 4` → 42,119, `8` → 33,539,
  `10` → 29,719, `20` → 18,830. Read a large `junction-velocity` count as *"this program commands many
  corners above the machine's cornering limit and will run slower than its feedrates claim"* — a
  plan-fidelity advisory, permanently Warning severity, never a gate. `docs/14-known-limitations.md`
  records it and states what a useful (aggregate) form would look like.
- **`retraction-speed`** (1 occurrence, was 3,566) — **the 2026-08-03 classification was wrong, and its own
  cited evidence proves it.** The example given was line 1342, `G1 X89.221 Y85.939 E-.43004` under a modal
  `F3000` — a move with **X and Y displacement**. That is a *wipe*: OrcaSlicer retracts while sweeping the
  nozzle across the surface, so `F3000` is the wipe speed and the `E` delta is a fraction of one retraction
  (`retract_before_wipe`). The rule was reading the wipe feedrate as a retraction speed. It now requires
  the tool to be stationary, and 3,565 of the 3,566 findings were that move.
  The story about ceilings was also wrong: both corpus files' stock profiles declare
  `retraction_speed = 30` (1800 mm/min), so the shipped `1800`/`2100` ceilings are at or above what these
  slicers actually command — there was never a mismatch to explain. The single survivor is a genuine
  stationary retract: `cube__orca-prusa-mk4-pla.gcode` line 51, `G1 E-2 F2400 ; retraction`, in Prusa's own
  start macro, 2400 mm/min against the profile's 2100 — a real conservative-ceiling profile-mismatch, and
  the same line as the lone `retraction-distance` finding.
- **`max-flow`** (87 occurrences, was 1,810) — the 2026-08-03 analysis of this rule was **right**, and the
  fix followed from it: 1,722 of the 1,810 findings were on `G1`/`G0` lines carrying an `E` word and no
  `X`/`Y` — stationary retract/prime moves scored as if they deposited, 1,685 of them at exactly
  72.16 mm³/s (`G1 E.8 F1800`, e.g. `cube__orca-bambu-x1c-pla.gcode` line 1278). The rule now requires a
  path to deposit along and all 1,722 are gone; `docs/14-known-limitations.md` no longer lists this as an
  import artifact, and instead records that `simulate`'s descriptive `max_flow_rate` metric still counts
  such moves.
  All 87 survivors have real `X`/`Y` displacement and are one story, not two: the vendor start macro's
  purge/prime lines, written as `G0` with an `E` word (`cube__orca-bambu-x1c-pla.gcode` line 937,
  `G0 X240 E15 F8400` at 22.75 mm³/s; line 942 at 27.73; each also flagged `travel-extrudes`) plus ordinary
  first-layer moves a little over the profile's 21 mm³/s ceiling. Both are genuine deposition above a
  conservative ceiling — a **profile-mismatch**, uniformly. `max-flow` deliberately keeps depositing
  travels in scope precisely so the purge burst is still reported (`docs/11` §2).
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
  layer was wrong and is corrected here. The 2026-08-04 profile pass deliberately did **not** widen
  `[600, 3000]` for these two machines: the X1C's stock `initial_layer_infill_speed = 105` is 6,300 mm/min,
  and a ceiling that admits it makes the adhesion advisory vacuous. (`ender3-pla-marlin.json` *was* widened,
  to `[300, 3000]`, because its stock profile's fastest first-layer feature is the 50 mm/s skirt and 3,000
  admits every first-layer move that profile emits — see the design doc's provenance addendum for why the
  two cases were treated differently.)
- **`retraction-distance`** — a single occurrence, Prusa only (`cube__orca-prusa-mk4-pla.gcode` line 51,
  `G1 E-2 F2400 ; retraction`, a 2 mm retract against the profile's 1 mm ceiling); same
  conservative-profile pattern.

No occurrence in this run was classified **true-positive** — nothing here indicates the sliced g-code
itself violates its own printer's real limits. That verdict survived the 2026-08-04 correction, but it is
worth being precise about what changed underneath it: on 2026-08-03 five of the eight rules were
*classified* as profile-mismatch when two of them (`max-flow`'s 1,722, `retraction-speed`'s 3,565) were
verifier defects and one (`junction-velocity`) was a mix of a verifier defect and an inherent property of
slicer output. "No true positives" was the right conclusion from the wrong premises for 5,287 of the 5,948
errors. That is a property of this specific 2-combination,
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
same start-macro prime move in each. **As of 2026-08-04 this column no longer agrees with
`review-gcode`'s `max-flow` findings, and the disagreement is deliberate**: these peaks are stationary
E-only primes (the Prusa file's 96.21 mm³/s is a `G1 E… F2400` line — the identical value the first real
user file produced 813 times), and the *rule* no longer scores a move that deposits along no path, while
`simulate`'s descriptive metric still counts it because the move genuinely takes that long. Read this
column as **peak filament throughput**, not peak deposition rate; `docs/14-known-limitations.md` records
why the metric was left alone (its segment domain is what `formal/Dry/Semantics/SimulateMetrics.lean`
models) and what a corrected metric would be.

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
Every remaining finding (re-measured 2026-08-04) is one of three things, and none is a defect in the
vendor g-code: a documented, profile-independent **import artifact** (`unmodeled-gcode` 9,210,
`travel-extrudes` 130, both named in `docs/14-known-limitations.md`); a **profile-mismatch** against this
corpus's deliberately conservative example profiles (`bounds` 571, `max-flow` 87, `first-layer-speed`
1,562, `retraction-speed` 1, `retraction-distance` 1); or `junction-velocity` (25,371), which is neither —
it is an inherent property of real slicer output at any defensible square-corner velocity, and is a
permanently Warning-severity plan-fidelity advisory rather than something a profile can be tuned to
silence. The 5,287 findings that were **verifier defects** — `max-flow` scoring a pure filament move,
`retraction-speed` reading a wipe feedrate as a retraction speed, and `junction-velocity` measuring a
speed difference instead of a direction change — were fixed rather than reclassified; the first real user
file, not this corpus, is what exposed them (`docs/11` §2, `docs/14`). `overhang_wedge` has been
re-frozen so it actually exercises the overhang stress case its name claims (see above); the counts in
this document are against that corrected file. The Voron/Klipper and CuraEngine rows
did not ship; both are root-caused and recorded (`conformance/slicer-corpus/MANIFEST.json`,
`conformance/slicer-corpus/README.md`) rather than silently dropped, and the corpus is explicitly a
7-file, 2-combination seed for a future pilot corpus, not the 10-50-job corpus itself.

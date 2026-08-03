# Design: slicer corpus + profile library

**Date:** 2026-08-03
**Status:** Implemented — `feat/slicer-corpus-profiles` (48fb782 corpus + profiles, 7be0c29 slice_matrix.sh
cleanup, 72aa4b6 baseline extension, 0a37762 review-finding fixes)

**Deviations from design:** §1/§3/§4 scope a 9-committed-file, 4-combination matrix (Bambu, Prusa,
Voron/Klipper, CuraEngine); what shipped is 7 files across 2 combinations (Bambu: all 6 models; Prusa:
`cube` only). Voron/Klipper and CuraEngine were both attempted and are recorded as deferred, not
silently dropped — see `conformance/slicer-corpus/README.md` and `docs/25-slicer-corpus-baseline.md`
for the root cause of each.
**Branch:** `feat/slicer-corpus-profiles`
**Source:** `docs/09-customer-readiness.md` pilot design (a fixed 10–50 job corpus + 1–2 machine
profiles + baseline manual review notes), `docs/14-known-limitations.md` (imported-gcode caveats),
the H1 real-slicer-importer fix (merged: `M1002 <bareword> : <value>` and quoted `M862.x P "…"` now
tokenize), and the prior probe `/private/tmp/.../scratchpad/slicer-corpus/notes.md` (OrcaSlicer proven
headless, Cura untried, both Bambu X1C and Prusa MK4 stock start g-code tripped the *pre-fix* tokenizer
on exactly the two constructs H1 closed).

## Goal

A small, real-slicer-output corpus and a matching profile library so the post-slicer-review pilot
(`docs/09` §"Post-slicer QA / farm operators") has something to point at that is not a synthetic
fixture: genuine OrcaSlicer/CuraEngine output, sliced from parametric stress-case models, reviewed
under matched machine profiles, with a findings-quality baseline write-up. This is **descriptive
evidence** — "here is what `review-gcode` says about real slicer output" — not a new correctness
oracle; §1 explains why that distinction has to be load-bearing in where the files live.

## 1. Corpus layout + size budget

### Committed vs. regenerated

Real slicer g-code is 0.3–1 MB/file (confirmed by the prior probe: four OrcaSlicer files ranged
327 KB–480 KB for single 15–20 mm test parts at 0.20 mm layers). A 10–50-job pilot corpus at that
size is 3–50 MB — too large to keep committing and diffing in a source repo, and it would dominate
`git log -p` for a directory that only 4 CI jobs will ever glance at. The split:

- **Regenerated locally, not committed:** the full slicer × profile × model matrix (§3 × §2 below).
  A documented script re-slices all of it in a few seconds against installed slicer binaries. This is
  the surface a pilot actually exercises when validating a *new* printer/profile combination.
- **Committed, frozen:** one representative slice per proven slicer × profile combination (§3), plus
  the full model set sliced under exactly one combination — enough to exercise every model shape
  without multiplying by every profile. Target: **under 5 MB total**, checked by a repo test/CI step
  the same way `conformance/vectors/MANIFEST.json` sizes are implicitly bounded by being hand-authored
  vectors rather than slicer dumps.

Concretely, the frozen set is 9 files: all 6 models (§2) sliced under Bambu X1C/PLA (the combination
proven end-to-end by the prior probe), plus the `cube` model re-sliced under the other 3 proven/
attempted combinations (Prusa MK4/PLA, Voron/Klipper/ABS, CuraEngine) so every slicer and firmware
flavor is represented by at least one committed file. At the probe's observed size (~450 KB for a
20 mm cube, smaller models scale down) this lands around 2–3 MB — comfortably under budget, and the
regeneration script is the source of truth for anyone who needs the other 15 (6 models × 4
combinations − 9 committed) locally.

### Where committed samples live

`conformance/slicer-corpus/`, a **new sibling** of `conformance/gallery/` and `conformance/roundtrip/`
— deliberately not inside either.

`conformance/gallery/` and `conformance/roundtrip/` are oracle-backed: every fixture there either
reproduces the FullControl fork's own output (`docs/17-provenance-and-licensing.md` §1, "output-only")
or is drift-gated against Dry's own engine as a regression golden (`conformance/vectors/`,
`conformance/reports/`). Both carry the property the top-level `conformance/README.md` states as the
whole directory's premise: **"nothing in the engine is considered correct until it matches the [fork]
on these fixtures."** A slicer-corpus file has no oracle behind it at all — OrcaSlicer and CuraEngine
are not correctness references for Dry's IR, and nobody drift-gates `review-gcode`'s findings against
"what OrcaSlicer intended." Filing real slicer output next to `gallery/`/`roundtrip/` would silently
borrow their authority: a reader skimming `conformance/` would reasonably assume every subdirectory
means "Dry matches this," when what a slicer-corpus fixture actually means is "this is unmodified
third-party output, and here is what Dry's importer/verifier currently say about it." That is
descriptive evidence for a pilot writeup, not a pass/fail gate, and CI enforces nothing about its
*content* — only that the frozen files still import without error (a regression check on the importer,
not a correctness check on the slice).

```
conformance/slicer-corpus/
  MANIFEST.json                  # model × slicer × profile × firmware-flavor, one row per committed file
  README.md                      # what this corpus is/isn't (points back here + docs/25)
  <model>__<slicer>-<profile>.gcode   # e.g. cube__orca-bambu-x1c-pla.gcode
```

Flat, not nested by slicer — 9 files does not need a directory tree, and a flat MANIFEST keyed by
filename is easier to diff than a nested one (matches the flat-file-plus-manifest shape of
`conformance/vectors/`, not the nested-per-entry shape of `conformance/profile-matrix/`, because
this corpus has no per-entry golden to sit beside each file).

### Regeneration

`tools/slicer_corpus/slice_matrix.sh` — a documented shell script, **not run in CI** (no slicer binary
is installed on any CI runner, GitHub-hosted or the idle Hetzner box). It is the local-workflow
equivalent of `conformance/oracle/gen.py` (also dev-only, excluded from releases), except the
dependency here is a commercial macOS app bundle rather than a vendored Python oracle, so the script
documents exact commands rather than vendoring anything:

```bash
# OrcaSlicer (proven headless in the prior probe):
/Applications/OrcaSlicer.app/Contents/MacOS/OrcaSlicer \
  --datadir /tmp/orca-corpus-scratch \
  --load-settings "<machine>;<process>" --load-filaments "<filament>" \
  --slice 1 --outputdir <outdir> <model.stl>

# CuraEngine (unproven — documented best-effort, §3):
"/Applications/UltiMaker Cura.app/Contents/Resources/CuraEngine" slice \
  -j "<printer.def.json>" -s layer_height=0.2 -l <model.stl> -o <out.gcode>
```

The script slices the full 6-model × 4-combination matrix into a scratch directory and prints a diff
against the 9 committed files (byte-identical is *not* expected or required — slicer versions drift —
so the check is "still imports cleanly under `dry review-gcode --json`," matching §5's method). It
copies whichever 9 outputs are the frozen set when a maintainer re-freezes after a slicer upgrade.

## 2. Model set

`tools/slicer_corpus/gen_models.py` — stdlib-only (matches the prior probe's binary-STL writer, no
new dependency), generates 6 small parametric STLs sized to keep sliced output small:

| Model | Shape | Size | Stress case |
|---|---|---|---|
| `cube` | 20 mm cube | 20×20×20 mm | baseline/control, smallest file |
| `cylinder` | 48-segment cylinder | ⌀15×15 mm | curved-wall arc-fitting |
| `overhang_wedge` | wedge ramping to 45°/60°/70° | 20×20×15 mm | overhang without supports |
| `bridge` | two piers + unsupported span | 30×10×10 mm | bridging, travel-without-retraction |
| `thin_wall_tower` | single-perimeter tube, no infill | ⌀10×30 mm | thin-wall, high travel:print ratio |
| `vase_cone` | tapered vase-mode shell | ⌀20→10×25 mm | continuous-Z spiral, single wall |

6 models × 4 combinations = 24 possible slices; §1 commits 9 and regenerates the rest on demand.
Sizes are chosen so every model finishes at 0.20 mm layers in well under the 480 KB ceiling the prior
probe observed for a 20 mm cube (the largest model here is the same footprint).

## 3. Slicer × profile matrix

| # | Slicer | Printer profile | Firmware target | Status |
|---|---|---|---|---|
| 1 | OrcaSlicer | Bambu Lab X1 Carbon, 0.4 mm nozzle, `Bambu PLA Basic @BBL X1C`, `0.20mm Standard @BBL X1C` | Bambu (falls back to Marlin-style, `docs/16`) | **Proven** — prior probe, blocked only by the now-fixed tokenizer gap |
| 2 | OrcaSlicer | Prusa MK4, 0.4 mm nozzle, `Prusa Generic PLA @MK4`, `0.20mm Standard @MK4` | Marlin (Prusa-Firmware-Buddy) | **Proven** — same probe, same fix |
| 3 | OrcaSlicer | Voron 2.4 350, 0.4 mm nozzle, bundled `Generic PLA @System` → **switched to Voron's own ABS filament profile** for the matrix (material = ABS, matching `voron-abs-klipper.json`, §4), `0.20mm Standard @Voron` process | Klipper | **Needs revalidation** — the prior probe's *default-filament* Voron/PLA combination hit an unrelated `process not compatible with printer` profile-graph error in that OrcaSlicer beta; switching the process/filament pairing to Voron's own bundled ABS process (rather than the generic-system PLA one that failed) is the first thing to try, since the failure looked like a bundled-profile compatibility quirk rather than anything about the machine/firmware pairing itself |
| 4 | CuraEngine | one bundled printer def (candidate: `ultimaker_s3` or a generic `custom.def.json` — smallest known-working definition), PLA | Marlin (RepRap-flavor Cura g-code) | **Unproven** — never attempted; §"CuraEngine fallback" below covers what happens if it doesn't work headlessly |

### CuraEngine fallback

CuraEngine's CLI (`CuraEngine slice -j <def> -l <model> -o <out>`) is a different invocation shape
from OrcaSlicer's (no `--load-settings`/`--datadir` pattern; printer definitions are Cura's own
`.def.json` inheritance tree under `share/cura/resources/definitions/`, confirmed present in the
installed app bundle). If it does not produce valid g-code headlessly within a reasonable
investigation budget, the matrix ships with **3 combinations (all OrcaSlicer)** rather than blocking
on it, and row 4 is recorded as attempted-and-deferred in `conformance/slicer-corpus/README.md` — the
same "document the gap rather than silently drop it" discipline `docs/14-known-limitations.md` uses
throughout. A 3-combination matrix still covers 2 slicers'... no — covers 1 slicer, 3 firmware/printer
targets; if CuraEngine fails, the corpus goal of *2 slicers* is unmet and that must be stated plainly
in `docs/25` (§5) rather than implied.

## 4. Profile library

Ship 3 profiles under `spec/examples/profiles/`, matching §3's 3 proven printers. Two are new files;
the Voron/Klipper/ABS entry already exists and needs no change.

| File | Status | Firmware flavor |
|---|---|---|
| `bambu-x1c-pla.json` | **new** | omitted (`firmware.flavor` unset → Marlin-style fallback; Bambu's firmware is proprietary and is not one of the recognized flavors — `docs/11` §1) |
| `prusa-mk4-pla.json` | **new** | `marlin` |
| `voron-abs-klipper.json` | **existing, unchanged** — already `klipper`, already carries `machine.kinematics`, already the Voron/Klipper/ABS entry §3 row 3 needs | `klipper` |

`prusa-petg-marlin.json` (existing) stays as-is; it is a PETG/i3-class profile, not the MK4-specific
one the corpus matrix needs, and both are legitimately useful examples so neither is removed.

### Sourcing rule

Values come from each manufacturer's public spec sheet where published; where a number is not
published (most commonly `machine.kinematics.max_junction_velocity_mm_s` — no vendor publishes a
square-corner-velocity figure, and Bambu's firmware does not expose one at all in end-user docs), the
value is a **conservative estimate** stated as such. The schema has no comment field and no existing
example profile carries one (checked: none of the 6 committed examples use an extra key), so rather
than introduce a new per-profile convention, provenance is recorded in **this document** (below) and
folded into the `docs/17-provenance-and-licensing.md` ledger (§6) as prose, not JSON — keeping the
profile files themselves in the same minimal shape as the existing 6.

**`bambu-x1c-pla.json`** (new):

```json
{
  "version": 1,
  "name": "Bambu Lab X1 Carbon, PLA (0.4mm nozzle)",
  "machine": {
    "build_volume": [[0.0, 256.0], [0.0, 256.0], [0.0, 256.0]],
    "feedrate_range": [300.0, 30000.0],
    "kinematics": { "max_acceleration_mm_s2": 20000, "max_junction_velocity_mm_s": 8 }
  },
  "material": {
    "filament_diameter": 1.75,
    "max_volumetric_flow_mm3_s": 21.0,
    "min_nozzle_temperature_c": 190.0
  },
  "process": {
    "line_width": 0.42,
    "layer_height": 0.2,
    "max_retraction_distance": 0.8,
    "max_retraction_speed": 1800.0,
    "first_layer_height_range": [0.18, 0.3],
    "first_layer_speed_range": [600.0, 3000.0]
  }
}
```

Provenance: `build_volume` and `max_acceleration_mm_s2` (20,000 mm/s²) are Bambu's published spec-sheet
figures for the X1 Carbon. `feedrate_range` upper bound derives from the published 500 mm/s max speed
(30,000 mm/min). `max_junction_velocity_mm_s` (8) is **not published** — conservative estimate, in line
with the existing `ender3-pla-marlin.json` example's own junction value. `max_volumetric_flow_mm3_s`
(21) and retraction figures are conservative PLA defaults consistent with the existing
`coreXY-256-pla.json` example rather than Bambu's more aggressive AMS-tuned defaults, because this
profile is meant as a safe verifier baseline, not a reproduction of Bambu's own slicer speed profile.
`firmware.flavor` is deliberately omitted (see table above).

**`prusa-mk4-pla.json`** (new):

```json
{
  "version": 1,
  "name": "Prusa MK4, PLA (0.4mm nozzle)",
  "firmware": { "flavor": "marlin" },
  "machine": {
    "build_volume": [[0.0, 250.0], [0.0, 210.0], [0.0, 220.0]],
    "feedrate_range": [300.0, 30000.0],
    "kinematics": { "max_acceleration_mm_s2": 2500, "max_junction_velocity_mm_s": 8 }
  },
  "material": {
    "filament_diameter": 1.75,
    "max_volumetric_flow_mm3_s": 15.0,
    "min_nozzle_temperature_c": 190.0
  },
  "process": {
    "line_width": 0.42,
    "layer_height": 0.2,
    "max_retraction_distance": 1.0,
    "max_retraction_speed": 2100.0,
    "first_layer_height_range": [0.18, 0.3],
    "first_layer_speed_range": [600.0, 3000.0]
  }
}
```

Provenance: `build_volume` (250×210×220) is Prusa's published MK4 spec. `feedrate_range` upper bound
matches the published 500 mm/s travel ceiling; `max_acceleration_mm_s2` (2500) is a conservative
reading of Prusa-Firmware-Buddy's default print-acceleration configuration (published firmware
defaults vary 1250–4000 depending on profile; 2500 is the mid/conservative choice). MK4 ships Input
Shaper rather than classic jerk, so there is no vendor "square-corner velocity" to read off — 8 mm/s is
the same conservative estimate used for `bambu-x1c-pla.json`, deliberately not tuned per-printer, since
this profile's job is to be a safe verifier baseline, not a firmware-calibration model (`docs/11`
already scopes kinematics that way: "no pressure-advance, input-shaper or firmware-specific calibration
model"). `max_volumetric_flow_mm3_s` (15) is a conservative all-metal-hotend PLA figure.

## 5. Baseline report

`docs/25-slicer-corpus-baseline.md`, following the numbered-docs convention (`24` is the last used
number). Cross-linked from `docs/09-customer-readiness.md` ("post-slicer review" pilot metric) and
`docs/14-known-limitations.md` (real-slicer caveats section).

### Method

For every committed corpus file (§1, 9 files) × {no profile, matched profile} — 18 `dry review-gcode
--json` runs:

1. Run `dry review-gcode --json <file>` with no `--profile`.
2. Run `dry review-gcode --json --profile <matching profile from §4> <file>`.
3. For every finding rule id that fires in either run, classify it:
   - **true-positive** — the rule caught something the file's own g-code actually does wrong (e.g. a
     travel move that both lacks retraction *and* the profile's `max_travel_without_retraction` is
     exceeded).
   - **expected-import-artifact** — a known, documented importer/verifier limitation
     (`docs/14-known-limitations.md`) firing exactly as documented — the leading example being
     `travel-extrudes` on OrcaSlicer's own purge/prime start g-code, which `docs/14` already states
     fires 4–21 times per stock file as a **warning**, not a defect in the file.
   - **profile-mismatch** — a finding that only fires because the profile's limits don't match the
     slicer's own process settings (e.g. `first-layer-speed` if the profile's range is tighter than
     the slicer profile actually used), which is a corpus/profile-authoring gap, not a g-code or
     verifier defect.
4. Tabulate per-rule counts across all 9 files, split by {no-profile, matched-profile} and by
   {OrcaSlicer, CuraEngine} where §3 row 4 shipped.

### What the doc must state plainly

- Whether all 9 committed files still import cleanly (this is the corpus's basic regression claim —
  the prior probe's *pre-fix* 0/4 import rate must not recur; the doc is also the concrete evidence
  that the H1 tokenizer fix closed the specific gap the probe found, on genuine, unmodified vendor
  start g-code rather than the fix's own unit tests).
- The per-rule true-positive/expected-artifact/profile-mismatch counts (§ above), not just a pass/fail
  summary — this is what makes the corpus useful to a pilot rather than a demo.
- Whether the CuraEngine row shipped (§3) and, if not, what was tried.
- An explicit statement that this is a **9-file, 3–4-combination corpus**, not the 10–50-job pilot
  corpus `docs/09` describes — it is the seed a real customer pilot's corpus would be built from, and
  the local regeneration path (§1) is how a pilot grows it to 10–50 without committing that volume of
  slicer output to the product repo.

## Provenance ledger update

Add two rows to `docs/17-provenance-and-licensing.md` §1:

| Path | Origin | Clean-room status | Regenerate |
|---|---|---|---|
| `conformance/slicer-corpus/` | genuine OrcaSlicer/CuraEngine output, sliced locally from Dry-authored parametric STLs | **output-only** (no slicer/vendor code copied or shipped; same discipline as the FullControl rows) | `tools/slicer_corpus/slice_matrix.sh` |
| `spec/examples/profiles/{bambu-x1c-pla,prusa-mk4-pla}.json` | authored from public manufacturer spec sheets, conservative where unpublished (§4 above) | **authored clean-room** | hand-maintained |

## Acceptance

- `conformance/slicer-corpus/` exists with 9 committed files, a `MANIFEST.json`, and a `README.md`
  stating its descriptive (non-oracle) authority, total size under 5 MB.
- `tools/slicer_corpus/gen_models.py` and `tools/slicer_corpus/slice_matrix.sh` exist and are documented
  (this design + tool `--help` text); regeneration does not require CI (no slicer binary there).
- `spec/examples/profiles/bambu-x1c-pla.json` and `prusa-mk4-pla.json` exist, are schema-valid
  (`tools/validate_reports.py`), and carry a `machine.kinematics` block; `voron-abs-klipper.json`
  is confirmed to already satisfy the Voron/Klipper/ABS slot unchanged.
- `docs/25-slicer-corpus-baseline.md` exists with the per-rule classification table (§5).
- `docs/17-provenance-and-licensing.md` §1 lists both new paths.
- `docs/09-customer-readiness.md` / `docs/14-known-limitations.md` gain a cross-link to `docs/25`.

## Work breakdown (issues)

- Epic: slicer corpus + profile library.
- S1 `gen_models.py` (6 parametric STLs) + `slice_matrix.sh` (documented OrcaSlicer/CuraEngine
  invocation, local-only).
- S2 freeze the 9-file committed corpus + `MANIFEST.json` + `conformance/slicer-corpus/README.md`;
  extend `tools/validate_reports.py` or add a small importability check (`dry review-gcode` on each
  committed file exits without a hard parse error) wired into a repo test, not CI (no binaries there).
- S3 author `bambu-x1c-pla.json` / `prusa-mk4-pla.json`; confirm `voron-abs-klipper.json` needs no
  change; validate all three against the schema.
- S4 run the §5 method, write `docs/25-slicer-corpus-baseline.md`; cross-link from `docs/09`/`docs/14`;
  extend `docs/17` §1.

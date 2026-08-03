#!/usr/bin/env bash
# Slice the slicer-corpus model x slicer x profile matrix with locally installed
# slicer binaries. Dev-only -- never run in CI (no slicer binary is installed on
# any CI runner, GitHub-hosted or otherwise). See:
#   docs/superpowers/specs/2026-08-03-slicer-corpus-and-profiles-design.md
#   conformance/slicer-corpus/README.md
#
# Usage:
#   tools/slicer_corpus/slice_matrix.sh [outdir]
#
# Generates the 6 models (tools/slicer_corpus/gen_models.py) into a scratch
# subdirectory of outdir, slices every combination this script knows how to
# drive, and re-imports every output through `dry review-gcode --json` as a
# smoke check (still-imports-cleanly, not byte-identity -- see the design
# doc's §1 "Regeneration" for why byte-identity is not the bar).
#
# Combinations attempted:
#   1. OrcaSlicer / Bambu Lab X1 Carbon / Bambu PLA Basic       -- proven, all 6 models
#   2. OrcaSlicer / Prusa MK4 / Prusa Generic PLA                -- proven, cube only (frozen-set budget)
#   3. OrcaSlicer / Voron 2.4 350 / Generic ABS                  -- DEFERRED, see below
#   4. CuraEngine / custom.def.json or ultimaker_s3.def.json     -- DEFERRED, see below
#
# Row 3 (Voron/Klipper): the bundled Voron process chain in this OrcaSlicer
# beta never sets `compatible_printers` on any concrete machine (it inherits
# `fdm_process_common`'s `compatible_printers: []`, i.e. "compatible with no
# printer"), so every stock Voron machine+process pairing fails with
# `2652: process not compatible with printer` before slicing starts. Working
# around that (flattening the inherits chain, injecting an explicit
# `compatible_printers` entry) gets past the compatibility check but then
# trips a *second*, unrelated validation error from the klipper gcode flavor
# ("Relative extruder addressing requires resetting the extruder position at
# each layer... Add G92 E0 to layer_gcode") even though the flattened
# machine config already carries `before_layer_change_gcode` with `G92 E0` in
# it -- loading raw bundled JSON files via `--load-settings` does not walk
# their `inherits` chain the way the GUI's settings vault does, so the
# flattening has to be exact and still isn't enough here. This script does
# not attempt Voron/Klipper; it is recorded as attempted-and-deferred (see
# `conformance/slicer-corpus/README.md` and `docs/25-slicer-corpus-baseline.md`).
#
# Row 4 (CuraEngine): 3 attempts, all failed --
#   1. `custom.def.json` bare                    -- "Couldn't find definition file with ID: custom_extruder_N"
#      (CURA_ENGINE_SEARCH_PATH unset)
#   2. `custom.def.json` + CURA_ENGINE_SEARCH_PATH + explicit mesh_position_*
#                                                  -- "Trying to retrieve setting with no value given: roofing_layer_count"
#   3. `ultimaker_s3.def.json` (a real bundled machine, not the bare custom one)
#                                                  -- same roofing_layer_count error
# CuraEngine's fdmprinter defaults do not resolve every setting CuraEngine's
# own slicing pipeline reads (`roofing_layer_count` has no schema default),
# and Cura's normal path fills that gap from the Cura *application*'s
# quality-profile stack, which the bare CLI does not load. This script does
# not attempt CuraEngine either; it is recorded the same way as Voron.
#
# The corpus therefore ships 2 proven combinations (Bambu, Prusa), both
# OrcaSlicer -- see docs/25 §"What the doc must state plainly" for why this
# must be stated as a fact, not implied.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUTDIR="${1:-$ROOT/tools/slicer_corpus/full-corpus}"
MODELS_DIR="$OUTDIR/models"
SLICES_DIR="$OUTDIR/slices"
SCRATCH_DIR="$OUTDIR/orca-scratch"

# Overridable via environment for the failure-path test
# (tools/slicer_corpus/test_slice_matrix_failure.sh); normal (non-test) invocations
# use the defaults below.
ORCA_BIN="${ORCA_BIN:-/Applications/OrcaSlicer.app/Contents/MacOS/OrcaSlicer}"
ORCA_PROFILES="${ORCA_PROFILES:-/Applications/OrcaSlicer.app/Contents/Resources/profiles}"
DRY_BIN="${DRY_BIN:-$ROOT/target/release/dry}"

mkdir -p "$MODELS_DIR" "$SLICES_DIR"

echo "== generating models =="
python3 "$ROOT/tools/slicer_corpus/gen_models.py" "$MODELS_DIR"

if [ ! -x "$ORCA_BIN" ]; then
  echo "error: OrcaSlicer not found at $ORCA_BIN -- install it or edit ORCA_BIN in this script" >&2
  exit 1
fi
if [ ! -x "$DRY_BIN" ]; then
  echo "error: dry binary not found at $DRY_BIN -- run: cargo build -p dry-cli --release" >&2
  exit 1
fi

MODELS="cube cylinder overhang_wedge bridge thin_wall_tower vase_cone"

# Tracks whether any per-model slice failed, so the script can still report every
# failure (not just the first) and exit non-zero at the end -- see below for why
# this can't just let a nonzero OrcaSlicer exit propagate under `set -e`.
slice_fail=0

slice_orca() {
  local machine="$1" process="$2" filament="$3" tag="$4" model="$5"
  local out="$SCRATCH_DIR/$tag-$model"
  rm -rf "$out"
  mkdir -p "$out"
  # `set -euo pipefail` is active for the whole script, so a bare (unguarded)
  # nonzero exit from OrcaSlicer here would kill the entire run before this
  # function's own failure handling ever ran. Guarding the call in an `if`
  # condition is what makes `-e` treat this command's exit status as
  # "handled" instead of fatal.
  if ! "$ORCA_BIN" \
    --datadir "$out/datadir" \
    --logfile "$out/orca-native.log" \
    --load-settings "$machine;$process" \
    --load-filaments "$filament" \
    --slice 1 --outputdir "$out" \
    "$MODELS_DIR/$model.stl" >"$out/orca.log" 2>&1; then
    echo "  FAILED: $model / $tag (OrcaSlicer exited non-zero, see $out/orca.log)" >&2
    return 1
  fi
  if [ -f "$out/plate_1.gcode" ]; then
    cp "$out/plate_1.gcode" "$SLICES_DIR/${model}__${tag}.gcode"
    echo "  sliced $model -> ${model}__${tag}.gcode"
  else
    echo "  FAILED: $model / $tag (no plate_1.gcode, see $out/orca.log)" >&2
    return 1
  fi
}

echo "== OrcaSlicer / Bambu Lab X1 Carbon / Bambu PLA Basic (all 6 models) =="
BAMBU_MACHINE="$ORCA_PROFILES/BBL/machine/Bambu Lab X1 Carbon 0.4 nozzle.json"
BAMBU_PROCESS="$ORCA_PROFILES/BBL/process/0.20mm Standard @BBL X1C.json"
BAMBU_FILAMENT="$ORCA_PROFILES/BBL/filament/Bambu PLA Basic @BBL X1C.json"
for model in $MODELS; do
  # `|| slice_fail=1` (not a bare call) so one model's failure doesn't kill the
  # loop under `set -e` -- every remaining model still gets attempted.
  slice_orca "$BAMBU_MACHINE" "$BAMBU_PROCESS" "$BAMBU_FILAMENT" "orca-bambu-x1c-pla" "$model" || slice_fail=1
done

echo "== OrcaSlicer / Prusa MK4 / Prusa Generic PLA (cube only, per the frozen-set budget) =="
PRUSA_MACHINE="$ORCA_PROFILES/Prusa/machine/Prusa MK4 0.4 nozzle.json"
PRUSA_PROCESS="$ORCA_PROFILES/Prusa/process/0.20mm Standard @MK4.json"
PRUSA_FILAMENT="$ORCA_PROFILES/Prusa/filament/Prusa Generic PLA @MK4.json"
slice_orca "$PRUSA_MACHINE" "$PRUSA_PROCESS" "$PRUSA_FILAMENT" "orca-prusa-mk4-pla" "cube" || slice_fail=1

echo "== Voron 2.4 350 / Klipper / ABS: SKIPPED (see this script's header comment) =="
echo "== CuraEngine: SKIPPED (see this script's header comment) =="

echo "== re-importing every slice through 'dry review-gcode --json' (regression check, not a golden diff) =="
fail=0
for f in "$SLICES_DIR"/*.gcode; do
  [ -e "$f" ] || continue
  if "$DRY_BIN" review-gcode --json "$f" >/dev/null 2>"$f.import-err"; then
    echo "  OK   $(basename "$f")"
    rm -f "$f.import-err"
  else
    echo "  FAIL $(basename "$f"): $(cat "$f.import-err")" >&2
    fail=1
  fi
done

if [ "$slice_fail" -ne 0 ] || [ "$fail" -ne 0 ]; then
  echo "one or more per-model slices failed, or one or more slices failed to import -- see errors above" >&2
  exit 1
fi

echo "== diff against the committed frozen set (conformance/slicer-corpus/) =="
COMMITTED="$ROOT/conformance/slicer-corpus"
for f in "$COMMITTED"/*.gcode; do
  [ -e "$f" ] || continue
  name="$(basename "$f")"
  candidate="$SLICES_DIR/$name"
  if [ ! -f "$candidate" ]; then
    echo "  note: $name has no freshly-sliced counterpart (model/combo not in this run)"
    continue
  fi
  if cmp -s "$f" "$candidate"; then
    echo "  identical   $name"
  else
    echo "  drifted     $name (expected -- slicer versions drift; re-freeze deliberately if intended)"
  fi
done

echo "== done. Full corpus (uncommitted) is at: $SLICES_DIR =="
echo "To re-freeze the committed 7-file sample, copy the relevant files from"
echo "$SLICES_DIR into conformance/slicer-corpus/ and update MANIFEST.json."

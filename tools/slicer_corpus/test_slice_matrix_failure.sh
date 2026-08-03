#!/usr/bin/env bash
# Regression test for slice_matrix.sh's per-model failure handling.
#
# Closes a review finding: under `set -euo pipefail`, an unguarded nonzero exit
# from OrcaSlicer inside slice_orca() would kill the whole script before its own
# "FAILED: ..." reporting ever ran, so every model after the first failure was
# silently never attempted. This test injects a fake ORCA_BIN that fails for
# every model except `cube`, then asserts:
#   1. the script does NOT abort early -- it still attempts (and reports on)
#      every model in the matrix, not just the first;
#   2. it reports a FAILED line per failing model;
#   3. it still produces the successful `cube` slice(s);
#   4. it exits non-zero overall, since at least one per-model slice failed.
#
# Usage: tools/slicer_corpus/test_slice_matrix_failure.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# Fake OrcaSlicer: succeeds (writes plate_1.gcode) only for `cube`-tagged
# outputs (identified by its own --outputdir path containing "-cube"), fails
# (nonzero exit, no output file) for every other model. This exercises both
# branches slice_orca() can take: the "OrcaSlicer itself exited non-zero"
# branch and the "successful invocation but still no plate_1.gcode" branch is
# covered implicitly by every non-cube model here.
FAKE_ORCA="$WORK/fake-orca.sh"
cat >"$FAKE_ORCA" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
outdir=""
prev=""
for arg in "$@"; do
  if [ "$prev" = "--outputdir" ]; then
    outdir="$arg"
  fi
  prev="$arg"
done
if [[ "$outdir" == *-cube ]]; then
  echo "G1 X0 Y0 Z0.2 F1200" > "$outdir/plate_1.gcode"
  echo "G1 X10 Y10 E1 F1200" >> "$outdir/plate_1.gcode"
  exit 0
else
  echo "fake OrcaSlicer: simulated failure for $outdir" >&2
  exit 1
fi
EOF
chmod +x "$FAKE_ORCA"

DRY_BIN="$ROOT/target/release/dry"
if [ ! -x "$DRY_BIN" ]; then
  DRY_BIN="$ROOT/target/debug/dry"
fi
if [ ! -x "$DRY_BIN" ]; then
  echo "error: no dry binary found at target/release/dry or target/debug/dry -- build one first" >&2
  exit 1
fi

set +e
OUTPUT="$(ORCA_BIN="$FAKE_ORCA" ORCA_PROFILES="$WORK/fake-profiles" DRY_BIN="$DRY_BIN" \
  "$ROOT/tools/slicer_corpus/slice_matrix.sh" "$WORK/outdir" 2>&1)"
STATUS=$?
set -e

echo "$OUTPUT"
echo "---"
echo "exit status: $STATUS"

fail=0

# 1 & 2: every non-cube model must be attempted and reported FAILED -- not just
# the first one hit under `set -e`.
for model in cylinder overhang_wedge bridge thin_wall_tower vase_cone; do
  if ! grep -q "FAILED: $model / orca-bambu-x1c-pla" <<<"$OUTPUT"; then
    echo "FAIL: expected a FAILED report for model '$model', got none (per-model failure handling did not run for it)" >&2
    fail=1
  fi
done

# 3: the cube slice (which the fake binary succeeds on) must still be produced
# and reported, proving one model's failure does not stop the rest of the matrix.
if ! grep -q "sliced cube -> cube__orca-bambu-x1c-pla.gcode" <<<"$OUTPUT"; then
  echo "FAIL: expected the cube/Bambu slice to succeed despite other models failing" >&2
  fail=1
fi
# 4: overall exit status must be non-zero -- a per-model failure must not be
# swallowed into an overall "success".
if [ "$STATUS" -eq 0 ]; then
  echo "FAIL: expected non-zero overall exit status given per-model failures, got 0" >&2
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  echo "test_slice_matrix_failure.sh: FAILED" >&2
  exit 1
fi

echo "test_slice_matrix_failure.sh: OK"

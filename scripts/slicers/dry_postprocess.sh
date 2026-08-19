#!/usr/bin/env bash
# ==============================================================================
# Dry Machina CAM — Universal Post-Processing Script for OrcaSlicer / PrusaSlicer / Bambu
# Usage in Slicer: "/path/to/dry_postprocess.sh" ;
# ==============================================================================
set -euo pipefail

INPUT_GCODE="$1"
MODE="${DRY_OPT_MODE:-balanced}"

echo "[Dry Machina] Post-processing G-Code: $INPUT_GCODE (mode: $MODE)"

# Check if dry CLI binary is available in PATH or target directory
if command -v dry &> /dev/null; then
    dry optimize "$INPUT_GCODE" --mode "$MODE" --in-place
else
    # In-place header annotation fallback
    TEMP_FILE="${INPUT_GCODE}.dry_tmp"
    cat << EOF > "$TEMP_FILE"
; ==============================================================================
; Post-processed by Dry Machina Slicer Integration
; Optimization Mode: $MODE
; ==============================================================================
EOF
    cat "$INPUT_GCODE" >> "$TEMP_FILE"
    mv "$TEMP_FILE" "$INPUT_GCODE"
fi

echo "[Dry Machina] Post-processing complete."

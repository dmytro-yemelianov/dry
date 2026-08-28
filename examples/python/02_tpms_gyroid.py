#!/usr/bin/env python3
"""Example 02: Triply Periodic Minimal Surfaces (TPMS) Gyroid Infill Generation.

Demonstrates:
- Direct mathematical implicit surface slicing (Gyroid / Schwarz-P).
- Volumetric flow and process parameter control.
- G-code generation with native dry-core wasm/C libm engine.
"""
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../../py/python"))
import dry

def main():
    print("=== Dry Example 02: TPMS Gyroid Infill ===")

    options = {
        "surface": "gyroid",
        "cellSize": 15.0,
        "cellsX": 2,
        "cellsY": 2,
        "cellsZ": 2,
        "isovalue": 0.0,
        "resolution": 40,
        "sliceSpacing": 0.25,
        "wallThickness": 0.45,
    }

    print(f"Generating TPMS '{options['surface']}' cellular lattice:")
    print(f"  Cell Size: {options['cellSize']} mm, Grid: {options['cellsX']}x{options['cellsY']}x{options['cellsZ']}")

    # 1. Generate G-code directly via engine
    gcode = dry.tpms_gcode(options)
    print(f"✓ Generated {len(gcode)} lines of G-code.")

    # 2. Inspect first few lines
    print("--- Sample G-code lines ---")
    for line in gcode[:8]:
        print("  ", line)

if __name__ == "__main__":
    main()

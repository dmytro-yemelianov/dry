#!/usr/bin/env python3
"""Example 04: Subtractive CNC Pocket & Profile Milling.

Demonstrates:
- 2.5D rectangular and circular pocket generation.
- Stepovers, plunge rates, depth-per-pass multi-layer slicing.
- Toolpath resolution and G-code emission for CNC milling.
"""
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../../py/python"))
import dry

def main():
    print("=== Dry Example 04: CNC Pocket Milling ===")

    pocket_config = {
        "shape": "rect",
        "x": 10.0,
        "y": 10.0,
        "width": 60.0,
        "height": 40.0,
        "toolDiameter": 6.0,
        "depth": 5.0,
        "depthPerPass": 2.5,
        "stepover": 0.45,
        "cutFeed": 1200.0,
        "plungeFeed": 300.0,
        "safeZ": 5.0,
    }

    print(f"Creating CNC Pocket: {pocket_config['width']}x{pocket_config['height']} mm, Depth: {pocket_config['depth']} mm")
    print(f"  Tool Diameter: {pocket_config['toolDiameter']} mm, Depth/Pass: {pocket_config['depthPerPass']} mm")

    # 1. Author with fluent Design builder
    d = dry.Design()
    d.pocket(pocket_config)

    # 2. Resolve IR
    ir = d.ir()
    print(f"✓ Generated {len(ir['segments'])} CNC toolpath segments.")

    # 3. Simulate metrics
    m = d.simulate()
    print(f"✓ Estimated Machining Time: {m['total_time_s']:.1f}s ({m['total_time_s']/60:.1f} min)")

    # 4. Emit G-code
    gcode = d.gcode()
    print(f"✓ Emitted {len(gcode)} lines of G-code.")
    print("--- Sample G-code lines ---")
    for line in gcode[:8]:
        print("  ", line)

if __name__ == "__main__":
    main()

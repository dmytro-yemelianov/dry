#!/usr/bin/env python3
"""Example 11: Parametric 2-Axis CNC Lathe Facing & OD Turning.

Demonstrates:
1. Generating a multi-pass facing cycle to face bar stock flush at Z=0.0.
2. Generating a stepped outer diameter (OD) turning cycle with roughing passes and finish allowance.
3. Resolving the composite turning program into L2 IR segments and simulating cycle times.
4. Emitting machine-ready RS-274 / Fanuc-compatible 2-axis lathe G-code (XZ plane).
"""

import os
import sys
import dry

def main():
    print("=== Dry Example 11: Parametric 2-Axis CNC Lathe Facing & Turning ===")

    # 1. Configure Facing Operation: Face Ø50mm raw billet down to Z=0.0 in 2 passes
    facing_params = {
        "stock_diameter": 50.0,
        "target_z": 0.0,
        "clearance_x": 2.0,
        "clearance_z": 2.0,
        "feedrate": 280.0,
        "spindle_rpm": 1200.0,
        "passes": 2,
        "depth_per_pass": 1.0,
    }
    facing_ops = dry.lathe_facing_ops(facing_params)
    print(f"✓ Generated {len(facing_ops)} facing operations.")

    # 2. Configure OD Turning Operation: Turn Ø50mm down to Ø32mm over 45mm cut length
    turning_params = {
        "raw_diameter": 50.0,
        "target_diameter": 32.0,
        "cut_length": 45.0,
        "depth_of_cut": 2.5,
        "finish_allowance": 0.5,
        "clearance_x": 1.5,
        "clearance_z": 2.0,
        "rough_feedrate": 220.0,
        "finish_feedrate": 140.0,
        "spindle_rpm": 1500.0,
    }
    turning_ops = dry.lathe_turning_ops(turning_params)
    print(f"✓ Generated {len(turning_ops)} OD turning operations.")

    # 3. Assemble complete lathe machining program into Dry Design
    design = dry.Design()
    for op in facing_ops:
        design.ops.append(op)
    for op in turning_ops:
        design.ops.append(op)

    # 4. Resolve & Simulate
    toolpath = design.ir()
    metrics = design.simulate()
    print(f"✓ Total Toolpath Segments: {metrics['segment_count']}")
    print(f"✓ Total Machining Time: {metrics['total_time_s']:.1f}s ({metrics['total_time_s']/60:.1f} min)")
    print(f"✓ Total Tool Motion Distance: {metrics['travel_distance']:.1f} mm")

    # 5. Emit G-code for CNC Lathe (GRBL / RS274 dialect supporting spindle S commands)
    gcode_lines = design.gcode(flavor="grbl")
    print(f"✓ Emitted {len(gcode_lines)} lines of CNC Lathe G-code.")
    print("\n--- Sample Lathe G-code Lines ---")
    for line in gcode_lines[:10]:
        print(f"   {line}")
    print("   ...")
    for line in gcode_lines[-6:]:
        print(f"   {line}")

    print("\n✓ Lathe Turning Cycle Generated Successfully!")

if __name__ == "__main__":
    main()

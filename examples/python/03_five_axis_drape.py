#!/usr/bin/env python3
"""Example 03: 5-Axis Non-Planar Toolframe Drape.

Demonstrates:
- 5-axis toolframe orientation vectors (i, j, k).
- Multi-axis kinematics resolution (AB / BC head-table configuration).
- Emitting 5-axis rotary moves (A, B or B, C words) for multi-axis CNC & 3D printers.
"""
import math
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../../py/python"))
import dry

def generate_hemisphere_drape(radius: float = 25.0, steps: int = 40) -> dry.Design:
    design = (
        dry.Design()
        .geometry(width=0.5, height=0.2)
        .extruder(on=True)
        .speed(1200.0)
    )

    # Move across the dome from -radius to +radius
    for step in range(steps + 1):
        t = step / steps
        # Angle from -pi/3 to +pi/3 across the dome
        phi = (t - 0.5) * (2.0 * math.pi / 3.0)
        x = radius * math.sin(phi) + 100.0
        y = 100.0
        z = radius * math.cos(phi) - (radius * math.cos(math.pi / 3.0)) + 0.2

        # Surface normal vector (i, j, k) pointing outward from dome center
        i = math.sin(phi)
        j = 0.0
        k = math.cos(phi)

        # Set orientation for 5-axis toolhead, then command position
        design.orient(i, j, k).point(x, y, z)

    return design

def main():
    print("=== Dry Example 03: 5-Axis Toolframe Drape ===")
    drape = generate_hemisphere_drape()

    # 1. Simulate motion
    metrics = drape.simulate()
    print(f"✓ Total Segments: {metrics['segment_count']}")
    print(f"✓ Extruded Volume: {metrics['extruded_volume']:.3f} mm³")
    print(f"✓ Total Time: {metrics['total_time_s']:.2f}s")

    # 2. Emit 5-axis G-code with AB rotary kinematics
    gcode_5axis = drape.gcode(five_axis=True, rotary_axes="ab")
    print(f"✓ Emitted {len(gcode_5axis)} lines of 5-axis (AB) G-code.")
    print("--- Sample 5-axis G-code Lines ---")
    for line in gcode_5axis[:8]:
        print("  ", line)

if __name__ == "__main__":
    main()

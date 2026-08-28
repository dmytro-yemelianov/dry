#!/usr/bin/env python3
"""Example 01: Parametric Mathematical Spiral Vase with Continuous Z.

Demonstrates:
- Continuous Z vase mode generation.
- Real-time simulation and extrusion physics.
- Safety verification against machine build envelope.
"""
import math
import sys
import os

# Auto-inject repo py/python for direct standalone execution
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../../py/python"))
import dry

def generate_spiral_vase(
    radius_base: float = 25.0,
    height: float = 50.0,
    layers: int = 250,
    lobes: int = 5,
    modulation_amp: float = 3.0,
) -> dry.Design:
    design = (
        dry.Design()
        .geometry(width=0.6, height=0.2)
        .extruder(on=True)
        .speed(1800.0)
    )

    steps_per_rev = 60
    total_steps = layers * steps_per_rev
    dz = height / total_steps
    d_theta = 2.0 * math.pi / steps_per_rev

    for i in range(total_steps):
        theta = i * d_theta
        z = i * dz
        # Modulated radius creating parametric wave patterns
        r = radius_base + modulation_amp * math.sin(lobes * theta + z * 0.1)
        x = r * math.cos(theta) + 100.0
        y = r * math.sin(theta) + 100.0
        design.point(x=x, y=y, z=z)

    return design

def main():
    print("=== Dry Example 01: Spiral Vase ===")
    vase = generate_spiral_vase()

    # 1. Verify build volume & monotonic-Z
    report = vase.verify(
        bounds=[[0, 250], [0, 250], [0, 300]],
        monotonic_z=True,
    )
    findings = report.get("findings", [])
    errors = [f for f in findings if f.get("severity") == "error"]
    assert len(errors) == 0, f"Verification failed with errors: {errors}"
    print(f"✓ Verification clean: {len(findings)} findings, 0 errors.")

    # 2. Simulate metrics
    metrics = vase.simulate()
    print(f"✓ Total Segments: {metrics['segment_count']}")
    print(f"✓ Print Time: {metrics['total_time_s']:.1f}s ({metrics['total_time_s']/60:.1f} min)")
    print(f"✓ Filament Used: {metrics['filament_length']:.1f} mm")

    # 3. Emit G-code sample
    gcode = vase.gcode()
    print(f"✓ Emitted {len(gcode)} lines of G-code.")
    print("--- Sample Emitted G-code (first 5 lines) ---")
    for line in gcode[:5]:
        print("  ", line)

if __name__ == "__main__":
    main()

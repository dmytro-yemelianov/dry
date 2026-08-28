#!/usr/bin/env python3
"""Dry Example 10: Direct STEP ISO 10303-21 B-Rep Solid Slicing.

Demonstrates slicing exact analytical CAD surfaces (cylinders, spheres, cones) directly
from STEP entities without intermediate STL triangulation, computing exact normal vectors.
"""
import dry


def main():
    print("=== Dry Example 10: Direct STEP B-Rep Solid Slicing ===")

    # Simulated ISO 10303-21 STEP model containing cylinder and spherical dome
    step_model = """
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('STEP AP242 Conformal Solid'),'2;1');
ENDSEC;
DATA;
#10 = CARTESIAN_POINT('', (50.0, 50.0, 0.0));
#20 = DIRECTION('', (0.0, 0.0, 1.0));
#100 = CYLINDRICAL_SURFACE('', #10, 22.0);
#200 = SPHERICAL_SURFACE('', #10, 22.0);
ENDSEC;
END-ISO-10303-21;
"""

    print("Slicing exact B-Rep solid from Z=1.0 to Z=15.0 mm (layer height: 0.5 mm)...")
    ops = dry.slice_step_solid(
        step_model,
        z_start=1.0,
        z_end=15.0,
        layer_height=0.5,
        samples_per_slice=36,
        feedrate=2400.0,
    )
    print(f"✓ Generated {len(ops)} L1 toolpath operations directly from STEP B-Rep.")

    design = dry.Design.from_ops(ops)
    metrics = design.simulate()
    print(f"✓ Simulated: {metrics['segment_count']} segments, {metrics['total_time_s']:.2f}s total duration.")

    gcode_5axis = design.gcode(five_axis=True, rotary_axes="bc")
    print(f"✓ Emitted {len(gcode_5axis)} lines of 5-axis (BC) G-code with exact surface normals.")

    print("\n--- Sample 5-Axis (BC) G-code Lines ---")
    for line in gcode_5axis[:8]:
        print(f"   {line}")


if __name__ == "__main__":
    main()

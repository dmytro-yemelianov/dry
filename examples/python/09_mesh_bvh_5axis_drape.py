#!/usr/bin/env python3
"""Example 09: Mesh Heightfield 5-Axis Drape with BVH Acceleration (E1.3).

Demonstrates:
- Importing and parsing 3D surface meshes (OBJ / STL).
- Accelerating ray-mesh projection via Bounding Volume Hierarchy (BVH).
- Conformal non-planar toolpath generation with exact surface normal orientations (i, j, k).
- Emitting 5-axis G-code with rotary toolhead/table kinematics (A, B, C rotary words).
"""
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../../py/python"))
import dry

SAMPLE_OBJ_SADDLE = """
# Hyperbolic paraboloid / saddle mesh surface
v 0.0 0.0 10.0
v 20.0 0.0 0.0
v 40.0 0.0 10.0
v 0.0 20.0 0.0
v 20.0 20.0 5.0
v 40.0 20.0 0.0
v 0.0 40.0 10.0
v 20.0 40.0 0.0
v 40.0 40.0 10.0
f 1 2 5
f 1 5 4
f 2 3 6
f 2 6 5
f 4 5 8
f 4 8 7
f 5 6 9
f 5 9 8
"""

def main():
    print("=== Dry Example 09: Mesh Heightfield 5-Axis Drape with BVH Acceleration ===")

    # 1. Parse mesh and build BVH
    mesh = dry.parse_obj_mesh(SAMPLE_OBJ_SADDLE)
    print(f"✓ Parsed mesh with {len(mesh['triangles'])} triangles.")
    print(f"  Bounds: X[{mesh['bounds']['min'][0]:.1f}, {mesh['bounds']['max'][0]:.1f}], "
          f"Y[{mesh['bounds']['min'][1]:.1f}, {mesh['bounds']['max'][1]:.1f}], "
          f"Z[{mesh['bounds']['min'][2]:.1f}, {mesh['bounds']['max'][2]:.1f}]")

    # 2. Configure 5-axis conformal draping
    options = {
        "mesh": mesh,
        "stepover": 4.0,        # 4mm pitch between tool passes
        "resolution": 2.0,      # 2mm point sampling along path
        "standoffOffset": 0.2,  # 0.2mm normal standoff clearance
        "safeZ": 25.0,          # Safe transit plane
        "pattern": "zigzag-x",  # Continuous bidirectional pass
        "feedrate": 1500.0,
        "plungeFeed": 400.0,
        "width": 0.45,
        "height": 0.2,
    }

    ops = dry.drape_ops(options)
    print(f"✓ Generated {len(ops)} L1 draping ops with BVH ray projection.")

    # 3. Construct Design and verify kinematics
    d = dry.Design()
    d.ops = ops

    report = d.verify(
        bounds=[[0, 100], [0, 100], [0, 50]],
        max_flow=20.0,
    )
    errors = [f for f in report["findings"] if f["severity"] == "error"]
    print(f"✓ Pre-Flight Verification Evaluated: {len(report['findings'])} findings ({len(errors)} errors caught by safety rules).")

    # 4. Simulate metrics
    metrics = d.simulate()
    print(f"✓ Simulated: {metrics['segment_count']} segments, {metrics['total_time_s']:.2f}s total duration.")

    # 5. Emit 5-axis rotary G-code
    gcode = d.gcode(five_axis=True, rotary_axes="ab")
    print(f"✓ Emitted {len(gcode)} lines of 5-axis G-code.")
    print("--- Sample 5-axis G-code Output ---")
    for line in gcode[10:18]:
        print("  ", line)

if __name__ == "__main__":
    main()

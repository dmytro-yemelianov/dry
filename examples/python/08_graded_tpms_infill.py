#!/usr/bin/env python3
"""Example 08: Functionally Graded TPMS Metamaterials (Track E).

Demonstrates:
- Functionally Graded Additive Manufacturing (FGAM) with variable density cellular lattices.
- Slicing Triply Periodic Minimal Surfaces (Gyroid, Schwarz Diamond) with Z-density gradient.
- Simulating lightweighting mass savings and exporting 3D Mesh and WebGL viewers.
"""
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../../py/python"))
import dry

def main():
    print("=== Dry Example 08: Functionally Graded TPMS Metamaterials ===")

    # Define functionally graded cellular lattice zones
    # Lower zone: High density (isoLevel = 0.4, higher volume fraction for structural base)
    # Upper zone: Low density (isoLevel = 0.0, minimal surface for weight reduction)
    print("Generating Functionally Graded 'schwarz-d' (Diamond) Cellular Metamaterial:")
    print("  Base Zone (0-15mm): High Density (isoLevel = 0.35)")
    print("  Top Zone (15-30mm): Low Density (isoLevel = 0.05)")

    base_tpms_opts = {
        "surface": "schwarz-d",
        "cellSize": 15.0,
        "cellsX": 2,
        "cellsY": 2,
        "cellsZ": 1,
        "isoLevel": 0.35,
        "layerHeight": 0.25,
        "wallThickness": 0.45,
    }

    top_tpms_opts = {
        "surface": "schwarz-d",
        "cellSize": 15.0,
        "cellsX": 2,
        "cellsY": 2,
        "cellsZ": 1,
        "isoLevel": 0.05,
        "layerHeight": 0.25,
        "wallThickness": 0.45,
    }

    # Generate L1 Ops for both zones and combine into a single functionally graded design
    base_ops = dry.tpms_ops(base_tpms_opts)
    top_ops = dry.tpms_ops(top_tpms_opts)

    # Shift top zone in Z by 15mm
    shifted_top_ops = []
    for op in top_ops:
        op_copy = dict(op)
        if op_copy.get("op") == "move" and op_copy.get("z") is not None:
            op_copy["z"] += 15.0
        shifted_top_ops.append(op_copy)

    graded_design = dry.Design()
    for op in base_ops + shifted_top_ops:
        graded_design.ops.append(op)

    # Simulate material usage
    sim = graded_design.simulate()
    print(f"✓ Total Toolpath Segments: {sim['segment_count']}")
    print(f"✓ Total Extruded Volume: {sim['extruded_volume']:.1f} mm³")
    print(f"✓ Print Duration: {sim['total_time_s']:.1f}s ({sim['total_time_s'] / 60:.1f} min)")

    # Emit G-code
    gcode = graded_design.gcode(printer="generic")
    print(f"✓ Emitted {len(gcode)} lines of G-code.")

    # Export 3D Mesh and Interactive Viewer
    out_dir = os.path.join(os.path.dirname(__file__), "../output")
    os.makedirs(out_dir, exist_ok=True)
    obj_path = os.path.join(out_dir, "graded_tpms_lattice.obj")
    html_path = os.path.join(out_dir, "graded_tpms_viewer.html")
    graded_design.export_obj(obj_path)
    graded_design.export_html(html_path, title="Functionally Graded Diamond TPMS Lattice")
    print(f"✓ Exported 3D Model to: {obj_path}")
    print(f"✓ Exported 3D HTML Viewer to: {html_path}")

if __name__ == "__main__":
    main()

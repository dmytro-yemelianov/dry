#!/usr/bin/env python3
"""Example 07: High-Speed Trochoidal Pocket Milling & Helical Ramp Entry (Track E).

Demonstrates:
- 2.5D Subtractive CNC pocket milling with automated helical ramp-in plunge protection.
- Constant radial tool engagement angle calculation to prevent tool breakage.
- Adaptive corner feedrate reduction around sharp 90-degree internal turns.
- Verification and G-code export for Haas / LinuxCNC milling centers.
"""
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../../py/python"))
import dry

def main():
    print("=== Dry Example 07: High-Speed CNC Pocket with Helical Entry ===")

    # Define pocketing options with helical entry
    pocket_config = {
        "shape": "rect",
        "x": 50.0,
        "y": 50.0,
        "width": 75.0,
        "height": 50.0,
        "toolDiameter": 6.0,
        "stepover": 0.45,            # 45% radial stepover
        "depth": 6.0,                # 6mm total depth
        "depthPerPass": 2.0,         # 3 depth passes (2mm, 4mm, 6mm)
        "zTop": 20.0,
        "safeZ": 25.0,
        "cutFeed": 1800.0,           # 1800 mm/min high-speed machining
        "plungeFeed": 600.0,         # 600 mm/min helical ramp feed
        "helicalEntry": True,        # Automated helical descent
    }

    print(f"Machining Pocket: {pocket_config['width']}x{pocket_config['height']} mm, Depth: {pocket_config['depth']} mm")
    print(f"  Tool: Ø{pocket_config['toolDiameter']} mm, Stepover: {pocket_config['stepover'] * 100:.0f}%")
    print(f"  Plunge Protection: Helical Ramp Entry (Helical spiral descent)")

    # 1. Build CNC Toolpath via Fluent API
    design = dry.Design().pocket(pocket_config)

    # 2. Simulate Machining Time and Material Removal
    sim = design.simulate()
    print(f"✓ Total Toolpath Segments: {sim['segment_count']}")
    print(f"✓ Estimated Machining Time: {sim['total_time_s']:.1f}s ({sim['total_time_s'] / 60:.1f} min)")

    # 3. Pre-flight Verify against Machine Limits (Haas VF-2)
    catalog = dry.MachineCatalog()
    haas_profile = catalog.get("haas-vf2").to_capabilities()
    
    report = design.check_compatibility(haas_profile)
    print(f"✓ Machine Pre-Flight (Haas VF-2): {'COMPATIBLE' if report['compatible'] else 'INCOMPATIBLE'}")

    # 4. Emit Production G-code
    gcode_lines = design.gcode(printer="generic")
    print(f"✓ Emitted {len(gcode_lines)} lines of CNC G-code.")
    print("--- Sample G-code with Helical Ramp-In ---")
    for line in gcode_lines[:15]:
        print(f"   {line}")

    # 5. Export 3D Mesh and Visualizer
    out_dir = os.path.join(os.path.dirname(__file__), "../output")
    os.makedirs(out_dir, exist_ok=True)
    obj_path = os.path.join(out_dir, "trochoidal_pocket.obj")
    html_path = os.path.join(out_dir, "trochoidal_pocket_viewer.html")
    design.export_obj(obj_path)
    design.export_html(html_path, title="Trochoidal Pocket Milling")
    print(f"✓ Exported 3D Model to: {obj_path}")
    print(f"✓ Exported 3D HTML Viewer to: {html_path}")

if __name__ == "__main__":
    main()

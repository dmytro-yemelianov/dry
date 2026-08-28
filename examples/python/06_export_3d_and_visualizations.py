#!/usr/bin/env python3
"""Example 06: 3D Model & Interactive Visualization Exporters.

Demonstrates:
- Exporting parametric toolpaths to 3D Wavefront OBJ mesh files.
- Exporting 2D/isometric vector SVG visualization blueprints.
- Generating a self-contained interactive 3D WebGL HTML viewer with Three.js.
"""
import sys
import os
import math

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../../py/python"))
import dry

def build_twisted_vase() -> dry.Design:
    design = (
        dry.Design()
        .geometry(width=0.6, height=0.25)
        .extruder(True)
        .speed(dry.mm_s(50))
    )
    layers = 80
    sides = 6
    radius = 30.0

    for l in range(layers):
        z = l * 0.25
        twist = l * (math.pi / 40.0)
        scale = 1.0 + 0.3 * math.sin(l * 0.1)

        for s in range(sides + 1):
            theta = twist + s * (2.0 * math.pi / sides)
            x = 100.0 + radius * scale * math.cos(theta)
            y = 100.0 + radius * scale * math.sin(theta)
            design.point(x, y, z)

    return design

def main():
    print("=== Dry Example 06: 3D Models & Interactive Visualizations ===")
    out_dir = os.path.join(os.path.dirname(__file__), "../output")
    os.makedirs(out_dir, exist_ok=True)

    vase = build_twisted_vase()
    print("✓ Built parametric twisted polygon vase design.")

    # 1. Export 3D Wavefront OBJ Model
    obj_path = os.path.join(out_dir, "twisted_vase.obj")
    vase.export_obj(obj_path)
    print(f"✓ Exported 3D Model to: {obj_path} ({os.path.getsize(obj_path)} bytes)")

    # 2. Export 2D Vector SVG Projection
    svg_path = os.path.join(out_dir, "twisted_vase.svg")
    vase.export_svg(svg_path, width=800, height=800)
    print(f"✓ Exported Vector SVG to: {svg_path} ({os.path.getsize(svg_path)} bytes)")

    # 3. Export Standalone 3D Interactive WebGL HTML Viewer
    html_path = os.path.join(out_dir, "twisted_vase_3d_viewer.html")
    vase.export_html(html_path, title="Twisted Vase 3D Toolpath", bounds=[[0, 200], [0, 200], [0, 200]])
    print(f"✓ Exported Interactive 3D HTML Viewer to: {html_path} ({os.path.getsize(html_path)} bytes)")

if __name__ == "__main__":
    main()

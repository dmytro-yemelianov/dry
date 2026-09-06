"""Test 3D visualization and model export helpers."""

import re

import dry

def test_visualizer_exports():
    d = (
        dry.Design()
        .geometry(0.6, 0.2)
        .extruder(True)
        .point(0, 0, 0.2)
        .point(20, 0, 0.2)
        .point(20, 20, 0.2)
        .point(0, 20, 0.2)
    )

    # 1. OBJ Mesh Export
    obj_text = d.to_obj()
    assert "o DryToolpath" in obj_text
    assert "v 0.0000 0.0000 0.2000" in obj_text
    assert "l 1 2" in obj_text

    # 2. SVG 2D Projection Export
    svg_text = d.to_svg()
    assert "<svg" in svg_text
    assert "<line" in svg_text
    assert "</svg>" in svg_text

    # 3. Standalone 3D Interactive WebGL HTML Viewer
    html_text = d.to_html(title="Test 3D Model")
    assert "<!DOCTYPE html>" in html_text
    assert "three.js" in html_text
    assert "Test 3D Model" in html_text


def test_visualizer_pins_dependencies_and_escapes_title():
    html_text = dry.Design().point(0, 0, 0.2).to_html(title="<script>alert(1)</script>")

    external_scripts = re.findall(r'<script\b[^>]*\bsrc="https://[^"]+"[^>]*></script>', html_text)
    assert len(external_scripts) == 2
    for script in external_scripts:
        assert re.search(r'\bintegrity="sha384-[A-Za-z0-9+/=]+"', script)
        assert 'crossorigin="anonymous"' in script
    assert "<script>alert(1)</script>" not in html_text
    assert "&lt;script&gt;alert(1)&lt;/script&gt;" in html_text

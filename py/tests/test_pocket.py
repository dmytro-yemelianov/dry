import pytest
import dry


def test_pocket_ops_generates_rectangular_pocket():
    options = {
        "shape": "rect",
        "x": 0.0,
        "y": 0.0,
        "width": 50.0,
        "height": 40.0,
        "toolDiameter": 6.0,
        "depth": 5.0,
        "depthPerPass": 2.5,
    }
    ops = dry.pocket_ops(options)
    assert isinstance(ops, list) and len(ops) > 5

    gcode = dry.pocket_gcode(options)
    assert isinstance(gcode, list) and len(gcode) > 5
    assert any("Z-2.5" in line or "Z-5" in line for line in gcode)


def test_pocket_ops_generates_circular_pocket():
    options = {
        "shape": "circle",
        "cx": 25.0,
        "cy": 25.0,
        "radius": 20.0,
        "toolDiameter": 4.0,
        "depth": 3.0,
        "depthPerPass": 1.5,
    }
    ops = dry.pocket_ops(options)
    assert isinstance(ops, list) and len(ops) > 5

    d = dry.Design.from_ops(ops)
    ir = d.ir()
    assert len(ir["segments"]) > 5


def test_pocket_tool_larger_than_geometry_raises():
    options = {
        "shape": "rect",
        "x": 0.0,
        "y": 0.0,
        "width": 4.0,
        "height": 4.0,
        "toolDiameter": 10.0,
        "depth": 2.0,
    }
    with pytest.raises(ValueError):
        dry.pocket_ops(options)


def test_design_pocket_fluent_builder():
    d = dry.Design()
    d.pocket({
        "shape": "rect",
        "x": 0.0,
        "y": 0.0,
        "width": 30.0,
        "height": 20.0,
        "toolDiameter": 4.0,
        "depth": 2.0,
        "depthPerPass": 1.0,
    })
    assert len(d.ops) > 0
    ir = d.ir()
    assert len(ir["segments"]) > 0


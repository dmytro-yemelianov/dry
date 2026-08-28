"""Test mesh BVH 5-axis draping Python API (E1.3)."""

import dry


def test_obj_parser_and_mesh_drape():
    obj_data = """
    v 0.0 0.0 0.0
    v 20.0 0.0 5.0
    v 20.0 20.0 5.0
    v 0.0 20.0 0.0
    f 1 2 3
    f 1 3 4
    """
    mesh = dry.parse_obj_mesh(obj_data)
    assert "triangles" in mesh
    assert len(mesh["triangles"]) == 2

    options = {
        "mesh": mesh,
        "stepover": 2.0,
        "resolution": 1.0,
        "standoffOffset": 0.5,
        "pattern": "zigzag-x",
    }

    ops = dry.drape_ops(options)
    assert len(ops) > 0

    d = dry.Design()
    d.ops = [dry.Op.from_dict(op) if hasattr(dry.Op, 'from_dict') else op for op in ops]

    # Verify gcode emission
    gcode = d.gcode(five_axis=True, rotary_axes="ab")
    assert len(gcode) > 0
    # Must contain rotary words A and/or B
    has_rotary = any('A' in line or 'B' in line for line in gcode)
    assert has_rotary, "Emitted 5-axis g-code must contain rotary words"

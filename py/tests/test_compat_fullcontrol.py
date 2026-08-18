import dry.compat.fullcontrol as fc


def test_fc_square_emits_gcode():
    steps = [
        fc.ExtrusionGeometry(width=0.6, height=0.2),
        fc.Extruder(on=True),
        fc.Point(x=0, y=0, z=0.2),
        fc.Point(x=10, y=0, z=0.2),
        fc.Point(x=10, y=10, z=0.2),
        fc.Point(x=0, y=10, z=0.2),
        fc.Point(x=0, y=0, z=0.2),
    ]
    lines = fc.gcode(steps)
    assert len(lines) == 5
    assert any("X10" in line for line in lines)


def test_fc_arc_and_channels_emit_gcode():
    steps = [
        fc.ExtrusionGeometry(width=0.6, height=0.2),
        fc.Extruder(on=True),
        fc.Point(x=10, y=0, z=0.2),
        fc.Arc(centre=fc.Point(x=0, y=0), end=fc.Point(x=0, y=10), direction="anticlockwise"),
        fc.Point(x=0, y=20, z=0.2),
    ]
    lines = fc.gcode(steps)
    assert len(lines) == 3
    assert any("G3" in line for line in lines)


def test_fc_transform_translates_and_rotates():
    steps = [
        fc.Point(x=10, y=0, z=0.5),
        fc.Point(x=20, y=0, z=0.5),
    ]
    shifted = fc.transform(steps, translation=fc.Point(x=5, y=10, z=0))
    assert shifted[0].x == 15.0
    assert shifted[0].y == 10.0
    assert shifted[0].z == 0.5

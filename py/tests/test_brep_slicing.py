"""Test B-Rep direct STEP solid slicing."""
import dry


def test_step_solid_slicing():
    step_mock = """
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('STEP AP242'),'2;1');
ENDSEC;
DATA;
#10 = CARTESIAN_POINT('', (50.0, 50.0, 0.0));
#20 = DIRECTION('', (0.0, 0.0, 1.0));
#100 = CYLINDRICAL_SURFACE('', #10, 20.0);
ENDSEC;
END-ISO-10303-21;
"""
    ops = dry.slice_step_solid(step_mock, z_start=1.0, z_end=5.0, layer_height=2.0)
    assert len(ops) > 0

    design = dry.Design.from_ops(ops)
    gcode = design.gcode()
    assert len(gcode) > 0


def test_brep_assembly_slicing():
    step_mock_1 = """
ISO-10303-21;
HEADER;
ENDSEC;
DATA;
#100 = CYLINDRICAL_SURFACE('', #10, 20.0);
ENDSEC;
END-ISO-10303-21;
"""
    step_mock_2 = """
ISO-10303-21;
HEADER;
ENDSEC;
DATA;
#200 = SPHERICAL_SURFACE('', #10, 25.0);
ENDSEC;
END-ISO-10303-21;
"""
    ops = dry.slice_brep_assembly([step_mock_1, step_mock_2], z_start=2.0, z_end=6.0, layer_height=2.0)
    assert len(ops) > 0

    design = dry.Design.from_ops(ops)
    gcode = design.gcode()
    assert len(gcode) > 0


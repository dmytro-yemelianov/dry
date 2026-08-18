import math
import dry


def test_dimensional_units_conversions():
    assert dry.mm(15) == 15.0
    assert dry.cm(2.5) == 25.0
    assert dry.inch(1.0) == 25.4
    assert dry.inch(2.0) == 50.8

    assert abs(dry.deg(180) - math.pi) < 1e-9
    assert abs(dry.deg(90) - math.pi / 2) < 1e-9
    assert dry.rad(1.5) == 1.5

    assert dry.mm_s(10) == 600.0  # 10 mm/s = 600 mm/min
    assert dry.mm_min(1200) == 1200.0

    assert dry.celsius(215) == 215.0
    assert dry.s(2.5) == 2.5
    assert dry.ms(500) == 0.5


def test_design_authors_with_dimensional_units():
    d = (
        dry.Design()
        .geometry(dry.mm(0.6), dry.mm(0.2))
        .extruder(True)
        .speed(dry.mm_s(20))  # 1200 mm/min
        .temperature(dry.celsius(210))
        .dwell(dry.ms(500))
        .point(dry.inch(0), dry.inch(0), dry.mm(0.2))
        .point(dry.inch(1), dry.inch(0), dry.mm(0.2))
    )
    gcode = d.gcode()
    assert any("X25.4" in line for line in gcode)
    assert any("F1200" in line for line in gcode)

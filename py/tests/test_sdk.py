"""The Python SDK authors a design and the engine reproduces the FullControl oracle (clean-room proof):
a design authored in Python emits the same g-code as the corresponding `conformance/gcode/*.json`."""
import json
import os

import dry

HERE = os.path.dirname(__file__)


def oracle_gcode(name):
    with open(os.path.join(HERE, "..", "..", "conformance", "gcode", f"{name}.json")) as f:
        return json.load(f)["expected"]


def oracle_metrics(name):
    with open(os.path.join(HERE, "..", "..", "conformance", "simulate", f"{name}.json")) as f:
        return json.load(f)["expected"]


def test_square_authored_in_python_matches_the_oracle():
    d = (dry.Design().geometry(0.6, 0.2).extruder(True)
         .point(0, 0, 0.2).point(10, 0, 0.2).point(10, 10, 0.2).point(0, 10, 0.2).point(0, 0, 0.2))
    assert d.gcode() == oracle_gcode("square")


def test_arc_authored_in_python_matches_the_oracle():
    d = (dry.Design().geometry(0.6, 0.2).extruder(True)
         .point(10, 0, 0.2)
         .arc(cx=0, cy=0, x=0, y=10, clockwise=False)
         .point(0, 20, 0.2))
    assert d.gcode() == oracle_gcode("arc_ccw")


def test_simulate_matches_the_oracle_metrics():
    d = (dry.Design().geometry(0.6, 0.2).extruder(True)
         .point(0, 0, 0.2).point(10, 0, 0.2).point(10, 10, 0.2).point(0, 10, 0.2).point(0, 0, 0.2))
    m = d.simulate()
    want = oracle_metrics("square")
    assert m["segment_count"] == want["segment_count"]
    assert abs(m["total_time_s"] - want["total_time_s"]) < 1e-9
    assert abs(m["extruded_volume"] - want["extruded_volume"]) < 1e-9


def test_ir_round_trips():
    d = dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(10, 0, 0.2)
    ir = d.ir()
    assert ir["version"] == 0 and len(ir["segments"]) == 2
    assert ir["segments"][1]["end"] == [10.0, 0.0, 0.2]

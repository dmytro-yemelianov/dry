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


def test_channels_and_dwell_author_through_the_builder():
    d = (dry.Design().geometry(0.6, 0.2).temperature(210).fan(0.5).tool(1).extruder(True)
         .point(0, 0, 0.2).point(10, 0, 0.2).dwell(2.5))
    ir = d.ir()
    assert ir["segments"][1]["temperature"] == 210
    assert ir["segments"][1]["fan"] == 0.5
    assert ir["segments"][1]["tool"] == 1
    assert any(line == "G4 S2.5" for line in d.gcode())


def test_toolframe_orientation_authors_onto_segments():
    d = (dry.Design().geometry(0.6, 0.2).orient(0.6, 0.0, 0.8).extruder(True)
         .point(0, 0, 0.2).point(10, 0, 0.2))
    assert d.ir()["segments"][1]["orientation"] == [0.6, 0.0, 0.8]


def test_spline_authors_through_the_builder():
    # 1 positioning move + a spline through 3 control points (3 spans × 16 samples = 48).
    d = (dry.Design().geometry(0.6, 0.2).extruder(True)
         .point(0, 0, 0.2)
         .spline([(10, 0, 0.2), (10, 10, 0.2), (0, 10, 0.2)]))
    ir = d.ir()
    assert len(ir["segments"]) == 1 + 48
    # the spline ends at the last control point.
    assert ir["segments"][-1]["end"] == [0.0, 10.0, 0.2]
    assert all(s["kind"] == "line" for s in ir["segments"][1:])


def test_flow_multiplier_scales_volume():
    base = dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(10, 0, 0.2)
    scaled = (dry.Design().geometry(0.6, 0.2).flow(0.8).extruder(True)
              .point(0, 0, 0.2).point(10, 0, 0.2))
    b = base.ir()["segments"][1]["volume"]
    s = scaled.ir()["segments"][1]["volume"]
    assert abs(s - b * 0.8) < 1e-12


def test_verify_contracts():
    # Valid design
    d = dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(10, 0, 0.2)
    report = d.verify(max_flow=15.0, bounds="0,100,0,100,0,50")
    assert report["findings"] == []

    # Out of bounds design
    d_bad_bounds = dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(150, 0, 0.2)
    report_bounds = d_bad_bounds.verify(bounds="0,100,0,100,0,50")
    assert len(report_bounds["findings"]) > 0
    assert report_bounds["findings"][0]["rule"] == "bounds"

    # Monotonic Z violation
    d_bad_z = dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.5).point(10, 0, 0.2)
    report_z = d_bad_z.verify(monotonic_z=True)
    assert len(report_z["findings"]) > 0
    assert report_z["findings"][0]["rule"] == "monotonic-z"


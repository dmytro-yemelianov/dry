"""The Python SDK authors a design and the engine reproduces the FullControl oracle (clean-room proof):
a design authored in Python emits the same g-code as the corresponding `conformance/gcode/*.json`."""
import json
import os

import pytest

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


def _oriented_5axis_design():
    # A tilted toolframe so 5-axis emit produces real rotary words that depend on the rotary-axes pick.
    return (dry.Design().geometry(0.6, 0.2).orient(0.6, 0.0, 0.8).extruder(True)
            .point(0, 0, 0.2).point(10, 0, 0.2))


def test_rotary_axes_selector_and_kinematics_alias_for_five_axis_emit():
    """`gcode` renamed `kinematics` → `rotary_axes` (the ab/ac/bc string). The new keyword selects the
    rotary axes; the old `kinematics` keyword stays a back-compat alias; both are non-breaking."""
    d = _oriented_5axis_design()
    ab = d.gcode(five_axis=True, rotary_axes="ab")
    ac_new = d.gcode(five_axis=True, rotary_axes="ac")
    # The selector actually changes the emitted rotary words.
    assert ac_new != ab, "rotary_axes='ac' must differ from the default 'ab' rotary words"
    # The legacy `kinematics=` alias still works and matches the new keyword.
    ac_alias = d.gcode(five_axis=True, kinematics="ac")
    assert ac_alias == ac_new, "legacy kinematics= alias must equal rotary_axes="
    # Positional compatibility: the 5th positional arg is still the rotary-axes string.
    assert d.gcode("generic", True, False, True, "ac") == ac_new


def test_spline_authors_through_the_builder():
    d = (dry.Design().geometry(0.6, 0.2).extruder(True)
         .point(0, 0, 0.2)
         .spline([(10, 0, 0.2), (10, 10, 0.2), (0, 10, 0.2)]))
    ir = d.ir()
    # Expect 2 segments: 1 positioning line + 1 first-class spline segment.
    assert len(ir["segments"]) == 2
    assert ir["segments"][0]["kind"] == "line"
    assert ir["segments"][1]["kind"] == "spline"
    assert ir["segments"][1]["end"] == [0.0, 10.0, 0.2]
    assert ir["segments"][1]["control_points"] == [[10.0, 0.0, 0.2], [10.0, 10.0, 0.2], [0.0, 10.0, 0.2]]
    
    # Verify that emitting g-code resolves the spline into 48 sub-moves.
    gcode = d.gcode()
    assert len(gcode) == 49


def test_flow_multiplier_scales_volume():
    base = dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(10, 0, 0.2)
    scaled = (dry.Design().geometry(0.6, 0.2).flow(0.8).extruder(True)
              .point(0, 0, 0.2).point(10, 0, 0.2))
    b = base.ir()["segments"][1]["volume"]
    s = scaled.ir()["segments"][1]["volume"]
    assert abs(s - b * 0.8) < 1e-12


def test_default_retraction_builders_emit_real_e_moves():
    d = dry.Design().geometry(0.6, 0.2).point(0, 0, 0.2).retract().unretract()
    gcode = d.gcode()
    assert gcode[1] == "G1 F1000 E-1"
    assert gcode[2] == "G1 E1"


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


def test_verify_accepts_structured_limits():
    # The SDK accepts structured limits, not just comma-strings; both must agree.
    d = dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(150, 0, 0.2)
    structured = d.verify(bounds=[[0, 100], [0, 100], [0, 50]], speed_range=[300, 9000])
    csv = d.verify(bounds="0,100,0,100,0,50", speed_range="300,9000")
    assert structured == csv
    assert any(f["rule"] == "bounds" for f in structured["findings"])
    # None and pass-through string still behave.
    assert d.verify(bounds=None)["findings"] == []


@pytest.mark.parametrize("limits", [
    {"bounds": [[100, 0], [0, 100], [0, 50]]},
    {"bounds": "100,0,0,100,0,50"},
    {"speed_range": [9000, 300]},
    {"speed_range": "9000,300"},
    {"first_layer_height_range": [0.5, 0.1]},
    {"first_layer_speed_range": [3000, 1000]},
])
def test_verify_rejects_inverted_contract_ranges(limits):
    d = dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(10, 0, 0.2)
    with pytest.raises(ValueError, match="lower bound must be <= upper bound"):
        d.verify(**limits)


def test_verify_validates_contract_ranges_before_resolving_toolpath():
    d = dry.Design().geometry(-0.6, 0.2).extruder(True).point(0, 0, 0.2).point(10, 0, 0.2)
    with pytest.raises(ValueError, match="bounds x lower bound must be <= upper bound"):
        d.verify(bounds=[[100, 0], [0, 100], [0, 50]])


def test_verify_allows_equal_contract_endpoints():
    d = dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(10, 0, 0.2)
    report = d.verify(
        bounds=[[0, 0], [0, 0], [0.2, 0.2]],
        speed_range=[600, 600],
        first_layer_height_range=[0.2, 0.2],
        first_layer_speed_range=[600, 600],
    )
    assert isinstance(report["findings"], list)


def test_verify_csv_string_backward_compat():
    # Legacy CSV strings for bounds/speed_range must keep working now that the binding takes
    # native typed contracts: the Python layer parses the string into the structured form.
    d = dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(150, 0, 0.2)
    report = d.verify(bounds="0,100,0,100,0,50", speed_range="300,9000")
    assert any(f["rule"] == "bounds" for f in report["findings"])
    # The CSV path is identical to passing the equivalent structured limits.
    structured = d.verify(bounds=[[0, 100], [0, 100], [0, 50]], speed_range=[300, 9000])
    assert report == structured


def test_verify_exposes_retraction_and_first_layer_limits():
    # The newly-exposed typed Contracts fields (retraction + first-layer limits) reach the engine
    # through structured kwargs and produce their findings.
    d = (dry.Design().geometry(0.6, 0.2).extruder(True)
         .point(0, 0, 0.2).point(10, 0, 0.2)
         .extruder(False)
         .point(200, 200, 0.2)                 # a long travel with no retraction
         .retract(distance=5.0, speed=3000.0))  # an over-long, over-fast retraction
    report = d.verify(
        max_retraction_distance=2.0,
        max_retraction_speed=1500.0,
        max_travel_without_retract=50.0,
        first_layer_height_range=[0.3, 0.5],
        first_layer_speed_range=[2000.0, 3000.0],
    )
    rules = {f["rule"] for f in report["findings"]}
    assert "retraction-distance" in rules
    assert "retraction-speed" in rules
    assert "travel-without-retraction" in rules
    assert "first-layer-height" in rules
    assert "first-layer-speed" in rules


def test_optimized_ir_runs_without_error():
    d = dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(10, 0, 0.2).point(20, 0, 0.2)
    assert len(d.ir()["segments"]) == 3
    opt = d.optimized_ir()
    assert opt["version"] == 0
    # The two collinear segments [0,0]->[10,0]->[20,0] should be merged into one.
    # Total segments: 1 positioning (travel) + 1 merged extrusion = 2.
    assert len(opt["segments"]) == 2
    assert opt["segments"][1]["end"] == [20.0, 0.0, 0.2]


def test_binary_roundtrips():
    d = dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(10, 0, 0.2)
    bin_data = d.binary()
    assert isinstance(bin_data, bytes)
    assert bin_data.startswith(b"DRY0") or bin_data.startswith(b"DRY1")



def _moves(lines):
    return [ln for ln in lines if ln.startswith("G0 ") or ln.startswith("G1 ")]


def test_tpms_gyroid_emits_gcode():
    # Default surface is gyroid; a single small cell still yields real contour moves.
    lines = dry.tpms_gcode({"cellsX": 1, "cellsY": 1, "cellsZ": 1})
    assert isinstance(lines, list) and lines
    assert _moves(lines), "expected G0/G1 moves"


def test_tpms_other_surfaces_emit_gcode():
    # A 2x2x2 block guarantees iso-crossings for every surface.
    base = {"cellsX": 2, "cellsY": 2, "cellsZ": 2, "cellSize": 8, "samplesPerCell": 12,
            "layerHeight": 0.4}
    for surface in ("schwarz-p", "fischer-koch-s", "split-p"):
        lines = dry.tpms_gcode({**base, "surface": surface})
        assert _moves(lines), f"{surface}: expected G0/G1 moves"


def test_tpms_all_surfaces_are_accepted():
    base = {"cellsX": 2, "cellsY": 2, "cellsZ": 2, "cellSize": 8, "samplesPerCell": 10,
            "layerHeight": 0.5}
    for surface in dry.TPMS_SURFACES:
        assert _moves(dry.tpms_gcode({**base, "surface": surface})), surface


def test_tpms_unknown_surface_raises():
    with pytest.raises(ValueError):
        dry.tpms_gcode({"surface": "bogus", "cellsX": 1, "cellsY": 1, "cellsZ": 1})


def test_tpms_budget_guard_raises():
    # 11x11 samples over 11 layers = 1331 field samples; a 1330 cap must trip the guardrail.
    with pytest.raises(ValueError):
        dry.tpms_gcode({"cellSize": 10, "cellsX": 1, "cellsY": 1, "cellsZ": 1,
                        "samplesPerCell": 10, "layerHeight": 1, "maxFieldSamples": 1330})

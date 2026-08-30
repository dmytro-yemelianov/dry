"""Cross-target parity for the Phase 7/8 kernel additions.

`optimize_five_axis_lookahead` and `analyze_machining_physics` reached `dry-core` without reaching
any binding (`docs/14-known-limitations.md`). These pin the Python half of closing that.
"""

import json

import pytest

import dry


def _tool():
    return {
        "diameter_mm": 10.0,
        "flute_count": 4,
        "stickout_length_mm": 40.0,
        "core_diameter_ratio": 0.6,
        "modulus_gpa": 600.0,
        "corner_radius_mm": 0.5,
    }


def _params():
    return {
        "spindle_rpm": 8000.0,
        "feedrate_mm_min": 1200.0,
        "axial_depth_ap_mm": 5.0,
        "radial_depth_ae_mm": 2.0,
        "ambient_temp_c": 20.0,
    }


def test_physics_report_has_every_documented_metric():
    report = dry.analyze_machining_physics(_tool(), "Aluminum6061", _params())
    for key in (
        "cutting_speed_m_min",
        "feed_per_tooth_mm",
        "material_removal_rate_cm3_min",
        "tangential_force_n",
        "spindle_power_kw",
        "spindle_torque_nm",
        "tool_deflection_um",
        "shear_temperature_c",
        "estimated_tool_life_min",
        "surface_roughness_ra_um",
        "chatter_risk",
        "model_saturated",
    ):
        assert key in report, key
    assert report["cutting_speed_m_min"] > 0.0
    assert isinstance(report["chatter_risk"], bool)


def test_a_clamped_result_reports_itself_as_saturated():
    """Both clamps are guardrails; a clamped value must not read as a prediction."""
    absurd = dict(_params(), spindle_rpm=8000.0)
    r = dry.analyze_machining_physics(_tool(), "TitaniumTi6Al4V", absurd)
    assert r["model_saturated"] is True
    assert r["estimated_tool_life_min"] == 0.1
    assert r["shear_temperature_c"] == 1220.0

    sane = dict(_params(), spindle_rpm=6000.0, feedrate_mm_min=900.0,
                axial_depth_ap_mm=2.0, radial_depth_ae_mm=3.0)
    assert dry.analyze_machining_physics(_tool(), "Aluminum6061", sane)["model_saturated"] is False


def test_physics_distinguishes_materials():
    """Inconel is far harder to cut than aluminium; the report must say so."""
    alu = dry.analyze_machining_physics(_tool(), "Aluminum6061", _params())
    inc = dry.analyze_machining_physics(_tool(), "Inconel718", _params())
    assert inc["tangential_force_n"] > alu["tangential_force_n"]
    assert inc["estimated_tool_life_min"] < alu["estimated_tool_life_min"]


def test_unknown_material_is_refused_not_defaulted():
    with pytest.raises(ValueError):
        dry.analyze_machining_physics(_tool(), "Unobtainium", _params())


def test_lookahead_preserves_segment_count_and_bounds_speed():
    d = dry.Design().geometry(0.4, 0.2).point(0, 0, 0.2)
    for i in range(1, 6):
        d = d.point(i * 10.0, 0.0, 0.2)
    tp = json.loads(json.dumps(d.ir()))  # plain dict, as the SDK returns

    params = {
        "max_linear_accel": 500.0,
        "max_linear_jerk": 5000.0,
        "max_rotary_speed_deg_s": 60.0,
        "max_rotary_accel_deg_s2": 300.0,
        "max_rotary_jerk_deg_s3": 3000.0,
    }
    out = dry.optimize_five_axis_lookahead(tp, params)
    assert len(out["segments"]) == len(tp["segments"])
    for a, b in zip(out["segments"], tp["segments"]):
        assert a["speed"] <= b["speed"] + 1e-9, "lookahead must never speed a segment up"


def _square():
    return (
        dry.Design()
        .geometry(0.4, 0.2)
        .point(0, 0, 0.2)
        .point(10, 0, 0.2)
        .point(10, 10, 0.2)
    )


FRAME = {"wcs": 54, "tool": 3, "spindle_rpm": 8000.0, "coolant": True}


def test_industrial_flavors_are_reachable_from_python():
    """The Phase 8 dialects existed in the kernel and the CLI but not here."""
    for flavor, marker, five_axis in [
        ("siemens", "TRAORI", True),
        ("heidenhain", "BEGIN PGM", False),
        ("haas", "G187", False),
        ("rapid", "MODULE DryProgram", False),
    ]:
        lines = _square().gcode(flavor=flavor, five_axis=five_axis, cnc_frame=FRAME)
        assert any(marker in line for line in lines), f"{flavor}: no {marker} in {lines[:8]}"


def test_the_machine_preamble_needs_a_cnc_frame():
    """Without a frame there is no work offset, tool change or spindle start — only motion."""
    bare = _square().gcode(flavor="siemens", five_axis=True)
    assert not any("TRAORI" in line for line in bare)
    assert not any(line.startswith("T3 ") for line in bare)

    framed = _square().gcode(flavor="siemens", five_axis=True, cnc_frame=FRAME)
    assert any("TRAORI" in line for line in framed)
    assert any("S8000 M3" in line for line in framed)


def test_an_invalid_cnc_frame_is_refused():
    with pytest.raises(ValueError):
        _square().gcode(flavor="siemens", cnc_frame={"wcs": 99})
    with pytest.raises(ValueError):
        _square().gcode(flavor="siemens", cnc_frame={"spindle_rpm": 0.0})


def test_unknown_flavor_is_an_error_not_silent_marlin():
    """It used to fall through to Marlin, so asking for a mill emitted FFF g-code."""
    with pytest.raises(ValueError) as exc:
        _square().gcode(flavor="sinumerik840d")
    assert "unknown firmware flavor" in str(exc.value)

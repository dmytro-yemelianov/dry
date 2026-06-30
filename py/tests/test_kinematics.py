"""Tests for the kinematics-aware PyO3 binding: `resolve_balanced_ir` and the
`kinematics` kwarg on `resolve_verify` / `Design.verify`.

NOTE: These tests require the compiled `_native` extension, which is built by
`maturin develop` (or `maturin build` + install) as part of the `py` CI job.
They are NOT runnable locally without maturin / a Python-linked `dry-py` crate.
All tests here are CI-gated (the `py` job in `.github/workflows/ci.yml`).
"""

import json
import pytest

import dry

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_CORNER_DESIGN = (
    dry.Design()
    .geometry(0.6, 0.2)
    .extruder(True)
    # Two printing segments meeting at a sharp 90° right-angle corner —
    # exactly the topology where adaptive_speed / junction-velocity shaping bites.
    .point(0, 0, 0.2)
    .point(10, 0, 0.2)
    .point(10, 10, 0.2)
)

# Kinematics with a tight junction-velocity cap: 5 mm/s SCV.
_KIN_TIGHT = {"max_acceleration_mm_s2": 3000.0, "max_junction_velocity_mm_s": 5.0}


# ---------------------------------------------------------------------------
# balanced_ir: output differs from optimized_ir for a cornering design
# ---------------------------------------------------------------------------

def test_balanced_ir_no_kinematics_returns_ir():
    """balanced_ir(kinematics=None) falls back to safe_pipeline — returns a valid IR."""
    ir = _CORNER_DESIGN.balanced_ir()
    assert ir["version"] == 0
    assert len(ir["segments"]) >= 1


def test_balanced_ir_with_kinematics_returns_ir():
    """balanced_ir with kinematics dict resolves and returns a valid IR."""
    ir = _CORNER_DESIGN.balanced_ir(kinematics=_KIN_TIGHT)
    assert ir["version"] == 0
    assert len(ir["segments"]) >= 1


def test_balanced_ir_differs_from_optimized_ir_for_corner():
    """With a tight junction-velocity cap, the balanced IR must have at least one segment with a
    lower speed than the corresponding optimized IR — adaptive_speed shaping has effect."""
    opt = _CORNER_DESIGN.optimized_ir()
    bal = _CORNER_DESIGN.balanced_ir(kinematics=_KIN_TIGHT)

    # Both must have segments.
    assert opt["segments"] and bal["segments"]

    # Collect printing (non-travel) segment speeds from each IR.
    opt_speeds = [s["speed"] for s in opt["segments"] if not s.get("travel", False)]
    bal_speeds = [s["speed"] for s in bal["segments"] if not s.get("travel", False)]

    # The balanced pipeline with a 5 mm/s SCV cap MUST lower at least one corner speed
    # compared to the unmodified optimized pipeline (which has no junction shaping).
    assert any(
        b < o for b, o in zip(bal_speeds, opt_speeds)
    ), (
        f"balanced_ir with tight junction-velocity cap should have at least one lower speed "
        f"than optimized_ir.\nopt speeds: {opt_speeds}\nbal speeds: {bal_speeds}"
    )


def test_balanced_ir_without_kinematics_matches_safe_pipeline():
    """balanced_ir(kinematics=None) must match the safe_pipeline path (same as no-kinematics
    balanced); it should still differ from optimized_ir only if the design has arcs/corners
    that arc_fit would not merge — for a simple corner, safe == optimized."""
    safe_ir = _CORNER_DESIGN.balanced_ir(kinematics=None)
    opt_ir = _CORNER_DESIGN.optimized_ir()
    # Both go through arc_fit + merge_collinear; for this non-collinear corner design
    # the segment count and geometry must be the same.
    assert len(safe_ir["segments"]) == len(opt_ir["segments"])


# ---------------------------------------------------------------------------
# balanced_ir: raw native binding
# ---------------------------------------------------------------------------

def test_native_resolve_balanced_ir_string_round_trip():
    """The raw `_native.resolve_balanced_ir` function is registered and returns a JSON string."""
    from dry import _native
    ops_json = json.dumps([
        {"op": "geometry", "width": 0.6, "height": 0.2},
        {"op": "extruder", "on": True},
        {"op": "move", "x": 0, "y": 0, "z": 0.2},
        {"op": "move", "x": 10, "y": 0, "z": 0.2},
        {"op": "move", "x": 10, "y": 10, "z": 0.2},
    ])
    params_json = json.dumps({"print_speed": 1000.0, "travel_speed": 8000.0, "dia": 1.75})

    # No kinematics — falls back to safe_pipeline.
    ir_str = _native.resolve_balanced_ir(ops_json, params_json)
    ir = json.loads(ir_str)
    assert ir["version"] == 0

    # With kinematics JSON string.
    kin_json = json.dumps(_KIN_TIGHT)
    ir_kin_str = _native.resolve_balanced_ir(ops_json, params_json, kin_json)
    ir_kin = json.loads(ir_kin_str)
    assert ir_kin["version"] == 0


def test_native_resolve_balanced_ir_bad_kinematics_raises():
    """A non-empty invalid kinematics_json raises ValueError — never a panic."""
    from dry import _native
    ops_json = json.dumps([
        {"op": "geometry", "width": 0.6, "height": 0.2},
        {"op": "extruder", "on": True},
        {"op": "move", "x": 0, "y": 0, "z": 0.2},
        {"op": "move", "x": 10, "y": 0, "z": 0.2},
    ])
    params_json = json.dumps({"print_speed": 1000.0, "travel_speed": 8000.0, "dia": 1.75})
    with pytest.raises(ValueError, match="kinematics_json"):
        _native.resolve_balanced_ir(ops_json, params_json, "not-valid-json{{{")


# ---------------------------------------------------------------------------
# resolve_verify: kinematics_json kwarg surfaces peak-acceleration finding
# ---------------------------------------------------------------------------

def _arc_design():
    """A design with a CCW arc of radius 10 mm at speed 3000 mm/min.
    Centripetal acceleration: a = v²/r = (3000/60)² / 10 = 50² / 10 = 250 mm/s².
    A max_acceleration_mm_s2 limit of 100 will trigger the peak-acceleration rule.
    """
    return (
        dry.Design()
        .geometry(0.6, 0.2)
        .extruder(True)
        .speed(3000)      # 50 mm/s → a = 250 mm/s² on r=10 arc
        .point(10, 0, 0.2)
        .arc(cx=0, cy=0, x=0, y=10, clockwise=False)
    )


def test_verify_kinematics_peak_acceleration_finding():
    """A tight max_acceleration_mm_s2 surfaces a peak-acceleration finding via verify(kinematics=…)."""
    d = _arc_design()
    report = d.verify(kinematics={"max_acceleration_mm_s2": 100.0})
    rules = {f["rule"] for f in report["findings"]}
    assert "peak-acceleration" in rules, (
        f"expected 'peak-acceleration' finding in report, got: {report['findings']}"
    )


def test_verify_no_kinematics_no_acceleration_finding():
    """Without kinematics, the peak-acceleration rule is disabled (no finding for the same design)."""
    d = _arc_design()
    report = d.verify()
    rules = {f["rule"] for f in report["findings"]}
    assert "peak-acceleration" not in rules, (
        f"peak-acceleration must not fire when kinematics=None, got: {report['findings']}"
    )


def test_verify_kinematics_junction_velocity_finding():
    """A tight max_junction_velocity_mm_s with a speed-change corner fires junction-velocity."""
    # Two contiguous printing segments at different speeds: Δv = |100 - 50| = 50 mm/s > 25 limit.
    d = (
        dry.Design()
        .geometry(0.6, 0.2)
        .extruder(True)
        .speed(6000)       # 100 mm/s
        .point(0, 0, 0.2)
        .point(10, 0, 0.2)
        .speed(3000)       # 50 mm/s — Δv = 50 mm/s at the junction
        .point(10, 10, 0.2)
    )
    report = d.verify(kinematics={"max_junction_velocity_mm_s": 25.0})
    rules = {f["rule"] for f in report["findings"]}
    assert "junction-velocity" in rules, (
        f"expected 'junction-velocity' finding, got: {report['findings']}"
    )


def test_verify_kinematics_does_not_break_existing_contracts():
    """Passing kinematics alongside existing contracts must not suppress other findings."""
    d = (
        dry.Design()
        .geometry(0.6, 0.2)
        .extruder(True)
        .point(0, 0, 0.2)
        .point(150, 0, 0.2)  # out of a 100 mm wide bounds → bounds finding
    )
    report = d.verify(
        bounds=[[0, 100], [0, 100], [0, 50]],
        kinematics={"max_acceleration_mm_s2": 3000.0},
    )
    rules = {f["rule"] for f in report["findings"]}
    assert "bounds" in rules, (
        f"bounds finding must still fire alongside kinematics kwarg: {report['findings']}"
    )


def test_verify_kinematics_backward_compat_no_kwarg():
    """Existing callers that omit kinematics=… continue to work without change."""
    d = dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(10, 0, 0.2)
    report = d.verify(max_flow=15.0, bounds="0,100,0,100,0,50")
    assert report["findings"] == [], (
        f"existing contracts API must still be clean: {report['findings']}"
    )

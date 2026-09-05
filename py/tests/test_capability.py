import pytest
from dry import Design

def test_python_preflight_compatibility_check():
    design = (
        Design()
        .point(0, 0, 0)
        .speed(3500)
        .point(250, 50, 10)
    )

    caps = {
        "x_range": [0, 200],
        "y_range": [0, 200],
        "z_range": [0, 100],
        "max_feedrate": 3000,
    }

    report = design.check_compatibility(caps)
    assert not report["compatible"]
    codes = [f["code"] for f in report["findings"]]
    assert "OUT_OF_BOUNDS_X" in codes
    assert "EXCEEDS_MAX_FEEDRATE" in codes

def test_an_arc_whose_circle_leaves_the_envelope_is_refused():
    """The engine bounds an arc by its full circle; the SDK must not report it compatible.

    Both endpoints sit inside X [0, 80]. The circle about (50, 50) with radius 40 spans X [10, 90]
    and leaves it, so only the arc rule can refuse this program — which is exactly what a check that
    walks segment endpoints alone gets wrong.
    """
    design = Design().point(50, 10, 0).arc(50, 50, x=50, y=90, z=0)

    caps = {
        "x_range": [0, 80],
        "y_range": [0, 100],
        "z_range": [0, 50],
    }

    report = design.check_compatibility(caps)
    codes = [f["code"] for f in report["findings"]]
    assert "ARC_OUT_OF_BOUNDS_X" in codes, codes
    assert "OUT_OF_BOUNDS_X" not in codes, codes
    assert report["compatible"] is False


def test_python_preflight_compatibility_pass():
    design = (
        Design()
        .point(10, 10, 0)
        .speed(1500)
        .point(50, 50, 10)
    )

    caps = {
        "x_range": [0, 200],
        "y_range": [0, 200],
        "z_range": [0, 100],
        "max_feedrate": 10000,
    }

    report = design.check_compatibility(caps)
    assert report["compatible"]
    assert len(report["findings"]) == 0

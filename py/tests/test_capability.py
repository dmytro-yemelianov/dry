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

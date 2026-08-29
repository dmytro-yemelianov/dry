"""Unit test for OctoPrint verification hook."""

import tempfile
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).parent))
from __init__ import verify_uploaded_gcode

SAMPLE_GCODE = """G21 ; set units to millimeters
G90 ; use absolute coordinates
M83 ; use relative extrusion
G1 Z0.200 F7800.000
G1 X10.0 Y10.0 E0.5 F1800.0
G1 X50.0 Y10.0 E1.5 F1800.0
"""


def test_verify_uploaded_gcode():
    with tempfile.NamedTemporaryFile("w", suffix=".gcode", delete=False) as f:
        f.write(SAMPLE_GCODE)
        temp_path = f.name

    try:
        res = verify_uploaded_gcode(temp_path, {"max_flow": 30.0})
        assert res["passed"] is True
        assert "findings" in res
    finally:
        Path(temp_path).unlink(missing_ok=True)


if __name__ == "__main__":
    test_verify_uploaded_gcode()
    print("OctoPrint plugin verification test passed!")

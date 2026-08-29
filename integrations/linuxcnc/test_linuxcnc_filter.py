"""Unit test for LinuxCNC filter."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from dry_linuxcnc_filter import filter_linuxcnc_gcode

SAMPLE_NGC = """G21 G90 G17 G54
G0 X0 Y0 Z10.0
G1 Z-2.0 F300
G1 X50.0 F1200
M5
M30
"""


def test_filter_linuxcnc_gcode():
    res = filter_linuxcnc_gcode(SAMPLE_NGC, max_feedrate=3000.0)
    assert "(Filtered with Dry LinuxCNC Safety Filter" in res
    assert "G1 X50.0 F1200" in res


if __name__ == "__main__":
    test_filter_linuxcnc_gcode()
    print("LinuxCNC filter tests passed!")

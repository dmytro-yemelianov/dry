"""Unit test for Blender Addon standalone helper functions."""

import sys
from pathlib import Path

# Add integration folder to path
sys.path.insert(0, str(Path(__file__).parent))
import __init__ as blender_addon


def test_generate_tpms_curves():
    contours = blender_addon.generate_tpms_curves(
        surface_name="gyroid",
        cell_size=10.0,
        iso_level=0.0,
        layer_height=1.0,
        size_x=20.0,
        size_y=20.0,
        size_z=5.0,
    )
    assert len(contours) > 0
    # Every contour must have 3D points
    for c in contours:
        assert len(c) > 0
        for pt in c:
            assert len(pt) == 3


if __name__ == "__main__":
    test_generate_tpms_curves()
    print("Blender addon helper test passed!")

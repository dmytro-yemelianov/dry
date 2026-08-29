"""Unit test for Autodesk Fusion 360 add-in helper functions."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from dry_fusion_addin import generate_tpms_lattice_for_bounds


def test_generate_tpms_lattice_for_bounds():
    bounds = [0.0, 0.0, 0.0, 30.0, 30.0, 10.0]
    ops = generate_tpms_lattice_for_bounds(
        surface="gyroid",
        bounds=bounds,
        cell_size=10.0,
        iso_level=0.0,
        layer_height=0.5,
    )
    assert len(ops) > 0
    has_moves = any(op.get("op") == "move" for op in ops)
    assert has_moves is True


if __name__ == "__main__":
    test_generate_tpms_lattice_for_bounds()
    print("Fusion 360 integration tests passed!")

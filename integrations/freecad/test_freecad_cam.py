"""Unit test for FreeCAD CAM integration."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from dry_freecad_cam import convert_dry_ops_to_path_commands, generate_freecad_pocket


def test_convert_dry_ops_to_path_commands():
    ops = [
        {"op": "move", "x": 10.0, "y": 20.0, "z": 5.0, "speed": 1500.0},
        {"op": "arc", "x": 30.0, "y": 20.0, "z": 5.0, "cx": 20.0, "cy": 20.0, "clockwise": False},
        {"op": "dwell", "seconds": 2.0},
    ]
    cmds = convert_dry_ops_to_path_commands(ops)
    assert len(cmds) == 3
    assert cmds[0]["name"] == "G1"
    assert cmds[0]["parameters"]["X"] == 10.0
    assert cmds[1]["name"] == "G3"
    assert cmds[2]["name"] == "G4"


def test_generate_freecad_pocket():
    ops = generate_freecad_pocket(
        width=40.0,
        height=30.0,
        depth=4.0,
        tool_diameter=5.0,
        stepover=0.5,
        stepdown=2.0,
        feedrate=1000.0,
    )
    assert len(ops) > 0
    cmds = convert_dry_ops_to_path_commands(ops)
    assert len(cmds) > 0


if __name__ == "__main__":
    test_convert_dry_ops_to_path_commands()
    test_generate_freecad_pocket()
    print("FreeCAD CAM integration tests passed!")

"""Unit test for RoboDK robotics bridge."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from dry_robodk_bridge import vector_to_euler_zyx, convert_toolpath_to_robodk_targets, check_dual_robot_clearance


def test_vector_to_euler_zyx():
    # Identity (+Z)
    a, b, c = vector_to_euler_zyx(0.0, 0.0, 1.0)
    assert abs(b) < 1e-4

    # 45 deg tilt in XZ
    a45, b45, c45 = vector_to_euler_zyx(1.0, 0.0, 1.0)
    assert abs(b45 - 45.0) < 1e-4


def test_convert_toolpath_to_robodk_targets():
    sample_tp = {
        "segments": [
            {"end": [0.0, 0.0, 10.0], "orientation": [0.0, 0.0, 1.0], "speed": 1200.0, "travel": True},
            {"end": [50.0, 0.0, 10.0], "orientation": [0.0, 1.0, 0.0], "speed": 600.0, "travel": False},
        ]
    }
    targets = convert_toolpath_to_robodk_targets(sample_tp)
    assert len(targets) == 2
    assert targets[0]["is_move_linear"] is False
    assert targets[1]["is_move_linear"] is True
    assert targets[1]["x"] == 50.0


def test_check_dual_robot_clearance():
    # Two close segments (distance 10mm < 25mm clearance)
    r1 = [([0.0, 0.0, 0.0], [50.0, 0.0, 0.0])]
    r2 = [([0.0, 0.0, 10.0], [50.0, 0.0, 10.0])]

    collisions = check_dual_robot_clearance(r1, r2, min_clearance_mm=25.0)
    assert len(collisions) == 1
    assert abs(collisions[0]["distance_mm"] - 10.0) < 1e-4


if __name__ == "__main__":
    test_vector_to_euler_zyx()
    test_convert_toolpath_to_robodk_targets()
    test_check_dual_robot_clearance()
    print("All RoboDK bridge tests passed!")

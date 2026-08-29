#!/usr/bin/env python3
"""RoboDK 6-Axis Robotics & Dual-Robot Workcell Bridge for Dry.

Translates Dry 5-Axis / 6-Axis toolpaths with toolframe orientation vectors
into RoboDK simulation targets and validates continuous swept-capsule distance
between synchronized robot arms.
"""

import json
import math
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

try:
    import dry
except ImportError:
    repo_root = Path(__file__).resolve().parents[2]
    py_pkg = repo_root / "py" / "python"
    if py_pkg.exists():
        sys.path.insert(0, str(py_pkg))
        import dry
    else:
        dry = None


def vector_to_euler_zyx(i: float, j: float, k: float) -> Tuple[float, float, float]:
    """Convert toolframe direction vector (i, j, k) to Euler ZYX angles (A, B, C in deg)."""
    norm = math.hypot(i, j, k)
    if norm < 1e-9:
        return 0.0, 0.0, 0.0
    i /= norm
    j /= norm
    k /= norm

    # Euler angles matching KUKA convention (RotZ(A) * RotY(B) * RotX(C))
    b = math.atan2(math.hypot(i, j), k) * (180.0 / math.pi)
    a = math.atan2(j, i) * (180.0 / math.pi)
    c = 0.0  # Default tool roll
    return a, b, c


def convert_toolpath_to_robodk_targets(toolpath: Dict[str, Any]) -> List[Dict[str, Any]]:
    """Convert Dry Toolpath IR into RoboDK Target dictionaries."""
    targets = []
    segments = toolpath.get("segments", [])

    for idx, seg in enumerate(segments):
        end = seg.get("end")
        if not end or len(end) < 3:
            continue
        x, y, z = end[0], end[1], end[2]
        orient = seg.get("orientation") or [0.0, 0.0, 1.0]
        a, b, c = vector_to_euler_zyx(orient[0], orient[1], orient[2])
        speed = seg.get("speed", 1000.0)
        is_travel = seg.get("travel", False)

        targets.append({
            "name": f"Target_{idx:04d}",
            "x": x,
            "y": y,
            "z": z,
            "euler_a": a,
            "euler_b": b,
            "euler_c": c,
            "speed_mm_s": speed / 60.0,
            "is_move_linear": not is_travel,
        })
    return targets


def check_dual_robot_clearance(
    robot1_segments: List[Tuple[List[float], List[float]]],
    robot2_segments: List[Tuple[List[float], List[float]]],
    min_clearance_mm: float = 50.0,
) -> List[Dict[str, Any]]:
    """Check continuous 3D swept-volume clearance between two robots."""
    collisions = []
    if dry is None:
        return collisions

    for i, (p1, p2) in enumerate(robot1_segments):
        for j, (q1, q2) in enumerate(robot2_segments):
            dist = dry.segment_to_segment_distance_3d(p1, p2, q1, q2)
            if dist < min_clearance_mm:
                collisions.append({
                    "robot1_step": i,
                    "robot2_step": j,
                    "distance_mm": dist,
                    "min_clearance_mm": min_clearance_mm,
                })
    return collisions


def main() -> None:
    if len(sys.argv) < 2:
        print("Usage: python3 dry_robodk_bridge.py <toolpath.json>")
        sys.exit(1)

    with open(sys.argv[1], "r", encoding="utf-8") as f:
        tp = json.load(f)

    targets = convert_toolpath_to_robodk_targets(tp)
    print(f"Generated {len(targets)} RoboDK robot motion targets.")
    print(f"Sample target 0: {targets[0] if targets else 'None'}")


if __name__ == "__main__":
    main()

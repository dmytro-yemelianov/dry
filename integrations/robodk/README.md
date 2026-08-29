# Dry RoboDK Robotics Bridge

Connects Dry 5-axis/6-axis toolpaths with **RoboDK** for offline robot programming (OLP), 3D multi-axis simulation, and dual-robot workcell collision validation.

---

## 1. Overview

- **Orientation Mapping**: Maps $\{X, Y, Z, I, J, K\}$ tool orientation channels to standard robot Euler $\{A, B, C\}$ angles.
- **Dual-Robot Swept-Volume Collision Detection**: Continuous 3D segment-to-segment distance calculations between dual robot arms (e.g. KUKA + ABB synchronized cells).
- **Target Export**: Emits linear `MoveL` and joint `MoveJ` waypoints directly into RoboDK projects.

---

## 2. Usage in RoboDK

Inside RoboDK's Python script editor:
```python
from dry_robodk_bridge import convert_toolpath_to_robodk_targets
import dry

# Author or import toolpath
d = dry.Design().speed(1500).point(0, 0, 10).orient(0, 0.707, 0.707).point(50, 0, 10)
tp = d.ir()

targets = convert_toolpath_to_robodk_targets(tp)
# Send targets to active RoboDK robot
```

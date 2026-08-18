"""Public dimensional quantity constructors (D1.1).

Normalizes user-facing dimensional values into Dry's canonical internal units:
- Length: millimetres (mm)
- Angle: radians (rad)
- Feedrate: mm/min (standard G-code F value)
- Temperature: degrees Celsius (°C)
- Time: seconds (s)
"""

import math
from typing import Union

Number = Union[int, float]


def mm(value: Number) -> float:
    """Length in millimetres (canonical unit)."""
    return float(value)


def cm(value: Number) -> float:
    """Length in centimetres -> converted to mm."""
    return float(value) * 10.0


def inch(value: Number) -> float:
    """Length in inches -> converted to mm."""
    return float(value) * 25.4


def deg(value: Number) -> float:
    """Angle in degrees -> converted to radians."""
    return math.radians(float(value))


def rad(value: Number) -> float:
    """Angle in radians (canonical unit)."""
    return float(value)


def mm_s(value: Number) -> float:
    """Feedrate in mm/s -> converted to mm/min (canonical G-code F value)."""
    return float(value) * 60.0


def mm_min(value: Number) -> float:
    """Feedrate in mm/min (canonical unit)."""
    return float(value)


def celsius(value: Number) -> float:
    """Temperature in degrees Celsius (canonical unit)."""
    return float(value)


def s(value: Number) -> float:
    """Duration in seconds (canonical unit)."""
    return float(value)


def ms(value: Number) -> float:
    """Duration in milliseconds -> converted to seconds."""
    return float(value) / 1000.0

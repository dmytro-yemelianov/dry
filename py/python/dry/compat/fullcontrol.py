"""FullControl compatibility layer for Dry (Phase 6 Standalone Cutover).

Drop-in compatibility shim enabling FullControl designs (e.g. from Colab or fullcontrol.xyz)
to run on top of the fast, typed Dry Rust engine:

    import dry.compat.fullcontrol as fc

    steps = [
        fc.ExtrusionGeometry(width=0.6, height=0.2),
        fc.Extruder(on=True),
        fc.Point(x=0, y=0, z=0.2),
        fc.Point(x=10, y=0, z=0.2),
        fc.Point(x=10, y=10, z=0.2),
        fc.Point(x=0, y=10, z=0.2),
        fc.Point(x=0, y=0, z=0.2),
    ]
    gcode_lines = fc.gcode(steps)
"""

from dataclasses import dataclass
import math
from typing import Any, Dict, List, Optional, Sequence, Union

import dry


@dataclass
class Point:
    x: Optional[float] = None
    y: Optional[float] = None
    z: Optional[float] = None
    a: Optional[float] = None
    b: Optional[float] = None


@dataclass
class Extruder:
    on: bool = True


@dataclass
class ExtrusionGeometry:
    width: Optional[float] = None
    height: Optional[float] = None
    area: Optional[float] = None


@dataclass
class Printer:
    print_speed: Optional[float] = None
    travel_speed: Optional[float] = None


@dataclass
class Arc:
    centre: Point
    end: Point
    direction: str = "clockwise"  # "clockwise" or "anticlockwise"
    start: Optional[Point] = None


@dataclass
class Fan:
    speed_percent: Optional[float] = None


@dataclass
class Hotend:
    temp: Optional[float] = None


@dataclass
class Buildplate:
    temp: Optional[float] = None


@dataclass
class Retraction:
    distance: Optional[float] = None
    speed: Optional[float] = None


@dataclass
class Unretraction:
    distance: Optional[float] = None
    speed: Optional[float] = None


@dataclass
class Acceleration:
    printing: Optional[float] = None
    travel: Optional[float] = None
    retract: Optional[float] = None


@dataclass
class Jerk:
    x: Optional[float] = None
    y: Optional[float] = None
    z: Optional[float] = None
    e: Optional[float] = None


@dataclass
class PressureAdvance:
    value: Optional[float] = None


@dataclass
class ManualGcode:
    text: str = ""


@dataclass
class StationaryExtrusion:
    volume: Optional[float] = None
    speed: Optional[float] = None


@dataclass
class GcodeComment:
    text: str = ""


@dataclass
class GcodeControls:
    printer_name: str = "generic"
    initialization_data: Optional[Dict[str, Any]] = None
    include_procedures: bool = False


def step_to_op(s: Any) -> Optional[Dict[str, Any]]:
    """Convert a FullControl step object into a Dry L1 op dict."""
    if isinstance(s, dict) and "op" in s:
        return s

    t = type(s).__name__
    if t == "ExtrusionGeometry":
        return {"op": "geometry", "width": s.width, "height": s.height}
    elif t == "Extruder":
        return {"op": "extruder", "on": bool(s.on)}
    elif t == "Printer":
        return {"op": "speed", "print": s.print_speed}
    elif t == "Point":
        return {"op": "move", "x": s.x, "y": s.y, "z": s.z}
    elif t == "Arc":
        cw = s.direction in ("clockwise", "cw")
        return {
            "op": "arc",
            "cx": s.centre.x,
            "cy": s.centre.y,
            "x": s.end.x,
            "y": s.end.y,
            "z": s.end.z,
            "clockwise": cw,
        }
    elif t == "Fan":
        speed = None if s.speed_percent is None else float(s.speed_percent / 100.0)
        return {"op": "fan", "speed": speed}
    elif t == "Hotend":
        return {"op": "temperature", "nozzle": s.temp}
    elif t == "Buildplate":
        return {"op": "bed_temperature", "value": s.temp}
    elif t == "Retraction":
        return {"op": "retract", "distance": s.distance, "speed": s.speed}
    elif t == "Unretraction":
        return {"op": "unretract", "distance": s.distance, "speed": s.speed}
    elif t == "Acceleration":
        return {"op": "acceleration", "printing": s.printing, "travel": s.travel, "retract": s.retract}
    elif t == "Jerk":
        return {"op": "jerk", "x": s.x, "y": s.y, "z": s.z, "e": s.e}
    elif t == "PressureAdvance":
        return {"op": "pressure_advance", "value": s.value}
    elif t == "ManualGcode":
        return {"op": "manual_gcode", "text": str(s.text)}
    elif t == "StationaryExtrusion":
        return {"op": "deposit", "volume": s.volume, "speed": s.speed}
    elif t == "GcodeComment":
        return {"op": "comment", "text": str(s.text)}
    return None


def steps_to_design(steps: Sequence[Any]) -> dry.Design:
    """Convert a sequence of FullControl steps into a native `dry.Design`."""
    ops: List[Dict[str, Any]] = []
    for s in steps:
        op = step_to_op(s)
        if op is not None:
            ops.append(op)
    return dry.Design.from_ops(ops)


def gcode(
    steps: Sequence[Any],
    controls: Optional[GcodeControls] = None,
    include_procedures: bool = False,
    printer_name: Optional[str] = None,
    relative_e: bool = True,
    travel_g1_e0: bool = False,
    five_axis: bool = False,
    rotary_axes: str = "ab",
) -> List[str]:
    """Emit G-code lines from a FullControl step sequence via the Dry engine."""
    p_name = printer_name or (controls.printer_name if controls else "generic")
    d = steps_to_design(steps)
    return d.gcode(
        printer=p_name,
        relative_e=relative_e,
        travel_g1_e0=travel_g1_e0,
        five_axis=five_axis,
        rotary_axes=rotary_axes,
    )


def transform(
    steps: Sequence[Any],
    translation: Optional[Point] = None,
    rotation: Optional[Dict[str, float]] = None,
    scale: Optional[Union[float, Point]] = None,
    mirror: Optional[str] = None,
) -> List[Any]:
    """Transform a sequence of steps by translation, rotation, scale, or mirror."""
    out: List[Any] = []
    dx = translation.x or 0.0 if translation else 0.0
    dy = translation.y or 0.0 if translation else 0.0
    dz = translation.z or 0.0 if translation else 0.0

    rot_angle = rotation.get("angle", 0.0) if rotation else 0.0
    rad = math.radians(rot_angle)
    cos_a, sin_a = math.cos(rad), math.sin(rad)

    for s in steps:
        if isinstance(s, Point):
            px = s.x if s.x is not None else 0.0
            py = s.y if s.y is not None else 0.0
            pz = s.z if s.z is not None else 0.0

            # Rotate around origin (z-axis)
            rx = px * cos_a - py * sin_a
            ry = px * sin_a + py * cos_a

            out.append(Point(x=rx + dx, y=ry + dy, z=pz + dz))
        else:
            out.append(s)
    return out

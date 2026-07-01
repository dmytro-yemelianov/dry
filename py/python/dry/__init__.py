"""Dry — author algorithmic machine toolpaths in Python.

A thin, logic-free front-end onto the Dry engine (Rust, via the `_native` extension). You build an L1
``Design`` with the ergonomic builders; ``resolve``/``emit``/``simulate`` run entirely in the engine.

    import dry
    d = (dry.Design()
         .geometry(width=0.6, height=0.2)
         .extruder(on=True)
         .point(0, 0, 0.2).point(10, 0, 0.2).point(10, 10, 0.2).point(0, 10, 0.2).point(0, 0, 0.2))
    print("\\n".join(d.gcode()))      # motion g-code
    print(d.simulate())               # metrics
"""
from __future__ import annotations

import json
from typing import Any, Dict, List, Mapping, Optional, Sequence, Tuple, Union

from . import _native  # the Rust engine (PyO3)

__all__ = [
    "Bounds",
    "Design",
    "Kinematics",
    "Metrics",
    "Number",
    "Op",
    "Point",
    "PRINTERS",
    "Range",
    "Report",
    "ResolveParams",
    "Toolpath",
    "TpmsOptions",
    "TPMS_SURFACES",
    "tpms_gcode",
]

Number = Union[int, float]
Point = Sequence[Number]
Bounds = Union[Sequence[Sequence[Number]], str]
Range = Union[Sequence[Number], str]
Kinematics = Mapping[str, Number]
ResolveParams = Mapping[str, Number]
Op = Dict[str, Any]
Metrics = Dict[str, Any]
Toolpath = Dict[str, Any]
Report = Dict[str, Any]
TpmsOptions = Mapping[str, Any]

# The TPMS surfaces the engine can slice (kebab-case, matching the `surface` option / TS SDK).
TPMS_SURFACES: Tuple[str, ...] = (
    "gyroid", "schwarz-p", "schwarz-d", "iwp", "neovius",
    "fischer-koch-s", "fischer-koch-y", "frd", "lidinoid", "split-p",
)

# Device defaults (the lowering's print/travel feedrate + filament diameter). Mirrors the engine's
# ResolveParams; more profiles are added as the device-profile work lands.
PRINTERS: Dict[str, ResolveParams] = {
    "generic": {"print_speed": 1000.0, "travel_speed": 8000.0, "dia": 1.75},
}


class Design:
    """An L1 design: a chain of authoring ops. Builders return ``self`` for fluent use."""

    def __init__(self) -> None:
        self.ops: List[Op] = []

    def geometry(self, width: Number, height: Number) -> "Design":
        "Set the extrusion bead cross-section (mm)."
        self.ops.append({"op": "geometry", "width": width, "height": height})
        return self

    def extruder(self, on: bool) -> "Design":
        "Turn the extruder on/off (off => subsequent moves are travels)."
        self.ops.append({"op": "extruder", "on": bool(on)})
        return self

    def speed(self, print_speed: Number) -> "Design":
        "Set the print feedrate (mm/min)."
        self.ops.append({"op": "speed", "print": print_speed})
        return self

    def point(
        self,
        x: Optional[Number] = None,
        y: Optional[Number] = None,
        z: Optional[Number] = None,
    ) -> "Design":
        "Move to a point; an omitted axis is inherited from the running position."
        self.ops.append({"op": "move", "x": x, "y": y, "z": z})
        return self

    def arc(
        self,
        cx: Number,
        cy: Number,
        x: Optional[Number] = None,
        y: Optional[Number] = None,
        z: Optional[Number] = None,
        clockwise: bool = False,
    ) -> "Design":
        "A circular arc about (cx, cy) to an end point; clockwise => G2, else G3."
        self.ops.append({"op": "arc", "cx": cx, "cy": cy, "x": x, "y": y, "z": z,
                         "clockwise": bool(clockwise)})
        return self

    def spline(self, points: Sequence[Point]) -> "Design":
        "A Catmull-Rom spline from the running position through each (x, y, z) control point."
        self.ops.append({"op": "spline",
                         "points": [[p[0], p[1], p[2]] for p in points]})
        return self

    # ---- process channels (§3): typed, defaulted, propagated by the engine ----
    def temperature(self, nozzle: Number) -> "Design":
        "Set the nozzle temperature channel (°C)."
        self.ops.append({"op": "temperature", "nozzle": nozzle})
        return self

    def fan(self, speed: Number) -> "Design":
        "Set the part-cooling fan channel (0..1)."
        self.ops.append({"op": "fan", "speed": speed})
        return self

    def flow(self, ratio: Number) -> "Design":
        "Set the flow multiplier channel (scales deposited volume; default 1.0)."
        self.ops.append({"op": "flow", "ratio": ratio})
        return self

    def tool(self, index: int) -> "Design":
        "Set the active tool channel."
        self.ops.append({"op": "tool", "index": int(index)})
        return self

    def orient(self, i: Number, j: Number, k: Number) -> "Design":
        "Set the toolframe orientation: the tool-direction vector (i, j, k). Identity is +Z."
        self.ops.append({"op": "orient", "i": i, "j": j, "k": k})
        return self

    def dwell(self, seconds: Number) -> "Design":
        "Pause in place for `seconds` (emits a G4 dwell)."
        self.ops.append({"op": "dwell", "seconds": seconds})
        return self

    def manual_gcode(self, text: str) -> "Design":
        "Inject verbatim custom G-code."
        self.ops.append({"op": "manual_gcode", "text": str(text)})
        return self

    def deposit(self, volume: Number, speed: Number) -> "Design":
        "Stationary extrusion of a set volume (mm³) at feedrate (mm/min)."
        self.ops.append({"op": "deposit", "volume": float(volume), "speed": float(speed)})
        return self

    def retract(
        self,
        distance: Optional[Number] = None,
        speed: Optional[Number] = None,
    ) -> "Design":
        "Retract filament."
        op = {"op": "retract"}
        if distance is not None:
            op["distance"] = float(distance)
        if speed is not None:
            op["speed"] = float(speed)
        self.ops.append(op)
        return self

    def unretract(
        self,
        distance: Optional[Number] = None,
        speed: Optional[Number] = None,
    ) -> "Design":
        "Prime filament back after a retraction."
        op = {"op": "unretract"}
        if distance is not None:
            op["distance"] = float(distance)
        if speed is not None:
            op["speed"] = float(speed)
        self.ops.append(op)
        return self

    # ---- engine calls ----
    def gcode(
        self,
        printer: str = "generic",
        relative_e: bool = True,
        travel_g1_e0: bool = False,
        five_axis: bool = False,
        rotary_axes: str = "ab",
        kinematics: Optional[str] = None,
    ) -> List[str]:
        """Resolve + emit motion g-code (a list of lines).

        `rotary_axes` is the rotary-axes selector — the ab/ac/bc STRING choosing which two rotary
        axes carry the toolframe orientation in 5-axis emit (`five_axis=True`). NOTE: this is NOT the
        machine motion-limits object (see ``balanced_ir`` / ``verify``'s ``kinematics`` for that).
        `kinematics` is a deprecated alias for `rotary_axes`, kept for backward compatibility; when
        provided (not ``None``) it takes precedence.
        """
        rotary = kinematics if kinematics is not None else rotary_axes
        return _native.resolve_gcode(
            json.dumps(self.ops),
            _params(printer),
            relative_e,
            bool(travel_g1_e0),
            bool(five_axis),
            str(rotary)
        )

    def simulate(self, printer: str = "generic") -> Metrics:
        "Resolve + simulate; returns a metrics dict (time, distances, material, peak flow)."
        return json.loads(_native.resolve_metrics(json.dumps(self.ops), _params(printer)))

    def ir(self, printer: str = "generic") -> Toolpath:
        "Resolve to the L2 Dry IR; returns a dict ({version, segments})."
        return json.loads(_native.resolve_ir(json.dumps(self.ops), _params(printer)))

    def optimized_ir(self, printer: str = "generic") -> Toolpath:
        "Resolve + optimize; returns a dict ({version, segments})."
        return json.loads(_native.resolve_optimized_ir(json.dumps(self.ops), _params(printer)))

    def balanced_ir(
        self,
        printer: str = "generic",
        kinematics: Optional[Kinematics] = None,
    ) -> Toolpath:
        """Resolve + balanced (kinematics-aware) optimize; returns a dict ({version, segments}).

        `kinematics` is a dict with optional keys `max_acceleration_mm_s2` (mm/s²) and
        `max_junction_velocity_mm_s` (mm/s). When supplied, the engine applies arc centripetal
        speed clamping and junction-velocity capping in addition to the standard optimizations
        (``balanced_pipeline``). When ``None`` (default), falls back to ``safe_pipeline``.
        """
        kin_json = json.dumps(kinematics) if kinematics is not None else None
        return json.loads(_native.resolve_balanced_ir(
            json.dumps(self.ops),
            _params(printer),
            kin_json,
        ))

    def binary(self, printer: str = "generic") -> bytes:
        "Resolve + encode to binary DRY1 format; returns a bytes object."
        return bytes(_native.resolve_binary(json.dumps(self.ops), _params(printer)))

    def verify(
        self,
        printer: str = "generic",
        max_flow: Optional[Number] = None,
        min_temp: Optional[Number] = None,
        bounds: Optional[Bounds] = None,
        monotonic_z: bool = False,
        speed_range: Optional[Range] = None,
        max_retraction_distance: Optional[Number] = None,
        max_retraction_speed: Optional[Number] = None,
        max_travel_without_retract: Optional[Number] = None,
        first_layer_height_range: Optional[Range] = None,
        first_layer_speed_range: Optional[Range] = None,
        kinematics: Optional[Kinematics] = None,
    ) -> Report:
        """Resolve + verify against machine-safety contracts; returns a report dict with findings.

        Structured limits cross to the engine as native typed contracts (no CSV round-trip):

          - `bounds` — build volume as `[[x0, x1], [y0, y1], [z0, z1]]` (mm). The legacy CSV string
            ``"x0,x1,y0,y1,z0,z1"`` is still accepted and parsed here for backward compatibility.
          - `speed_range` — extruding feedrate `[min, max]` (mm/min); the legacy ``"min,max"`` CSV
            string is still accepted.
          - `max_flow` (mm³/s), `min_temp` (°C), `monotonic_z` (bool).
          - `max_retraction_distance` (mm), `max_retraction_speed` (mm/min),
            `max_travel_without_retract` (mm) — retraction / stringing limits.
          - `first_layer_height_range`, `first_layer_speed_range` — first-layer adhesion limits, each
            `[min, max]` (or a ``"min,max"`` CSV string).
          - `kinematics` — dict with optional `max_acceleration_mm_s2` (mm/s²) and/or
            `max_junction_velocity_mm_s` (mm/s). When supplied, enables the ``peak-acceleration``
            and ``junction-velocity`` verify rules; ``None`` disables them.
        """
        kin_json = json.dumps(kinematics) if kinematics is not None else None
        return json.loads(_native.resolve_verify(
            json.dumps(self.ops),
            _params(printer),
            max_flow,
            min_temp,
            _bounds_to_list(bounds),
            bool(monotonic_z),
            _range_to_list(speed_range),
            max_retraction_distance,
            max_retraction_speed,
            max_travel_without_retract,
            _range_to_list(first_layer_height_range),
            _range_to_list(first_layer_speed_range),
            kin_json,
        ))


def _bounds_to_list(bounds: Optional[Bounds]) -> Any:
    """Normalise bounds to the structured `[[x0,x1],[y0,y1],[z0,z1]]` the binding expects.

    A structured list/tuple passes straight through (the binding validates its shape); a legacy
    ``"x0,x1,y0,y1,z0,z1"`` CSV string is parsed here; ``None`` stays ``None``.
    """
    if bounds is None or not isinstance(bounds, str):
        return bounds
    flat = [float(v) for v in bounds.split(",")]
    if len(flat) != 6:
        raise ValueError("bounds CSV must be 'x0,x1,y0,y1,z0,z1'")
    return [[flat[0], flat[1]], [flat[2], flat[3]], [flat[4], flat[5]]]


def _range_to_list(rng: Optional[Range]) -> Any:
    """Normalise a `[min, max]` range to the list the binding expects.

    A structured list/tuple passes straight through (the binding validates its shape); a legacy
    ``"min,max"`` CSV string is parsed here; ``None`` stays ``None``.
    """
    if rng is None or not isinstance(rng, str):
        return rng
    return [float(v) for v in rng.split(",")]


def tpms_gcode(
    options: Optional[TpmsOptions],
    printer: str = "generic",
    relative_e: bool = True,
    travel_g1_e0: bool = False,
    five_axis: bool = False,
    rotary_axes: str = "ab",
    kinematics: Optional[str] = None,
) -> List[str]:
    """Generate TPMS infill g-code (a list of lines) from an options dict.

    `options` is the TPMS option bundle with camelCase keys (matching the engine / TS SDK), e.g.
    ``{"surface": "schwarz-p", "cellSize": 12, "cellsX": 2}``. The `surface` is one of
    `TPMS_SURFACES` (default ``"gyroid"``); an unknown name raises ``ValueError``. The field math runs
    in the engine (libm), so output differs sub-micron from the TypeScript generator — there is no
    byte-identity contract between them.

    `rotary_axes` is the rotary-axes selector (the ab/ac/bc STRING) for 5-axis emit — NOT the machine
    motion-limits object. `kinematics` is a deprecated alias for `rotary_axes`, kept for backward
    compatibility; when provided (not ``None``) it takes precedence.
    """
    rotary = kinematics if kinematics is not None else rotary_axes
    return _native.resolve_tpms_gcode(
        json.dumps(options or {}),
        _params(printer),
        relative_e,
        bool(travel_g1_e0),
        bool(five_axis),
        str(rotary),
    )


def _params(printer: str) -> str:
    if printer not in PRINTERS:
        raise KeyError(f"unknown printer {printer!r}; known: {sorted(PRINTERS)}")
    return json.dumps(PRINTERS[printer])

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
import math
from typing import Any, Dict, List, Mapping, Optional, Sequence, Tuple, Union

from . import _native  # the Rust engine (PyO3)

__all__ = [
    "Bounds",
    "Design",
    "FeatureNode",
    "FeaturePose",
    "FeatureProgram",
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
    "feature",
    "group",
    "repeat",
    "tpms_ops",
    "tpms_gcode",
    "pocket_ops",
    "pocket_gcode",
    "PocketOptions",
    "drape_ops",
    "parse_obj_mesh",
    "slice_step_solid",
    "lathe_facing_ops",
    "lathe_turning_ops",
    "check_tool_holder_collision",
    "analyze_machining_physics",
    "optimize_five_axis_lookahead",
    "reverse_toolpath",
    "mm",
    "cm",
    "inch",
    "deg",
    "rad",
    "mm_s",
    "mm_min",
    "celsius",
    "s",
    "ms",
    "MachineProfile",
    "MachineCatalog",
    "BUILTIN_MACHINES",
    "toolpath_to_obj",
    "toolpath_to_svg",
    "toolpath_to_interactive_html",
]

from .visualizer import (
    toolpath_to_interactive_html,
    toolpath_to_obj,
    toolpath_to_svg,
)

from .machine import (
    BUILTIN_MACHINES,
    MachineCatalog,
    MachineProfile,
)
from .units import (
    celsius,
    cm,
    deg,
    inch,
    mm,
    mm_min,
    mm_s,
    ms,
    rad,
    s,
)

Number = Union[int, float]
Point = Sequence[Number]
Bounds = Union[Sequence[Sequence[Number]], str]
Range = Union[Sequence[Number], str]
Kinematics = Mapping[str, Number]
ResolveParams = Mapping[str, Number]
Op = Dict[str, Any]
FeaturePose = Mapping[str, Number]
FeatureNode = Dict[str, Any]
Metrics = Dict[str, Any]
Toolpath = Dict[str, Any]
Report = Dict[str, Any]
TpmsOptions = Mapping[str, Any]
PocketOptions = Mapping[str, Any]

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

    @classmethod
    def from_ops(cls, ops: Sequence[Op]) -> "Design":
        "Create an L1 design from an existing canonical op list."
        design = cls()
        design.ops.extend(dict(op) for op in ops)
        return design

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

    def clothoid(
        self,
        corner_x: Number,
        corner_y: Number,
        blend: Number,
        x: Optional[Number] = None,
        y: Optional[Number] = None,
        z: Optional[Number] = None,
    ) -> "Design":
        """A clothoid (Euler-spiral) corner blend around construction corner (corner_x, corner_y),
        consuming `blend` mm of tangent length from each leg on the way to (x, y, z).
        """
        self.ops.append({
            "op": "clothoid",
            "corner_x": float(corner_x),
            "corner_y": float(corner_y),
            "blend": float(blend),
            "x": None if x is None else float(x),
            "y": None if y is None else float(y),
            "z": None if z is None else float(z),
        })
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

    def power(self, level: Number) -> "Design":
        """Set the spindle/laser power channel, in the target controller's S-word units
        (RPM for a spindle, PWM counts for a laser).

        Must be finite and >= 0. `0` commands it off, which is distinct from never setting the
        channel. Only the `grbl` flavor renders it; the others refuse a toolpath that carries it
        rather than silently dropping the command.

        NOTE: To render spindle/laser power channels, emit with ``design.gcode(flavor="grbl")`` or
        ``design.gcode(flavor="rs274")``, or emit through the CLI (``dry emit --format grbl``).
        """
        self.ops.append({"op": "power", "level": level})
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

    def pocket(self, options: PocketOptions) -> "Design":
        "Append CNC pocket/profile milling ops generated from an options dict."
        self.ops.extend(pocket_ops(options))
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
        flavor: Optional[str] = None,
        cnc_frame: Optional[Dict[str, Any]] = None,
    ) -> List[str]:
        """Resolve + emit motion g-code (a list of lines).

        `rotary_axes` is the rotary-axes selector — the ab/ac/bc STRING choosing which two rotary
        axes carry the toolframe orientation in 5-axis emit (`five_axis=True`). NOTE: this is NOT the
        machine motion-limits object (see ``balanced_ir`` / ``verify``'s ``kinematics`` for that).
        `kinematics` is a deprecated alias for `rotary_axes`, kept for backward compatibility; when
        provided (not ``None``) it takes precedence.

        `flavor` selects the target dialect: ``marlin`` (default), ``klipper``, ``duet``, ``rs274``
        (aka ``linuxcnc``), ``grbl`` (aka ``laser``), ``krl``, ``siemens`` (aka ``sinumerik``),
        ``heidenhain`` (aka ``tnc``), ``haas``, ``rapid``. An unknown name raises ``ValueError``; it
        used to fall through to Marlin, so asking for a mill quietly emitted FFF g-code.

        `cnc_frame` supplies the machine preamble for the CNC dialects —
        ``{"wcs": 54, "tool": 3, "spindle_rpm": 8000, "coolant": true}``. Without it those flavors
        emit motion lines and no work offset, tool change or spindle start (and no ``TRAORI`` under
        ``five_axis``). ``wcs`` must be in 54..=59 and ``spindle_rpm`` positive, or ``ValueError``.
        """
        rotary = kinematics if kinematics is not None else rotary_axes
        return _native.resolve_gcode(
            json.dumps(self.ops),
            _params(printer),
            relative_e,
            bool(travel_g1_e0),
            bool(five_axis),
            str(rotary),
            flavor,
            None if cnc_frame is None else json.dumps(cnc_frame),
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

    def check_compatibility(
        self,
        capabilities: Mapping[str, Any],
        printer: str = "generic",
    ) -> Mapping[str, Any]:
        """Pre-flight check toolpath against machine capabilities (D2.2).

        The rules live in the engine (``dry_core::check_compatibility``), not here. This method
        previously carried its own copy of the loop, implementing five of the engine's seven rules;
        it omitted ``ARC_OUT_OF_BOUNDS_X`` and ``ARC_OUT_OF_BOUNDS_Y``, so an arc whose swept circle
        leaves the build envelope was reported compatible here and incompatible by the engine. The
        engine bounds an arc by its full circle deliberately — refusing a safe program is
        recoverable, passing an unsafe one is not.

        ``capabilities`` keeps the SDK's shape (``x_range`` as ``[min, max]``, ``max_feedrate``);
        it is adapted to the engine's wire form at this boundary.
        """
        return json.loads(_native.check_compatibility_json(
            json.dumps(self.ops),
            _params(printer),
            json.dumps(_engine_capabilities(capabilities)),
        ))

    # ---- 3D Visualization and Export Helpers ----
    def to_obj(self, include_travel: bool = False, printer: str = "generic") -> str:
        """Export toolpath as 3D Wavefront OBJ mesh string."""
        return toolpath_to_obj(self.ir(printer), include_travel=include_travel)

    def export_obj(self, path: str, include_travel: bool = False, printer: str = "generic") -> None:
        """Save toolpath as 3D Wavefront OBJ mesh file."""
        with open(path, "w", encoding="utf-8") as f:
            f.write(self.to_obj(include_travel=include_travel, printer=printer))

    def to_svg(self, width: int = 800, height: int = 800, printer: str = "generic") -> str:
        """Export toolpath as 2D vector SVG projection string."""
        return toolpath_to_svg(self.ir(printer), width=width, height=height)

    def export_svg(self, path: str, width: int = 800, height: int = 800, printer: str = "generic") -> None:
        """Save toolpath as 2D vector SVG projection file."""
        with open(path, "w", encoding="utf-8") as f:
            f.write(self.to_svg(width=width, height=height, printer=printer))

    def to_html(
        self,
        title: str = "Dry 3D Toolpath Viewer",
        bounds: Optional[Sequence[Sequence[float]]] = None,
        printer: str = "generic",
    ) -> str:
        """Export toolpath as an interactive 3D WebGL HTML viewer string."""
        return toolpath_to_interactive_html(self.ir(printer), title=title, bounds=bounds)

    def export_html(
        self,
        path: str,
        title: str = "Dry 3D Toolpath Viewer",
        bounds: Optional[Sequence[Sequence[float]]] = None,
        printer: str = "generic",
    ) -> None:
        """Save toolpath as an interactive 3D WebGL HTML viewer file."""
        with open(path, "w", encoding="utf-8") as f:
            f.write(self.to_html(title=title, bounds=bounds, printer=printer))


def feature(
    design: Union[Design, Sequence[Op]],
    pose: Optional[FeaturePose] = None,
    name: Optional[str] = None,
) -> FeatureNode:
    """Wrap a coordinate-local L1 design/op list as a feature at a planar pose."""
    ops = design.ops if isinstance(design, Design) else design
    node: FeatureNode = {"kind": "feature", "ops": [dict(op) for op in ops]}
    if pose:
        node["pose"] = dict(pose)
    if name is not None:
        node["name"] = name
    return node


def group(*children: FeatureNode) -> FeatureNode:
    """Compose feature nodes in source order."""
    return {"kind": "group", "children": list(children)}


def repeat(
    child: FeatureNode,
    count: int,
    step: Optional[FeaturePose] = None,
) -> FeatureNode:
    """Repeat a child; instance zero is unchanged and later instances compose ``step``."""
    node: FeatureNode = {"kind": "repeat", "count": int(count), "child": child}
    if step:
        node["step"] = dict(step)
    return node


class FeatureProgram:
    """The bounded P2.3 L0 graph: Feature-at-pose, ordered Group and Repeat."""

    def __init__(self) -> None:
        self.features: List[FeatureNode] = []

    def add(self, *nodes: FeatureNode) -> "FeatureProgram":
        self.features.extend(nodes)
        return self

    def expand(self) -> Design:
        """Expand through the Rust engine and return the canonical L1 ``Design``."""
        ops = json.loads(_native.expand_features(json.dumps({"features": self.features})))
        return Design.from_ops(ops)


def _csv_floats(name: str, text: str, expected: int, shape: str) -> list:
    """Parse a contract CSV into exactly ``expected`` finite floats.

    ``float()`` already refuses ``"abc"`` and an empty field, which is more than JavaScript's
    ``Number`` does, but it still accepts ``"nan"``, ``"inf"`` and ``"1e400"`` (the last as
    infinity). A non-finite contract cannot decide anything — every ordering comparison against NaN
    is false — so the engine treats one as not in force, and a caller passing it would get a report
    that simply omits the rule. Refusing here says why instead.
    """
    parts = text.split(",")
    if len(parts) != expected:
        raise ValueError(f"{name} CSV must be {shape}")
    values = []
    for index, token in enumerate(parts):
        try:
            value = float(token)
        except ValueError:
            raise ValueError(
                f"{name} field {index + 1} is not a number: {token.strip()!r}"
            ) from None
        if not math.isfinite(value):
            raise ValueError(f"{name} values must all be finite, got {token.strip()!r}")
        values.append(value)
    return values


def _finite_all(name: str, values: Any) -> Any:
    """Require every component of an already-structured contract to be finite."""
    for index, value in enumerate(_flatten_numbers(values)):
        if not math.isfinite(value):
            raise ValueError(f"{name} values must all be finite, got {value} at index {index}")
    return values


def _flatten_numbers(values: Any) -> Any:
    """Yield every number in a nested list/tuple, leaving anything else to the binding to reject."""
    if isinstance(values, (list, tuple)):
        for item in values:
            yield from _flatten_numbers(item)
    elif isinstance(values, (int, float)) and not isinstance(values, bool):
        yield float(values)


def _bounds_to_list(bounds: Optional[Bounds]) -> Any:
    """Normalise bounds to the structured `[[x0,x1],[y0,y1],[z0,z1]]` the binding expects.

    A structured list/tuple passes straight through (the binding validates its shape); a legacy
    ``"x0,x1,y0,y1,z0,z1"`` CSV string is parsed here; ``None`` stays ``None``.
    """
    if bounds is None:
        return None
    if not isinstance(bounds, str):
        return _finite_all("bounds", bounds)
    flat = _csv_floats("bounds", bounds, 6, "'x0,x1,y0,y1,z0,z1'")
    return [[flat[0], flat[1]], [flat[2], flat[3]], [flat[4], flat[5]]]


def _range_to_list(rng: Optional[Range]) -> Any:
    """Normalise a `[min, max]` range to the list the binding expects.

    A structured list/tuple passes straight through (the binding validates its shape); a legacy
    ``"min,max"`` CSV string is parsed here; ``None`` stays ``None``.
    """
    if rng is None:
        return None
    if not isinstance(rng, str):
        return _finite_all("range", rng)
    return _csv_floats("range", rng, 2, "'min,max'")


def tpms_ops(options: Optional[TpmsOptions] = None) -> List[Op]:
    """Generate TPMS cellular lattice L1 ops from an options dict."""
    return json.loads(_native.tpms_ops_json(json.dumps(options or {})))


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


def pocket_ops(options: PocketOptions) -> List[Op]:
    """Generate CNC pocket/profile milling L1 ops from an options dict.

    `options` specifies shape (e.g. `{"shape": "rect", "x": 0, "y": 0, "width": 50, "height": 30}`
    or `{"shape": "circle", "cx": 25, "cy": 25, "radius": 20}`), `toolDiameter`, `depth`,
    and optional `stepover`, `depthPerPass`, `safeZ`, `zTop`, `cutFeed`, `plungeFeed`, `mode` ("pocket" | "profile").
    """
    return json.loads(_native.pocket_ops_json(json.dumps(options)))


def pocket_gcode(
    options: PocketOptions,
    printer: str = "generic",
    relative_e: bool = True,
    travel_g1_e0: bool = False,
    five_axis: bool = False,
    rotary_axes: str = "ab",
) -> List[str]:
    """Generate CNC pocket/profile milling g-code from an options dict."""
    return _native.resolve_pocket_gcode(
        json.dumps(options),
        _params(printer),
        relative_e,
        bool(travel_g1_e0),
        bool(five_axis),
        str(rotary_axes),
    )


def drape_ops(options: Dict[str, Any]) -> List[Op]:
    """Generate 5-axis conformal mesh draping L1 ops over a 3D triangle mesh.

    `options` specifies `mesh` (dict with `triangles`), and optional `stepover`, `resolution`,
    `standoffOffset`, `safeZ`, `feedrate`, `plungeFeed`, `pattern` ("raster-x", "raster-y", "zigzag-x").
    """
    return json.loads(_native.drape_ops_json(json.dumps(options)))


def parse_obj_mesh(obj_text: str) -> Dict[str, Any]:
    """Parse Wavefront OBJ format string into a serialized TriangleMesh dict."""
    return json.loads(_native.parse_obj_mesh_json(obj_text))


def slice_step_solid(
    step_content: str,
    z_start: float = 0.0,
    z_end: float = 10.0,
    layer_height: float = 0.2,
    samples_per_slice: int = 36,
    feedrate: float = 1800.0,
) -> List[Op]:
    """Slice an ISO 10303-21 STEP CAD solid directly into L1 ops with analytical surface normals."""
    return json.loads(
        _native.slice_step_solid_json(
            step_content,
            float(z_start),
            float(z_end),
            float(layer_height),
            int(samples_per_slice),
            float(feedrate),
        )
    )


def slice_brep_assembly(
    step_solids: Sequence[str],
    z_start: float = 0.0,
    z_end: float = 10.0,
    layer_height: float = 0.2,
    samples_per_slice: int = 36,
    feedrate: float = 1800.0,
) -> List[Op]:
    """Slice a multi-solid B-Rep assembly directly into L1 ops with 5-axis surface normals."""
    return json.loads(
        _native.slice_brep_assembly_json(
            json.dumps(list(step_solids)),
            float(z_start),
            float(z_end),
            float(layer_height),
            int(samples_per_slice),
            float(feedrate),
        )
    )



def lathe_facing_ops(params: Dict[str, Any]) -> List[Op]:
    """Generate CNC Lathe Facing L1 ops from parameters dict."""
    return json.loads(_native.lathe_facing_ops_json(json.dumps(params)))


def lathe_turning_ops(params: Dict[str, Any]) -> List[Op]:
    """Generate CNC Lathe OD Turning (roughing & finishing) L1 ops from parameters dict."""
    return json.loads(_native.lathe_od_turning_ops_json(json.dumps(params)))


def check_tool_holder_collision(
    toolpath: Union[Toolpath, Dict[str, Any]],
    holder: Dict[str, Any],
    stock_bounds: Sequence[float],
) -> List[Dict[str, Any]]:
    """Check toolpath for tool holder collisions against stock volume bounds."""
    tp_json = toolpath if isinstance(toolpath, str) else json.dumps(toolpath)
    return json.loads(
        _native.check_tool_holder_collision_json(
            tp_json,
            json.dumps(holder),
            json.dumps(list(stock_bounds)),
        )
    )


def reverse_toolpath(toolpath: Union[Toolpath, Dict[str, Any]]) -> List[Op]:
    """Reverse-engineer an L1 Design op list from a resolved L2 Toolpath dict/JSON."""
    tp_json = toolpath if isinstance(toolpath, str) else json.dumps(toolpath)
    return json.loads(_native.reverse_toolpath_json(tp_json))


def slice_brep_assembly_csg(
    step_additives: Sequence[str],
    step_voids: Sequence[str],
    z_start: float = 0.0,
    z_end: float = 10.0,
    layer_height: float = 0.2,
    samples_per_slice: int = 36,
    feedrate: float = 1800.0,
) -> List[Op]:
    """Slice a multi-solid B-Rep assembly with CSG boolean void subtraction in Python."""
    return json.loads(
        _native.slice_brep_assembly_csg_json(
            json.dumps(list(step_additives)),
            json.dumps(list(step_voids)),
            float(z_start),
            float(z_end),
            float(layer_height),
            int(samples_per_slice),
            float(feedrate),
        )
    )


def optimize_constant_mrr(
    toolpath: Union[Toolpath, Dict[str, Any]],
    depth_of_cut: float,
    target_mrr_mm3_min: float,
    min_feedrate: float = 100.0,
    max_feedrate: float = 5000.0,
) -> Toolpath:
    """Optimize toolpath feedrates to maintain Constant Material Removal Rate (MRR)."""
    tp_json = toolpath if isinstance(toolpath, str) else json.dumps(toolpath)
    return json.loads(
        _native.optimize_constant_mrr_json(
            tp_json,
            float(depth_of_cut),
            float(target_mrr_mm3_min),
            float(min_feedrate),
            float(max_feedrate),
        )
    )


def simulate_dexel_stock(
    toolpath: Union[Toolpath, Dict[str, Any]],
    stock_bounds: Sequence[float],
    resolution_mm: float = 1.0,
    tool_radius: float = 3.0,
    is_ballnose: bool = False,
) -> Dict[str, Any]:
    """Simulate 3D Dexel grid stock subtraction against a toolpath."""
    tp_json = toolpath if isinstance(toolpath, str) else json.dumps(toolpath)
    b = list(stock_bounds)
    return json.loads(
        _native.simulate_dexel_stock_json(
            tp_json,
            float(b[0]),
            float(b[1]),
            float(b[2]),
            float(b[3]),
            float(b[4]),
            float(b[5]),
            float(resolution_mm),
            float(tool_radius),
            bool(is_ballnose),
        )
    )


def analyze_machining_physics(
    tool: Dict[str, Any],
    material: str,
    params: Dict[str, Any],
) -> Dict[str, Any]:
    """Run the digital-twin machining physics analysis.

    ``material`` is one of ``Aluminum6061``, ``Steel4140``, ``TitaniumTi6Al4V``, ``Inconel718``,
    ``ThermoplasticPLA``, ``ThermoplasticPEEK``; an unknown name raises ``ValueError``.

    The estimates are analytic closed-form models with textbook coefficients. Nothing in this repo
    validates them against a dynamometer, a thermocouple or a real cut — treat them as indicative,
    not as a process guarantee (``docs/14-known-limitations.md``).
    """
    return json.loads(
        _native.analyze_machining_physics_json(
            json.dumps(tool), str(material), json.dumps(params)
        )
    )


def optimize_five_axis_lookahead(
    toolpath: Union[Toolpath, Dict[str, Any]],
    params: Dict[str, Any],
) -> Dict[str, Any]:
    """Apply the synchronised 5-axis jerk-limited lookahead optimiser to a toolpath."""
    tp_json = toolpath if isinstance(toolpath, str) else json.dumps(toolpath)
    return json.loads(_native.optimize_five_axis_lookahead_json(tp_json, json.dumps(params)))


def segment_to_segment_distance_3d(
    p1: Sequence[float],
    p2: Sequence[float],
    q1: Sequence[float],
    q2: Sequence[float],
) -> float:
    """Calculate minimum Euclidean distance between two 3D line segments."""
    return float(
        _native.segment_to_segment_distance_3d_py(
            [float(p1[0]), float(p1[1]), float(p1[2])],
            [float(p2[0]), float(p2[1]), float(p2[2])],
            [float(q1[0]), float(q1[1]), float(q1[2])],
            [float(q2[0]), float(q2[1]), float(q2[2])],
        )
    )



def _params(printer: str) -> str:
    if printer not in PRINTERS:
        raise KeyError(f"unknown printer {printer!r}; known: {sorted(PRINTERS)}")
    return json.dumps(PRINTERS[printer])


def _axis_range(value: Any) -> Dict[str, float]:
    """Accept the SDK's ``[min, max]`` pair or the engine's ``{"min": .., "max": ..}`` object."""
    if isinstance(value, Mapping):
        return {"min": float(value["min"]), "max": float(value["max"])}
    lo, hi = value
    return {"min": float(lo), "max": float(hi)}


def _engine_capabilities(capabilities: Mapping[str, Any]) -> Dict[str, Any]:
    """Adapt the SDK capability dict to ``dry_core::MachineCapabilities``.

    The SDK shape predates the engine one and stays as the public contract: axis ranges are
    ``[min, max]`` pairs and the feedrate ceiling is ``max_feedrate``. The engine wants
    ``{"min": .., "max": ..}`` and ``max_feedrate_mm_min``. Both already read mm/min, so this is a
    shape translation and not a unit conversion. Absent ranges keep the SDK's historic ``[0, 300]``
    default rather than failing, which is what callers relied on.
    """
    caps: Dict[str, Any] = {
        "name": str(capabilities.get("name", "unnamed")),
        "x_range": _axis_range(capabilities.get("x_range", [0, 300])),
        "y_range": _axis_range(capabilities.get("y_range", [0, 300])),
        "z_range": _axis_range(capabilities.get("z_range", [0, 300])),
    }
    if capabilities.get("max_feedrate") is not None:
        caps["max_feedrate_mm_min"] = float(capabilities["max_feedrate"])
    if capabilities.get("max_spindle_rpm") is not None:
        caps["max_spindle_rpm"] = float(capabilities["max_spindle_rpm"])
    return caps

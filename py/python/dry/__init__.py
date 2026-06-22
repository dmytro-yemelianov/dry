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
import json

from . import _native  # the Rust engine (PyO3)

__all__ = ["Design", "PRINTERS"]

# Device defaults (the lowering's print/travel feedrate + filament diameter). Mirrors the engine's
# ResolveParams; more profiles are added as the device-profile work lands.
PRINTERS = {
    "generic": {"print_speed": 1000.0, "travel_speed": 8000.0, "dia": 1.75},
}


class Design:
    """An L1 design: a chain of authoring ops. Builders return ``self`` for fluent use."""

    def __init__(self):
        self.ops = []

    def geometry(self, width, height):
        "Set the extrusion bead cross-section (mm)."
        self.ops.append({"op": "geometry", "width": width, "height": height})
        return self

    def extruder(self, on):
        "Turn the extruder on/off (off => subsequent moves are travels)."
        self.ops.append({"op": "extruder", "on": bool(on)})
        return self

    def speed(self, print_speed):
        "Set the print feedrate (mm/min)."
        self.ops.append({"op": "speed", "print": print_speed})
        return self

    def point(self, x=None, y=None, z=None):
        "Move to a point; an omitted axis is inherited from the running position."
        self.ops.append({"op": "move", "x": x, "y": y, "z": z})
        return self

    def arc(self, cx, cy, x=None, y=None, z=None, clockwise=False):
        "A circular arc about (cx, cy) to an end point; clockwise => G2, else G3."
        self.ops.append({"op": "arc", "cx": cx, "cy": cy, "x": x, "y": y, "z": z,
                         "clockwise": bool(clockwise)})
        return self

    def spline(self, points):
        "A Catmull-Rom spline from the running position through each (x, y, z) control point."
        self.ops.append({"op": "spline",
                         "points": [[p[0], p[1], p[2]] for p in points]})
        return self

    # ---- process channels (§3): typed, defaulted, propagated by the engine ----
    def temperature(self, nozzle):
        "Set the nozzle temperature channel (°C)."
        self.ops.append({"op": "temperature", "nozzle": nozzle})
        return self

    def fan(self, speed):
        "Set the part-cooling fan channel (0..1)."
        self.ops.append({"op": "fan", "speed": speed})
        return self

    def flow(self, ratio):
        "Set the flow multiplier channel (scales deposited volume; default 1.0)."
        self.ops.append({"op": "flow", "ratio": ratio})
        return self

    def tool(self, index):
        "Set the active tool channel."
        self.ops.append({"op": "tool", "index": int(index)})
        return self

    def orient(self, i, j, k):
        "Set the toolframe orientation: the tool-direction vector (i, j, k). Identity is +Z."
        self.ops.append({"op": "orient", "i": i, "j": j, "k": k})
        return self

    def dwell(self, seconds):
        "Pause in place for `seconds` (emits a G4 dwell)."
        self.ops.append({"op": "dwell", "seconds": seconds})
        return self

    # ---- engine calls ----
    def gcode(self, printer="generic", relative_e=True, travel_g1_e0=False, five_axis=False, kinematics="ab"):
        "Resolve + emit motion g-code (a list of lines)."
        return _native.resolve_gcode(
            json.dumps(self.ops),
            _params(printer),
            relative_e,
            bool(travel_g1_e0),
            bool(five_axis),
            str(kinematics)
        )

    def simulate(self, printer="generic"):
        "Resolve + simulate; returns a metrics dict (time, distances, material, peak flow)."
        return json.loads(_native.resolve_metrics(json.dumps(self.ops), _params(printer)))

    def ir(self, printer="generic"):
        "Resolve to the L2 Dry IR; returns a dict ({version, segments})."
        return json.loads(_native.resolve_ir(json.dumps(self.ops), _params(printer)))

    def optimized_ir(self, printer="generic"):
        "Resolve + optimize; returns a dict ({version, segments})."
        return json.loads(_native.resolve_optimized_ir(json.dumps(self.ops), _params(printer)))

    def binary(self, printer="generic"):
        "Resolve + encode to binary DRY1 format; returns a bytes object."
        return bytes(_native.resolve_binary(json.dumps(self.ops), _params(printer)))

    def verify(self, printer="generic", max_flow=None, min_temp=None, bounds=None, monotonic_z=False, speed_range=None):
        "Resolve + verify; returns a report dict with findings."
        return json.loads(_native.resolve_verify(
            json.dumps(self.ops),
            _params(printer),
            max_flow,
            min_temp,
            bounds,
            bool(monotonic_z),
            speed_range
        ))


def _params(printer):
    if printer not in PRINTERS:
        raise KeyError(f"unknown printer {printer!r}; known: {sorted(PRINTERS)}")
    return json.dumps(PRINTERS[printer])

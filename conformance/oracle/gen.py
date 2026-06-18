"""Conformance ORACLE (dev/CI only — never shipped, never copied into Dry).

Runs FullControl on a few tiny, independently-authored designs and writes, per design:
  conformance/simulate/<name>.json  — Dry-IR segments + the simulation metrics FullControl computes
  conformance/gcode/<name>.json     — Dry-IR segments + the motion g-code FullControl emits (Marlin,
                                      relative-E) — the byte-identical target

Dry must reproduce both *outputs* from the same segments, written from scratch (clean-room — matching
functional output, not copying code). See ../../docs/CLEANROOM.md. Run with the FullControl venv:
  /Users/dmytro/Documents/github/fullcontrol/.venv/bin/python conformance/oracle/gen.py
"""
import json
import os

import fullcontrol as fc
from fullcontrol.ir import resolve
from fullcontrol.ir.toolpath import Segment
from fullcontrol.ir.kernel import emit_gcode_moves_rust
from fullcontrol.simulate.run import simulate_from_ir

HERE = os.path.dirname(__file__)
CONTROLS = fc.GcodeControls(printer_name='generic', initialization_data={'nozzle_temp': 210})
RELATIVE_E, TRAVEL_G1_E0 = True, False


def _a(v):
    return None if v is None else float(v)


def seg_to_dry(s: Segment) -> dict:
    return {'start': [_a(s.start[0]), _a(s.start[1]), _a(s.start[2])],
            'end': [_a(s.end[0]), _a(s.end[1]), _a(s.end[2])], 'travel': bool(s.travel),
            'speed': float(s.speed), 'length': float(s.length), 'volume': float(s.deposited_volume),
            'filament': float(s.filament_length), 'width': None if s.width is None else float(s.width),
            'height': None if s.height is None else float(s.height), 'kind': s.kind,
            'centre': None if s.centre is None else [float(s.centre[0]), float(s.centre[1])],
            'clockwise': bool(s.clockwise)}


def sim_metrics(tp) -> dict:
    r = simulate_from_ir(tp)
    fields = ['total_time_s', 'print_time_s', 'travel_time_s', 'extruding_distance',
              'travel_distance', 'extruded_volume', 'filament_length', 'segment_count', 'max_flow_rate']
    return {f: getattr(r, f) for f in fields if hasattr(r, f)}


def write(name: str, steps: list):
    tp = resolve(steps, CONTROLS, include_procedures=False)
    segs = [seg_to_dry(e) for e in tp.events if isinstance(e, Segment)]
    ir = {'version': 0, 'segments': segs}
    with open(os.path.join(HERE, '..', 'simulate', f'{name}.json'), 'w') as f:
        json.dump({'design': name, 'oracle': 'fullcontrol', 'ir': ir,
                   'expected': sim_metrics(tp)}, f, indent=2)
    gcode = emit_gcode_moves_rust(tp, relative_e=RELATIVE_E, travel_g1_e0=TRAVEL_G1_E0)
    with open(os.path.join(HERE, '..', 'gcode', f'{name}.json'), 'w') as f:
        json.dump({'design': name, 'oracle': 'fullcontrol', 'ir': ir,
                   'params': {'relative_e': RELATIVE_E, 'travel_g1_e0': TRAVEL_G1_E0, 'flavor': 'marlin'},
                   'expected': list(gcode)}, f, indent=2)
    print(f'wrote {name}: {len(segs)} segments, {len(gcode)} g-code lines')


G = lambda: fc.ExtrusionGeometry(width=0.6, height=0.2)  # noqa: E731
ON = lambda: fc.Extruder(on=True)                         # noqa: E731
OFF = lambda: fc.Extruder(on=False)                       # noqa: E731

write('square', [G(), ON(), fc.Point(x=0, y=0, z=0.2), fc.Point(x=10, y=0, z=0.2),
                 fc.Point(x=10, y=10, z=0.2), fc.Point(x=0, y=10, z=0.2), fc.Point(x=0, y=0, z=0.2)])
write('travel_mix', [G(), ON(), fc.Point(x=0, y=0, z=0.2), fc.Point(x=8, y=0, z=0.2),
                     OFF(), fc.Point(x=20, y=0, z=0.2), ON(), fc.Point(x=28, y=0, z=0.2)])
write('stack3', [G(), ON()] + [fc.Point(x=x, y=y, z=z) for z in (0.2, 0.4, 0.6)
                               for x, y in ((6, 0), (6, 6), (0, 6), (0, 0))])
write('fast_thin', [fc.ExtrusionGeometry(width=0.4, height=0.1), ON(), fc.Printer(print_speed=6000),
                    fc.Point(x=0, y=0, z=0.1), fc.Point(x=40, y=0, z=0.1)])
write('ramp_speed', [G(), ON(), fc.Point(x=0, y=0, z=0.3), fc.Printer(print_speed=2400),
                     fc.Point(x=15, y=0, z=0.3), fc.Printer(print_speed=1200), fc.Point(x=15, y=9, z=0.3)])

# arcs: native G2/G3 (the thing Dry keeps as an arc; FullControl emits one arc move per fc.Arc)
write('arc_ccw', [G(), ON(), fc.Point(x=10, y=0, z=0.2),
                  fc.Arc(centre=fc.Point(x=0, y=0), end=fc.Point(x=0, y=10), direction='anticlockwise'),
                  fc.Point(x=0, y=20, z=0.2)])
write('arc_cw', [G(), ON(), fc.Point(x=0, y=10, z=0.2),
                 fc.Arc(centre=fc.Point(x=0, y=0), end=fc.Point(x=10, y=0), direction='clockwise')])
write('arcs_mix', [G(), ON(), fc.Point(x=20, y=5, z=0.4), fc.Printer(print_speed=1800),
                   fc.Arc(centre=fc.Point(x=10, y=5), end=fc.Point(x=0, y=5), direction='clockwise'),
                   fc.Point(x=0, y=15, z=0.4),
                   fc.Arc(centre=fc.Point(x=10, y=15), end=fc.Point(x=20, y=15), direction='clockwise')])

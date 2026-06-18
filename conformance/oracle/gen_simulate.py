"""Conformance ORACLE (dev/CI only — never shipped, never copied into Dry).

Runs FullControl on a few tiny, independently-authored designs and writes, per design, a fixture:
the resolved toolpath as Dry-IR-shaped segments (functional motion data) + the simulation metrics
FullControl computes for them. Dry's own `simulate` must reproduce those metrics from the same
segments (clean-room: matching functional output, not copying code). See ../../docs/CLEANROOM.md.

Run with the FullControl venv:
  /Users/dmytro/Documents/github/fullcontrol/.venv/bin/python conformance/oracle/gen_simulate.py
"""
import json
import os

import fullcontrol as fc
from fullcontrol.ir import resolve
from fullcontrol.ir.toolpath import Segment
from fullcontrol.simulate.run import simulate_from_ir

OUT = os.path.join(os.path.dirname(__file__), '..', 'simulate')
CONTROLS = fc.GcodeControls(printer_name='generic', initialization_data={'nozzle_temp': 210})


def _axis(v):
    return None if v is None else float(v)


def seg_to_dry(s: Segment) -> dict:
    return {
        'start': [_axis(s.start[0]), _axis(s.start[1]), _axis(s.start[2])],
        'end': [_axis(s.end[0]), _axis(s.end[1]), _axis(s.end[2])],
        'travel': bool(s.travel),
        'speed': float(s.speed),
        'length': float(s.length),
        'volume': float(s.deposited_volume),
        'filament': float(s.filament_length),
        'width': None if s.width is None else float(s.width),
        'height': None if s.height is None else float(s.height),
        'kind': s.kind,
    }


def metrics(tp) -> dict:
    r = simulate_from_ir(tp)
    fields = ['total_time_s', 'print_time_s', 'travel_time_s', 'extruding_distance',
              'travel_distance', 'extruded_volume', 'filament_length', 'segment_count',
              'max_flow_rate']
    return {f: getattr(r, f) for f in fields if hasattr(r, f)}


def fixture(name: str, steps: list):
    tp = resolve(steps, CONTROLS, include_procedures=False)
    segs = [seg_to_dry(e) for e in tp.events if isinstance(e, Segment)]
    doc = {'design': name, 'oracle': 'fullcontrol', 'ir': {'version': 0, 'segments': segs},
           'expected': metrics(tp)}
    path = os.path.join(OUT, f'{name}.json')
    with open(path, 'w') as f:
        json.dump(doc, f, indent=2)
    print(f'wrote {name}: {len(segs)} segments, time {doc["expected"].get("total_time_s"):.3f}s')


G = lambda: fc.ExtrusionGeometry(width=0.6, height=0.2)  # noqa: E731
ON = lambda: fc.Extruder(on=True)                         # noqa: E731
OFF = lambda: fc.Extruder(on=False)                       # noqa: E731

# independently-authored tiny designs (not from FC's gallery)
fixture('square', [G(), ON(), fc.Point(x=0, y=0, z=0.2), fc.Point(x=10, y=0, z=0.2),
                   fc.Point(x=10, y=10, z=0.2), fc.Point(x=0, y=10, z=0.2), fc.Point(x=0, y=0, z=0.2)])

fixture('travel_mix', [G(), ON(), fc.Point(x=0, y=0, z=0.2), fc.Point(x=8, y=0, z=0.2),
                       OFF(), fc.Point(x=20, y=0, z=0.2), ON(), fc.Point(x=28, y=0, z=0.2)])

fixture('stack3', [G(), ON()] + [fc.Point(x=x, y=y, z=z)
                                 for z in (0.2, 0.4, 0.6)
                                 for x, y in ((6, 0), (6, 6), (0, 6), (0, 0))])

fixture('fast_thin', [fc.ExtrusionGeometry(width=0.4, height=0.1), ON(),
                      fc.Printer(print_speed=6000),
                      fc.Point(x=0, y=0, z=0.1), fc.Point(x=40, y=0, z=0.1)])

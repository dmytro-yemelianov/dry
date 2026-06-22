import ast
import json
import math
import os
import re
import shutil
import sys
import glob

# Ensure we can import fullcontrol by pointing to sibling repo
sys.path.insert(0, '/Users/dmytro/Documents/github/fullcontrol')

import fullcontrol as fc
from fullcontrol.gcode.state import State
from fullcontrol.gcode.dialect import gcode_from_ir
from fullcontrol.ir import resolve
from fullcontrol.ir.toolpath import Segment
from fullcontrol.ir.kernel import emit_gcode_moves_rust, emit_gcode_rust
from fullcontrol.simulate.run import simulate_from_ir
from fullcontrol.gcode.import_printer import resolve_initialization_data, load_json

HERE = os.path.dirname(os.path.abspath(__file__))
DRY_ROOT = os.path.join(HERE, '..')
FULLCONTROL_ROOT = '/Users/dmytro/Documents/github/fullcontrol'

# Create target directories
for name in ['golden', 'gcode', 'gallery', 'profiles', 'roundtrip', 'simulate']:
    os.makedirs(os.path.join(DRY_ROOT, 'conformance', name), exist_ok=True)

# 1. Export Golden Output
print("1. Exporting golden outputs...")
golden_src = os.path.join(FULLCONTROL_ROOT, 'tests', 'unit', 'golden')
golden_dst = os.path.join(DRY_ROOT, 'conformance', 'golden')
for txt_path in glob.glob(os.path.join(golden_src, '*.txt')):
    shutil.copy(txt_path, golden_dst)
print(f"Copied {len(glob.glob(os.path.join(golden_dst, '*.txt')))} golden files.")


# Helper to convert Point coordinates
def _a(v):
    return None if v is None else float(v)

# Convert Segment to dict
def seg_to_dry(s: Segment) -> dict:
    return {
        'start': [_a(s.start[0]), _a(s.start[1]), _a(s.start[2])],
        'end': [_a(s.end[0]), _a(s.end[1]), _a(s.end[2])],
        'travel': bool(s.travel),
        'speed': float(s.speed),
        'length': float(s.length),
        'volume': float(s.deposited_volume),
        'filament': float(s.filament_length),
        'width': None if s.width is None else float(s.width),
        'height': None if s.height is None else float(s.height),
        'kind': s.kind,
        'centre': None if s.centre is None else [float(s.centre[0]), float(s.centre[1])],
        'clockwise': bool(s.clockwise),
        'temperature': None if getattr(s, 'temperature', None) is None else float(s.temperature),
        'fan': None if getattr(s, 'fan', None) is None else float(s.fan),
        'flow': None if getattr(s, 'flow', None) is None else float(s.flow),
        'tool': None if getattr(s, 'tool', None) is None else int(s.tool),
        'dwell_s': None if getattr(s, 'dwell_s', None) is None else float(s.dwell_s),
        'orientation': None if getattr(s, 'orientation', None) is None else [float(x) for x in s.orientation]
    }

def sim_metrics(tp) -> dict:
    r = simulate_from_ir(tp)
    fields = ['total_time_s', 'print_time_s', 'travel_time_s', 'extruding_distance',
              'travel_distance', 'extruded_volume', 'filament_length', 'segment_count', 'max_flow_rate']
    return {f: getattr(r, f) for f in fields if hasattr(r, f)}

# Step to Op mapping
def step_to_op(s):
    t = type(s).__name__
    if t == 'ExtrusionGeometry':
        return {'op': 'geometry', 'width': _a(s.width), 'height': _a(s.height)}
    elif t == 'Extruder':
        return {'op': 'extruder', 'on': bool(s.on)}
    elif t == 'Printer':
        return {'op': 'speed', 'print': _a(s.print_speed)}
    elif t == 'Point':
        return {'op': 'move', 'x': _a(s.x), 'y': _a(s.y), 'z': _a(s.z)}
    elif t == 'Arc':
        return {'op': 'arc', 'cx': _a(s.centre.x), 'cy': _a(s.centre.y),
                'x': _a(s.end.x), 'y': _a(s.end.y), 'z': _a(s.end.z),
                'clockwise': s.direction == 'clockwise'}
    elif t == 'Fan':
        return {'op': 'fan', 'speed': None if s.speed_percent is None else float(s.speed_percent / 100.0)}
    elif t == 'Hotend':
        return {'op': 'temperature', 'value': _a(s.temp)}
    elif t == 'Buildplate':
        return {'op': 'bed_temperature', 'value': _a(s.temp)}
    elif t == 'Retraction':
        return {'op': 'retract', 'distance': _a(s.distance), 'speed': _a(s.speed)}
    elif t == 'Unretraction':
        return {'op': 'unretract', 'distance': _a(s.distance), 'speed': _a(s.speed)}
    elif t == 'Acceleration':
        return {'op': 'acceleration', 'printing': _a(s.printing), 'travel': _a(s.travel), 'retract': _a(s.retract)}
    elif t == 'Jerk':
        return {'op': 'jerk', 'x': _a(s.x), 'y': _a(s.y), 'z': _a(s.z), 'e': _a(s.e)}
    elif t == 'PressureAdvance':
        return {'op': 'pressure_advance', 'value': _a(s.value)}
    elif t == 'ManualGcode':
        return {'op': 'manual_gcode', 'text': str(s.text)}
    elif t == 'GcodeComment':
        return {'op': 'comment', 'text': str(s.text)}
    raise ValueError(f'no L1 mapping for step {t}')

RESOLVE_PARAMS = {'print_speed': 1000.0, 'travel_speed': 8000.0, 'dia': 1.75}

# 2. Export base gcode and simulate conformance files (from gen.py)
print("2. Generating base gcode and simulate conformance files...")
# Define identical designs to gen.py
G = lambda: fc.ExtrusionGeometry(width=0.6, height=0.2)
ON = lambda: fc.Extruder(on=True)
OFF = lambda: fc.Extruder(on=False)

BASE_DESIGNS = {
    'square': [G(), ON(), fc.Point(x=0, y=0, z=0.2), fc.Point(x=10, y=0, z=0.2),
               fc.Point(x=10, y=10, z=0.2), fc.Point(x=0, y=10, z=0.2), fc.Point(x=0, y=0, z=0.2)],
    'travel_mix': [G(), ON(), fc.Point(x=0, y=0, z=0.2), fc.Point(x=8, y=0, z=0.2),
                   OFF(), fc.Point(x=20, y=0, z=0.2), ON(), fc.Point(x=28, y=0, z=0.2)],
    'stack3': [G(), ON()] + [fc.Point(x=x, y=y, z=z) for z in (0.2, 0.4, 0.6)
                             for x, y in ((6, 0), (6, 6), (0, 6), (0, 0))],
    'fast_thin': [fc.ExtrusionGeometry(width=0.4, height=0.1), ON(), fc.Printer(print_speed=6000),
                  fc.Point(x=0, y=0, z=0.1), fc.Point(x=40, y=0, z=0.1)],
    'ramp_speed': [G(), ON(), fc.Point(x=0, y=0, z=0.3), fc.Printer(print_speed=2400),
                   fc.Point(x=15, y=0, z=0.3), fc.Printer(print_speed=1200), fc.Point(x=15, y=9, z=0.3)],
    'arc_ccw': [G(), ON(), fc.Point(x=10, y=0, z=0.2),
                fc.Arc(centre=fc.Point(x=0, y=0), end=fc.Point(x=0, y=10), direction='anticlockwise'),
                fc.Point(x=0, y=20, z=0.2)],
    'arc_cw': [G(), ON(), fc.Point(x=0, y=10, z=0.2),
               fc.Arc(centre=fc.Point(x=0, y=0), end=fc.Point(x=10, y=0), direction='clockwise')],
    'arcs_mix': [G(), ON(), fc.Point(x=20, y=5, z=0.4), fc.Printer(print_speed=1800),
                 fc.Arc(centre=fc.Point(x=10, y=5), end=fc.Point(x=0, y=5), direction='clockwise'),
                 fc.Point(x=0, y=15, z=0.4),
                 fc.Arc(centre=fc.Point(x=10, y=15), end=fc.Point(x=20, y=15), direction='clockwise')]
}

# Add spiral vase
def spiral(radius=15.0, height=1.5, layer_h=0.3, per_layer=24, centre=(50.0, 50.0)):
    steps = [G(), ON()]
    n = int(round(height / layer_h * per_layer))
    for i in range(n + 1):
        frac = i / per_layer
        a = frac * 2 * math.pi
        steps.append(fc.Point(x=centre[0] + radius * math.cos(a),
                              y=centre[1] + radius * math.sin(a),
                              z=0.2 + frac * layer_h))
    return steps

BASE_DESIGNS['spiral_vase'] = spiral()

for name, steps in BASE_DESIGNS.items():
    tp = resolve(steps, fc.GcodeControls(printer_name='generic', initialization_data={'nozzle_temp': 210}), include_procedures=False)
    segs = [seg_to_dry(e) for e in tp.events if isinstance(e, Segment)]
    ir = {'version': 0, 'segments': segs}
    l1 = {'ops': [step_to_op(s) for s in steps]}
    
    # Write simulate fixture
    with open(os.path.join(DRY_ROOT, 'conformance', 'simulate', f'{name}.json'), 'w') as f:
        json.dump({'design': name, 'oracle': 'fullcontrol', 'ir': ir,
                   'expected': sim_metrics(tp)}, f, indent=2)
    
    # Write gcode fixture
    gcode = emit_gcode_moves_rust(tp, relative_e=True, travel_g1_e0=False)
    with open(os.path.join(DRY_ROOT, 'conformance', 'gcode', f'{name}.json'), 'w') as f:
        json.dump({'design': name, 'oracle': 'fullcontrol', 'l1': l1,
                   'resolve_params': RESOLVE_PARAMS, 'ir': ir,
                   'params': {'relative_e': True, 'travel_g1_e0': False, 'flavor': 'marlin'},
                   'expected': list(gcode)}, f, indent=2)


# 3. Export Gallery Designs (the 27 examples)
print("3. Exporting Gallery Designs...")
from tests.unit.test_examples import _SMALL, _BUILD

for name in sorted(_SMALL):
    func = _SMALL[name]
    steps = func()
    
    # Resolve
    controls = fc.GcodeControls(printer_name='generic', initialization_data=_BUILD)
    controls.initialize()
    dstate = State(steps, controls)
    tp = resolve(steps, controls, state=dstate)
    
    # Map to Dry L1 Ops
    ops = []
    for s in steps:
        try:
            ops.append(step_to_op(s))
        except ValueError as e:
            # print warning and skip or handle
            pass
            
    segs = [seg_to_dry(e) for e in tp.events if isinstance(e, Segment)]
    ir = {'version': 0, 'segments': segs}
    metrics = sim_metrics(tp)
    
    # Generate G-code (Marlin default)
    gcode = emit_gcode_moves_rust(tp, relative_e=True, travel_g1_e0=False)
    
    # Write gallery design fixture
    with open(os.path.join(DRY_ROOT, 'conformance', 'gallery', f'{name}.json'), 'w') as f:
        json.dump({
            'design': name,
            'oracle': 'fullcontrol',
            'l1': {'ops': ops},
            'resolve_params': RESOLVE_PARAMS,
            'ir': ir,
            'params': {'relative_e': True, 'travel_g1_e0': False, 'flavor': 'marlin'},
            'expected_metrics': metrics,
            'expected_gcode': list(gcode)
        }, f, indent=2)
print(f"Exported {len(_SMALL)} gallery designs.")


# 4. Export Device Profiles
print("4. Exporting Device Profiles...")
def export_profile(printer_name, prefix, filename):
    try:
        data = resolve_initialization_data(printer_name, {})
        # Map variables
        vol_x = float(data.get('build_volume_x', 200.0))
        vol_y = float(data.get('build_volume_y', 200.0))
        vol_z = float(data.get('build_volume_z', 200.0))
        
        flavor_str = data.get('gcode_flavor', 'marlin')
        
        profile_json = {
            "version": 1,
            "name": f"{prefix}-{data.get('name', printer_name)}",
            "firmware": {
                "flavor": flavor_str
            },
            "machine": {
                "build_volume": [
                    [0.0, vol_x],
                    [0.0, vol_y],
                    [0.0, vol_z]
                ],
                "feedrate_range": [
                    float(data.get('print_speed', 40.0 * 60)),
                    float(data.get('travel_speed', 120.0 * 60))
                ]
            },
            "material": {
                "filament_diameter": float(data.get('dia_feed', 1.75)),
                "min_nozzle_temperature_c": float(data.get('min_temp', 190.0))
            },
            "process": {
                "line_width": float(data.get('extrusion_width', 0.6)),
                "layer_height": float(data.get('extrusion_height', 0.2)),
                "monotonic_z": False
            },
            "start_gcode": data.get('start_gcode'),
            "end_gcode": data.get('end_gcode')
        }
        
        # Write to profiles/
        out_name = f"{prefix}_{filename.replace('.py', '')}.json"
        with open(os.path.join(DRY_ROOT, 'conformance', 'profiles', out_name), 'w') as f:
            json.dump(profile_json, f, indent=2)
        return True
    except Exception as e:
        # print(f"Failed to export profile {printer_name}: {e}")
        return False

profile_count = 0

# Cura profiles
cura_lib = load_json('cura', 'library.json')
for display, filename in cura_lib.items():
    if export_profile(f"Cura/{display}", "cura", filename):
        profile_count += 1

# Community profiles
comm_lib = load_json('community_minimal', 'library.json')
for display, filename in comm_lib.items():
    if export_profile(f"Community/{display}", "community", filename):
        profile_count += 1

# Singletool profiles
singletool_dir = os.path.join(FULLCONTROL_ROOT, 'fullcontrol', 'devices', 'community', 'singletool')
for fn in os.listdir(singletool_dir):
    if fn.endswith('.py') and not fn.startswith('_') and fn != 'base_settings.py':
        name = fn[:-3]
        if export_profile(name, "singletool", fn):
            profile_count += 1

print(f"Exported {profile_count} device profiles.")


# 5. Export Round-trip & Simulate fixtures
print("5. Exporting roundtrip fixtures...")
from tests.unit.test_gcode_roundtrip import DESIGNS as RT_DESIGNS, E_VARIANTS as RT_VARIANTS, _emit_motion_only, _controls as rt_controls

rt_count = 0
for dname in RT_DESIGNS:
    for vname, init in RT_VARIANTS:
        steps = RT_DESIGNS[dname]()
        controls = rt_controls(**init)
        gc = _emit_motion_only(steps, controls)
        
        # Output
        out_name = f"{dname}_{vname}.json"
        with open(os.path.join(DRY_ROOT, 'conformance', 'roundtrip', out_name), 'w') as f:
            json.dump({
                "design": dname,
                "variant": vname,
                "init_params": init,
                "gcode": gc
            }, f, indent=2)
        rt_count += 1

print(f"Exported {rt_count} roundtrip fixtures.")
print("Conformance export completed successfully.")

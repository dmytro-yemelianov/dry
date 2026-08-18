# Migrating from FullControl to Dry

This guide covers migrating existing FullControl Python scripts and CAD/CAM workflows to **Dry**.

---

## 1. Overview & Key Differences

| Dimension | FullControl (Legacy Python) | Dry (Modern Rust Core) |
|---|---|---|
| **Architecture** | Dynamic Python object walk with mutable global state | Strict multi-level intermediate representation ($L0 \to L1 \to L2 \to L3$) |
| **Performance** | Slicing / resolving large designs takes seconds–minutes | Sub-millisecond to tens of milliseconds via optimized Rust core |
| **Verification & Simulation** | Post-hoc validation checks with loose typing | Integrated simulation metrics, typed invariants, formal verification models (Lean 4) |
| **Language Support** | Python only | Native Python, TypeScript (Node/Browser via Wasm), and Rust APIs |
| **Advanced Kinematics** | Planar 3-axis FFF | True 5-axis drape/tilt kinematics, Euler-spiral clothoids, TPMS infill, CNC pocketing |

---

## 2. Drop-in Migration Shim (`dry.compat.fullcontrol`)

For legacy scripts that construct step lists (e.g. from Google Colab or fullcontrol.xyz notebooks), you can use the compatibility shim with a 1-line import change:

```python
# Before:
# import fullcontrol as fc

# After:
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
```

---

## 3. Idiomatic Native Dry API

For new designs and refactored code, we recommend using Dry's fluent builder API:

### Python ([py/](py/))
```python
import dry

design = (
    dry.Design()
    .geometry(width=0.6, height=0.2)
    .extruder(True)
    .speed(1200)
    .point(0, 0, 0.2)
    .point(10, 0, 0.2)
    .clothoid(corner_x=10, corner_y=10, blend=3, x=0, y=10, z=0.2)
    .point(0, 0, 0.2)
)

# Emit G-code for Marlin/Klipper/Duet
gcode = design.gcode(printer="generic")

# Inspect simulation metrics
metrics = design.simulate()
print(f"Total print time: {metrics['total_time_s']}s, Volume: {metrics['extruded_volume']}mm³")
```

### TypeScript ([sdk/ts/](sdk/ts/))
```typescript
import { Design, pocket } from '@dry/sdk';

const design = new Design()
  .geometry(0.6, 0.2)
  .extruder(true)
  .point(0, 0, 0.2)
  .point(10, 0, 0.2);

const gcode = design.gcode();
const metrics = design.simulate();
```

---

## 4. Advanced Features in Dry

- **Clothoid Corner Blends**: `design.clothoid(corner_x, corner_y, blend, x, y, z)` generates Euler-spiral continuous curvature transitions.
- **Parametric Infill & Pockets**: Native `dry.tpms_gcode()` for minimal surface infill and `dry.pocket_gcode()` for CNC pocket clearing.
- **5-Axis True Orientations**: `design.orient(i, j, k)` maps tool surface normals directly onto 5-axis machine kinematics (`ab`, `ac`, `bc`).

# Dry FreeCAD Path / CAM Integration

FreeCAD integration connecting FreeCAD Part/B-Rep objects to Dry's deterministic CAM toolpath engine.

---

## 1. Features

- **Direct B-Rep CSG Slicing**: Slices FreeCAD 3D solid geometry into continuous L1 toolpaths with analytical 5-axis surface normals.
- **Path Command Conversion**: Converts Dry operations into native FreeCAD `Path.Command` (`G1`, `G2`, `G3`, `G4`) structures.
- **Parametric Pocketing**: Generates multi-depth pocketing directly on FreeCAD sketches and faces.

---

## 2. Usage in FreeCAD

In the FreeCAD Python Console or Macro Editor:
```python
import FreeCAD
import dry_freecad_cam

# Generate stepped pocket
ops = dry_freecad_cam.generate_freecad_pocket(
    width=50.0, height=40.0, depth=10.0,
    tool_diameter=6.0, stepover=3.0, stepdown=2.0, feedrate=1200.0
)

# Convert to FreeCAD Path commands
cmds = dry_freecad_cam.convert_dry_ops_to_path_commands(ops)
print(f"Generated {len(cmds)} FreeCAD Path commands.")
```

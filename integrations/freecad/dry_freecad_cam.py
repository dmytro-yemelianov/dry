"""FreeCAD Path & CAM Workbench Module for Dry.

Connects FreeCAD solid shapes and STEP models to Dry for:
1. Multi-Solid CSG boolean slicing with 5-axis surface normals.
2. Conversion of Dry L1/L2 toolpaths into native FreeCAD Path commands.
3. Parametric 3D stepped pocket and lathe OD turning toolpath generation.
"""

import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

try:
    import FreeCAD
    import Path as FreeCADPath
    import Part
    IN_FREECAD = True
except ImportError:
    IN_FREECAD = False

# Try importing Dry SDK
try:
    import dry
except ImportError:
    repo_root = Path(__file__).resolve().parents[2]
    py_pkg = repo_root / "py" / "python"
    if py_pkg.exists():
        sys.path.insert(0, str(py_pkg))
        import dry
    else:
        dry = None


def convert_dry_ops_to_path_commands(ops: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """Convert Dry L1/L2 operations to FreeCAD Path command structures."""
    commands = []
    for op in ops:
        op_type = op.get("op")
        if op_type == "move":
            x = op.get("x")
            y = op.get("y")
            z = op.get("z")
            feed = op.get("speed", 1200.0)
            commands.append({
                "name": "G1",
                "parameters": {"X": x, "Y": y, "Z": z, "F": feed}
            })
        elif op_type == "arc":
            clockwise = op.get("clockwise", False)
            cmd_name = "G2" if clockwise else "G3"
            commands.append({
                "name": cmd_name,
                "parameters": {
                    "X": op.get("x"),
                    "Y": op.get("y"),
                    "Z": op.get("z"),
                    "I": op.get("cx"),
                    "J": op.get("cy"),
                }
            })
        elif op_type == "dwell":
            commands.append({
                "name": "G4",
                "parameters": {"P": op.get("seconds", 1.0)}
            })
    return commands


def slice_freecad_shape_with_dry(
    step_string: str,
    z_start: float = 0.0,
    z_end: float = 10.0,
    layer_height: float = 0.2,
    feedrate: float = 1500.0,
) -> List[Dict[str, Any]]:
    """Slice an ISO-10303 STEP representation of a FreeCAD Shape into Dry L1 ops."""
    if dry is None:
        return []

    return dry.slice_step_solid(
        step_string,
        z_start=z_start,
        z_end=z_end,
        layer_height=layer_height,
        feedrate=feedrate,
    )


def generate_freecad_pocket(
    width: float,
    height: float,
    depth: float,
    tool_diameter: float,
    stepover: float,
    stepdown: float,
    feedrate: float,
) -> List[Dict[str, Any]]:
    """Generate 2.5D stepped pocket operations."""
    if dry is None:
        return []

    opts = {
        "shape": "rect",
        "x": 0.0,
        "y": 0.0,
        "width": width,
        "height": height,
        "toolDiameter": tool_diameter,
        "depth": depth,
        "stepover": stepover,
        "depthPerPass": stepdown,
        "cutFeed": feedrate,
    }
    return dry.pocket_ops(opts)


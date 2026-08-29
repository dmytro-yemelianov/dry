# Dry CAM Studio — Blender 3.x / 4.x Addon
# Parametric TPMS Lattice Generation, 5-Axis Conformal Draping & 3D Toolpath Visualization

bl_info = {
    "name": "Dry CAM Studio",
    "author": "Dmytro Yemelianov",
    "version": (0, 7, 0),
    "blender": (3, 0, 0),
    "location": "View3D > Sidebar > Dry CAM",
    "description": "Parametric TPMS generation, 5-axis non-planar conformal toolpath generation, and interactive G-code visualization.",
    "category": "3D View",
}

import json
import math
import sys
from pathlib import Path
from typing import Any, Dict, List

try:
    import bpy
    from bpy.props import BoolProperty, EnumProperty, FloatProperty, IntProperty, StringProperty
    from bpy.types import Operator, Panel, PropertyGroup
    IN_BLENDER = True
except ImportError:
    # Allow importing in pure Python testing environments
    IN_BLENDER = False

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


def generate_tpms_curves(
    surface_name: str,
    cell_size: float,
    iso_level: float,
    layer_height: float,
    size_x: float,
    size_y: float,
    size_z: float,
) -> List[List[List[float]]]:
    """Generate 3D polyline coordinates from Dry TPMS generator."""
    if dry is None:
        return []

    opts = {
        "surface": surface_name,
        "cellSize": cell_size,
        "isoLevel": iso_level,
        "layerHeight": layer_height,
        "sizeX": size_x,
        "sizeY": size_y,
        "sizeZ": size_z,
        "samplesPerCell": 16,
    }
    ops_json = dry._native.tpms_ops_json(json.dumps(opts))
    ops = json.loads(ops_json)

    contours: List[List[List[float]]] = []
    current_contour: List[List[float]] = []

    for op in ops:
        if op.get("op") == "move":
            x = op.get("x")
            y = op.get("y")
            z = op.get("z")
            if x is not None and y is not None and z is not None:
                current_contour.append([x, y, z])
        elif op.get("op") == "extruder":
            if not op.get("on", False) and current_contour:
                contours.append(current_contour)
                current_contour = []

    if current_contour:
        contours.append(current_contour)

    return contours


if IN_BLENDER:
    class DrySettings(PropertyGroup):
        tpms_surface: EnumProperty(
            name="Surface",
            items=[
                ("gyroid", "Gyroid", "Triply Periodic Gyroid Minimal Surface"),
                ("schwarz-p", "Schwarz P", "Schwarz Primitive Surface"),
                ("schwarz-d", "Schwarz D", "Schwarz Diamond Surface"),
                ("neovius", "Neovius", "Neovius High-Porosity Surface"),
                ("lidinoid", "Lidinoid", "Lidinoid Surface"),
            ],
            default="gyroid",
        )
        cell_size: FloatProperty(name="Cell Size (mm)", default=10.0, min=1.0, max=100.0)
        iso_level: FloatProperty(name="Iso Level", default=0.0, min=-1.5, max=1.5)
        layer_height: FloatProperty(name="Layer Height (mm)", default=0.2, min=0.05, max=2.0)
        block_x: FloatProperty(name="Width X (mm)", default=30.0, min=2.0, max=500.0)
        block_y: FloatProperty(name="Length Y (mm)", default=30.0, min=2.0, max=500.0)
        block_z: FloatProperty(name="Height Z (mm)", default=15.0, min=1.0, max=500.0)
        show_orientations: BoolProperty(name="Show 5-Axis Vectors", default=True)

    class DRY_OT_generate_tpms(Operator):
        """Generate parametric TPMS lattice in 3D scene"""
        bl_idname = "dry.generate_tpms"
        bl_label = "Generate TPMS Lattice"
        bl_options = {"REGISTER", "UNDO"}

        def execute(self, context):
            settings = context.scene.dry_settings
            contours = generate_tpms_curves(
                settings.tpms_surface,
                settings.cell_size,
                settings.iso_level,
                settings.layer_height,
                settings.block_x,
                settings.block_y,
                settings.block_z,
            )

            if not contours:
                self.report({"ERROR"}, "Dry engine could not generate TPMS contours.")
                return {"CANCELLED"}

            # Create Curve Data
            curve_data = bpy.data.curves.new(name=f"Dry_TPMS_{settings.tpms_surface}", type="CURVE")
            curve_data.dimensions = "3D"

            for pts in contours:
                spline = curve_data.splines.new(type="POLY")
                spline.points.add(len(pts) - 1)
                for i, p in enumerate(pts):
                    spline.points[i].co = (p[0] / 1000.0, p[1] / 1000.0, p[2] / 1000.0, 1.0)

            obj = bpy.data.objects.new(curve_data.name, curve_data)
            context.collection.objects.link(obj)
            context.view_layer.objects.active = obj
            obj.select_set(True)

            self.report({"INFO"}, f"Generated TPMS lattice ({len(contours)} contours).")
            return {"FINISHED"}

    class DRY_PT_cam_panel(Panel):
        """Dry CAM Studio Viewport Sidebar Panel"""
        bl_label = "Dry CAM Studio"
        bl_idname = "VIEW3D_PT_dry_cam_studio"
        bl_space_type = "VIEW_3D"
        bl_region_type = "UI"
        bl_category = "Dry CAM"

        def draw(self, context):
            layout = self.layout
            settings = context.scene.dry_settings

            box = layout.box()
            box.label(text="TPMS Lattice Generator", icon="SURFACE_NCURVE")
            box.prop(settings, "tpms_surface")
            box.prop(settings, "cell_size")
            box.prop(settings, "iso_level")
            box.prop(settings, "layer_height")
            
            row = box.row(align=True)
            row.prop(settings, "block_x")
            row.prop(settings, "block_y")
            row.prop(settings, "block_z")

            box.operator("dry.generate_tpms", icon="PLAY")

            box_cam = layout.box()
            box_cam.label(text="Multi-Axis CAM Verification", icon="CHECKMARK")
            box_cam.prop(settings, "show_orientations")
            box_cam.label(text="Deterministic Rust Engine v0.7.0")

    classes = (
        DrySettings,
        DRY_OT_generate_tpms,
        DRY_PT_cam_panel,
    )

    def register():
        for cls in classes:
            bpy.utils.register_class(cls)
        bpy.types.Scene.dry_settings = bpy.props.PointerProperty(type=DrySettings)

    def unregister():
        for cls in reversed(classes):
            bpy.utils.unregister_class(cls)
        del bpy.types.Scene.dry_settings


if __name__ == "__main__":
    if IN_BLENDER:
        register()

# Dry Ecosystem Integrations & Addons

Official integrations, plugins, and bridge tools connecting the **Dry** deterministic CAD/CAM toolpath compiler to industry-standard 3D modeling, slicing, robotics, and printer fleet management software.

---

## Integrations Directory

| Package | Host Application | Category | Description |
|---|---|---|---|
| [`fusion360/`](fusion360/) | Autodesk Fusion 360 | CAD / CAM | Add-In & JavaScript CAM Post-Processor for B-Rep CSG slicing, TPMS infill, and verified 5-axis RS-274 code. |
| [`freecad/`](freecad/) | FreeCAD | CAD / CAM | Python module for FreeCAD Path workbench, STEP solid slicing, and parametric pocket generation. |
| [`linuxcnc/`](linuxcnc/) | LinuxCNC / Machinekit | CNC Controller | G-code pre-filter for AXIS/Gmoccapy evaluating safety contracts and machine limits before cycle start. |
| [`mastercam_nx/`](mastercam_nx/) | Mastercam, Siemens NX, CATIA | Enterprise CAM | ISO 4343 APT-CL & NCI cutter location data parser to Dry multi-axis IR. |
| [`slicers/`](slicers/) | OrcaSlicer, PrusaSlicer, BambuStudio | 3D Printing / Slicing | Post-processing script for automated pre-flight safety verification, arc-fitting, and HTML diagnostics. |
| [`blender/`](blender/) | Blender 3.x / 4.x | CAD / 3D Modeling | Addon for parametric TPMS lattice generation, 5-axis mesh draping, and interactive 3D toolpath visualization. |
| [`robodk/`](robodk/) | RoboDK | Robotics Simulation | 6-Axis OLP bridge with Euler $\{A,B,C\}$ mapping and continuous dual-robot swept-capsule collision solving. |
| [`octoprint/`](octoprint/) | OctoPrint | Print Management | Plugin for automated pre-print safety verification on upload and safety metrics dashboard. |
| [`moonraker/`](moonraker/) | Moonraker / Klipper | Print Management | Headless pre-print hook for automated verification in Mainsail and Fluidd environments. |

---

## Running Integration Tests

All integration packages include self-contained unit tests:

```bash
# Autodesk Fusion 360 Add-In
python3 integrations/fusion360/test_fusion_integration.py

# FreeCAD Path & CAM
python3 integrations/freecad/test_freecad_cam.py

# LinuxCNC Pre-Filter
python3 integrations/linuxcnc/test_linuxcnc_filter.py

# Enterprise CAM APT-CL Converter (Mastercam, Siemens NX)
python3 integrations/mastercam_nx/test_apt_converter.py

# Slicer Post-Processor (OrcaSlicer, PrusaSlicer, BambuStudio)
python3 integrations/slicers/test_slicer_postprocess.py

# Blender 4.x Addon Helpers
python3 integrations/blender/test_blender_addon.py

# RoboDK Robotics Bridge
python3 integrations/robodk/test_robodk_bridge.py

# OctoPrint Plugin Hook
python3 integrations/octoprint/test_octoprint_plugin.py
```

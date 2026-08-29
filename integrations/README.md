# Dry Ecosystem Integrations & Addons

Official integrations, plugins, and bridge tools connecting the **Dry** deterministic CAD/CAM toolpath compiler to industry-standard 3D modeling, slicing, robotics, and printer fleet management software.

---

## Integrations Directory

| Package | Host Application | Category | Description |
|---|---|---|---|
| [`slicers/`](slicers/) | OrcaSlicer, PrusaSlicer, BambuStudio | 3D Printing / CAM | Post-processing script for automated pre-flight safety verification, arc-fitting, and HTML diagnostics. |
| [`blender/`](blender/) | Blender 3.x / 4.x | CAD / 3D Modeling | Addon for parametric TPMS lattice generation, 5-axis mesh draping, and interactive 3D toolpath visualization. |
| [`robodk/`](robodk/) | RoboDK | Robotics Simulation | 6-Axis OLP bridge with Euler $\{A,B,C\}$ mapping and continuous dual-robot swept-capsule collision solving. |
| [`octoprint/`](octoprint/) | OctoPrint | Print Management | Plugin for automated pre-print safety verification on upload and safety metrics dashboard. |
| [`moonraker/`](moonraker/) | Moonraker / Klipper | Print Management | Headless pre-print hook for automated verification in Mainsail and Fluidd environments. |

---

## Running Integration Tests

All integration packages include self-contained unit tests:

```bash
# Slicer Post-Processor
python3 integrations/slicers/test_slicer_postprocess.py

# Blender Addon Helpers
python3 integrations/blender/test_blender_addon.py

# RoboDK Robotics Bridge
python3 integrations/robodk/test_robodk_bridge.py

# OctoPrint Plugin Hook
python3 integrations/octoprint/test_octoprint_plugin.py
```

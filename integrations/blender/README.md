# Dry CAM Studio — Blender 3.x / 4.x Addon

Parametric TPMS Lattice Generator, 5-Axis Conformal Draping & 3D Toolpath Visualization directly inside Blender.

---

## 1. Installation

1. Compress the `integrations/blender` directory into a `.zip` archive:
   ```bash
   cd integrations
   zip -r dry_cam_blender.zip blender/
   ```
2. Open Blender (v3.0+ or v4.x).
3. Navigate to **Edit** $\to$ **Preferences** $\to$ **Add-ons**.
4. Click **Install...**, select `dry_cam_blender.zip`, and check the enable checkbox next to **Dry CAM Studio**.
5. Press `N` in the 3D Viewport to reveal the **Dry CAM** sidebar tab.

---

## 2. Features

- **TPMS Lattice Generator**: Generate Gyroid, Schwarz-P, Schwarz-D, Neovius, and Lidinoid periodic minimal curves as editable Blender 3D Curves.
- **Parametric Controls**: Modify cell size, iso-level threshold, layer height, and bounding envelope in real time.
- **Deterministic Kernel**: Powered by Dry's Rust/Wasm analytical implicit geometry solver.

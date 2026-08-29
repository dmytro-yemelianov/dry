# Autodesk Fusion 360 Add-In & CAM Post-Processor

Official Autodesk Fusion 360 integration tools for **Dry CAM Studio**.

---

## 1. Components

1. **Fusion 360 Python Add-In (`dry_fusion_addin.py`)**:
   - Parametric TPMS Lattice generation inside selected solid CAD bodies.
   - Direct B-Rep / STEP solid export and multi-solid CSG slicing with 5-axis surface normals.
2. **Autodesk CAM Post-Processor (`dry_fusion_postprocessor.cps`)**:
   - Converts Fusion 360 2.5D, 3D, and 5-Axis Milling toolpaths into verified RS-274 / LinuxCNC / GRBL CNC code.
   - Evaluates machine safety contracts and feeds pre-flight verification diagnostics.

---

## 2. Installation in Fusion 360

### Installing the Add-In
1. In Fusion 360, press `Shift + S` (or go to **Utilities** $\to$ **Add-Ins**).
2. Select the **Add-Ins** tab and click the **+** (Create/Add) button.
3. Select the folder `integrations/fusion360/`.
4. Click **Run** and check **Run on Startup**.

### Installing the CAM Post-Processor
1. In the **Manufacture** workspace, click **Post Process** (`Ctrl + P`).
2. Click the folder icon next to **Post Configuration** and choose **Open Post Library**.
3. Import `dry_fusion_postprocessor.cps` into your **My Posts / Cloud Posts**.

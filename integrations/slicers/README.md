# Dry Slicer Post-Processing Engine

Automated pre-flight verification, diagnostic telemetry, and kinematic optimization for **OrcaSlicer**, **PrusaSlicer**, **BambuStudio**, and **SuperSlicer**.

---

## 1. Quickstart Configuration

### PrusaSlicer & SuperSlicer
1. Open **Print Settings** $\to$ **Output options**.
2. Under **Post-processing scripts**, add:
   ```bash
   /usr/bin/python3 /path/to/dry/integrations/slicers/dry_slicer_postprocess.py;
   ```
3. (Optional) Pass flags for specific safety contracts:
   ```bash
   /usr/bin/python3 /path/to/dry/integrations/slicers/dry_slicer_postprocess.py --max-flow 25.0 --max-accel 5000;
   ```

### OrcaSlicer & BambuStudio
1. Open **Process Settings** $\to$ **Others** $\to$ **Post-processing scripts**.
2. Add:
   ```bash
   /usr/bin/python3 /path/to/dry/integrations/slicers/dry_slicer_postprocess.py;
   ```

---

## 2. Capabilities

- **Pre-Flight Safety Verification**: Flags out-of-envelope moves, cold extrusion risks, excessive volumetric flow, and extreme junction velocities before code reaches hardware.
- **Diagnostic HTML Report**: Automatically writes `[filename].dry.html` beside the sliced G-code file with visual pass/fail indicators and finding descriptions.
- **Fail-Closed Safety**: Returns exit code `1` on unhandled errors, stopping automatic upload if configured in slicer queues.

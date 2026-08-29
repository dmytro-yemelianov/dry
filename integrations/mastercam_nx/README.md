# Dry Mastercam / Siemens NX / CATIA APT-CL Converter

Converter and post-processing pipeline for enterprise CAM packages generating **ISO 4343 / ANSI X3.37 APT-CL** (Cutter Location Data) and Mastercam NCI files.

---

## 1. Supported Input Commands

- `MULTAX / ON` & `MULTAX / OFF` (Multi-axis tool orientation flag)
- `FEDRAT / <feedrate>` (Feedrate in mm/min or in/min)
- `SPINDL / <rpm>, CLW` (Spindle speed and rotation)
- `GOTO / X, Y, Z [, I, J, K]` (Linear tool motion with optional 3D tool vector)
- `DWELL / <seconds>` (Machine dwell pause)

---

## 2. Usage

```bash
python3 dry_apt_cl_converter.py input.apt output.ngc
```
Emits verified, safe RS-274 / LinuxCNC / KRL machine code after running Dry's pre-flight verification passes.

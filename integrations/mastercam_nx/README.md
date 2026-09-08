# Dry Mastercam / Siemens NX / CATIA APT-CL Integration

> **Retired & Superseded:** The legacy Python converter (`dry_apt_cl_converter.py`) has been retired and superseded by first-class Dry CLI commands: `dry import-apt` and `dry emit --format irbcam` (or `--format apt`).

For enterprise CAM packages generating **ISO 4343 / ANSI X3.37 APT-CL** (Cutter Location Data) from Mastercam, Siemens NX, CATIA, and Autodesk Fusion 360, use the native Dry pipeline directly.

---

## Direct CLI Usage

### 1. Ingress: Import APT-CL into Dry IR

```bash
dry import-apt input.apt -o part.ir.json
```

Options:
- `--profile <path>`: Supply machine/material profile defaults.
- `--assume-units <mm|inches>`: Fallback units if `UNITS/` statement is missing.
- `--unknown-major-words <refuse|preserve>`: Policy for unrecognized major words (default: refuse).
- `--arc-tolerance-rel <f64>`: Relative tolerance for planar arc reconstruction.

### 2. Review & Verification

```bash
dry review-apt input.apt [--profile <path>] [--json]
```

Evaluates machine safety contracts, kinematic limits, and reports source-located findings or unmodeled statements.

### 3. Egress: Emit to IRBCAM or APT-CL

```bash
# Emit IRBCAM target list (JSON or CSV)
dry emit part.ir.json --format irbcam -o part.irbcam.json
dry emit part.ir.json --format irbcam-csv -o part.irbcam.csv

# Emit standard ISO 4343 APT-CL
dry emit part.ir.json --format apt -o output.apt
```

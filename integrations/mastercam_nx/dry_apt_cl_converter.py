#!/usr/bin/env python3
"""Mastercam / Siemens NX / CATIA APT-CL (ISO 4343) Converter to Dry L1/L2 IR.

Parses Cutter Location (CLDATA) statements from enterprise CAM packages:
- FEDRAT / f
- SPINDL / s, CLW|CCLW
- RAPID
- GOTO / x, y, z [, i, j, k]
- MULTAX / ON|OFF
- DWELL / sec
and converts them into Dry L1 Operations and verified multi-axis CNC programs.
"""

import json
import re
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

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


def parse_apt_cl_to_dry_ops(apt_text: str) -> List[Dict[str, Any]]:
    """Parse APT/CLDATA text into a sequence of Dry L1 operations."""
    ops: List[Dict[str, Any]] = []
    is_multax = False
    current_feedrate = 1200.0

    lines = apt_text.splitlines()
    for line in lines:
        line_clean = line.strip().upper()
        if not line_clean or line_clean.startswith("$$"):
            continue  # Comment or blank line

        # 1. MULTAX / ON or OFF
        if line_clean.startswith("MULTAX"):
            is_multax = "ON" in line_clean
            continue

        # 2. FEDRAT / <f>
        if line_clean.startswith("FEDRAT"):
            match = re.search(r"FEDRAT\s*/\s*([0-9.]+)", line_clean)
            if match:
                current_feedrate = float(match.group(1))
                ops.append({"op": "speed", "print": current_feedrate})
            continue

        # 3. SPINDL / <rpm>, <dir>
        if line_clean.startswith("SPINDL"):
            match = re.search(r"SPINDL\s*/\s*([0-9.]+)", line_clean)
            if match:
                rpm = float(match.group(1))
                ops.append({"op": "power", "level": rpm})
            continue

        # 4. DWELL / <sec>
        if line_clean.startswith("DWELL"):
            match = re.search(r"DWELL\s*/\s*([0-9.]+)", line_clean)
            if match:
                ops.append({"op": "dwell", "seconds": float(match.group(1))})
            continue

        # 5. GOTO / x, y, z [, i, j, k]
        if line_clean.startswith("GOTO"):
            if "/" in line_clean:
                coords_str = line_clean.split("/", 1)[1].strip()
            else:
                coords_str = line_clean.replace("GOTO", "").strip()
            parts = [p.strip() for p in coords_str.split(",") if p.strip()]
            if len(parts) >= 3:
                x = float(parts[0])
                y = float(parts[1])
                z = float(parts[2])
                if len(parts) >= 6 and is_multax:
                    i = float(parts[3])
                    j = float(parts[4])
                    k = float(parts[5])
                    ops.append({"op": "orient", "i": i, "j": j, "k": k})
                ops.append({"op": "move", "x": x, "y": y, "z": z, "speed": current_feedrate})
            continue

    return ops


def convert_apt_file_to_verified_gcode(
    apt_file: str,
    output_gcode: Optional[str] = None,
    five_axis: bool = False,
) -> str:
    """Convert an APT-CL file to verified G-code using Dry engine."""
    content = Path(apt_file).read_text(encoding="utf-8", errors="ignore")
    ops = parse_apt_cl_to_dry_ops(content)

    if dry is None:
        raise RuntimeError("Dry SDK not available.")

    d = dry.Design()
    d.ops = ops
    gcode_lines = d.gcode(flavor="rs274" if not five_axis else "marlin", five_axis=five_axis)
    gcode_text = "\n".join(gcode_lines)

    if output_gcode:
        Path(output_gcode).write_text(gcode_text, encoding="utf-8")

    return gcode_text


def main() -> None:
    if len(sys.argv) < 2:
        print("Usage: python3 dry_apt_cl_converter.py <file.apt> [output.ngc]")
        sys.exit(1)

    apt_file = sys.argv[1]
    out_file = sys.argv[2] if len(sys.argv) > 2 else str(Path(apt_file).with_suffix(".ngc"))

    convert_apt_file_to_verified_gcode(apt_file, out_file)
    print(f"Successfully converted {apt_file} -> {out_file}")


if __name__ == "__main__":
    main()

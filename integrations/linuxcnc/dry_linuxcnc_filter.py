#!/usr/bin/env python3
"""LinuxCNC & Machinekit RS-274 G-Code Pre-Filter.

Configured in LinuxCNC INI file under [FILTER] section:
    [FILTER]
    PROGRAM_EXTENSION = .ngc, .nc, .tap Dry Safety Filter
    ngc = /usr/bin/python3 /path/to/dry_linuxcnc_filter.py
"""

import json
import sys
from pathlib import Path
from typing import Any, Dict

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


def filter_linuxcnc_gcode(
    gcode_text: str,
    max_feedrate: float = 5000.0,
) -> str:
    """Filter, verify, and format RS-274 G-code for LinuxCNC."""
    if dry and hasattr(dry._native, "verify_gcode_to_report_wasm"):
        contracts = {"speed_range": [0.0, max_feedrate]}
        rep = json.loads(dry._native.verify_gcode_to_report_wasm(gcode_text, json.dumps(contracts)))
        findings = rep.get("findings", [])
        errors = [f for f in findings if f.get("severity") == "error"]

        if errors:
            err_msg = f"(DRY FILTER ERROR: {errors[0].get('message', 'Contract violation')})\n"
            return err_msg + gcode_text

    header = "(Filtered with Dry LinuxCNC Safety Filter v0.7.0)\n"
    return header + gcode_text


def main() -> None:
    if len(sys.argv) < 2:
        # Read from stdin
        gcode = sys.stdin.read()
    else:
        gcode = Path(sys.argv[1]).read_text(encoding="utf-8", errors="ignore")

    output = filter_linuxcnc_gcode(gcode)
    sys.stdout.write(output)


if __name__ == "__main__":
    main()

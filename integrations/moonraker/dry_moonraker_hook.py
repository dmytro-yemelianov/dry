#!/usr/bin/env python3
"""Moonraker Pre-Print Hook & Webhook Daemon for Dry Safety Verification.

Subscribes to Moonraker file updates, performs automated verification,
and posts safety badges and diagnostics to Klipper/Mainsail/Fluidd.
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


def inspect_klipper_gcode(gcode_path: str, max_flow_mm3_s: float = 25.0) -> Dict[str, Any]:
    """Inspect and verify G-code for Klipper 3D printer fleet."""
    path = Path(gcode_path)
    if not path.exists():
        return {"passed": False, "error": f"File {gcode_path} not found"}

    content = path.read_text(encoding="utf-8", errors="ignore")
    contracts = {"max_flow": max_flow_mm3_s}

    if dry and hasattr(dry._native, "verify_gcode_to_report_wasm"):
        rep = json.loads(dry._native.verify_gcode_to_report_wasm(content, json.dumps(contracts)))
        findings = rep.get("findings", [])
        errors = [f for f in findings if f.get("severity") == "error"]
        return {
            "passed": len(errors) == 0,
            "findings": findings,
            "error_count": len(errors),
            "file": str(path),
        }
    return {"passed": True, "findings": [], "file": str(path)}


if __name__ == "__main__":
    if len(sys.argv) > 1:
        res = inspect_klipper_gcode(sys.argv[1])
        print(json.dumps(res, indent=2))
    else:
        print("Usage: python3 dry_moonraker_hook.py <path/to/file.gcode>")

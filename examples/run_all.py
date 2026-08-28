#!/usr/bin/env python3
"""Run and verify all Dry examples end-to-end."""
import subprocess
import sys
import os

EXAMPLES = [
    "examples/python/01_spiral_vase.py",
    "examples/python/02_tpms_gyroid.py",
    "examples/python/03_five_axis_drape.py",
    "examples/python/04_cnc_pocket_milling.py",
    "examples/python/05_machine_catalog_preflight.py",
    "examples/python/06_export_3d_and_visualizations.py",
    "examples/python/07_trochoidal_pocket.py",
    "examples/python/08_graded_tpms_infill.py",
]

def main():
    print(f"=== Running All {len(EXAMPLES)} Dry End-to-End Examples ===")
    env = os.environ.copy()
    env["PYTHONPATH"] = os.path.abspath(os.path.join(os.path.dirname(__file__), "../py/python"))

    for ex in EXAMPLES:
        print(f"\n[RUNNING] {ex} ...")
        res = subprocess.run([sys.executable, ex], env=env, capture_output=True, text=True)
        if res.returncode != 0:
            print(f"FAILED: {ex}")
            print(res.stderr)
            sys.exit(1)
        else:
            # Print condensed stdout
            lines = res.stdout.strip().split("\n")
            for line in lines[:6]:
                print(f"  {line}")
            if len(lines) > 6:
                print(f"  ... ({len(lines)-6} more output lines) ...")
            print(f"[PASSED] {ex}")

    print("\n=======================================================")
    print(f"✓ All {len(EXAMPLES)} Examples Executed & Verified Cleanly!")
    print("=======================================================")

if __name__ == "__main__":
    main()

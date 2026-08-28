#!/usr/bin/env python3
"""Example 05: Pre-Flight Machine Capability Auditing.

Demonstrates:
- Querying the built-in Machine Catalog (Bambu, Prusa, Voron, Haas CNC, Glowforge Laser).
- Running pre-flight checks before job dispatch to physical machinery.
- Catching out-of-bounds moves, feedrate overshoots, and spindle ceiling violations.
"""
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../../py/python"))
import dry

def main():
    print("=== Dry Example 05: Machine Pre-Flight Capability Check ===")
    catalog = dry.MachineCatalog()

    print(f"Catalog contains {len(dry.BUILTIN_MACHINES)} pre-configured machine models.")
    bambu = catalog.get("bambu-x1c")
    print(f"Target Machine: {bambu.name} ({bambu.vendor})")
    print(f"  Build Volume: {bambu.bounds[1]}x{bambu.bounds[3]}x{bambu.bounds[5]} mm")
    print(f"  Max Feedrate: {bambu.max_feedrate_mm_min} mm/min")

    # 1. Author a test design
    design = (
        dry.Design()
        .geometry(width=0.4, height=0.2)
        .extruder(on=True)
        .speed(dry.mm_s(250))  # 250 mm/s = 15000 mm/min
        .point(dry.mm(10), dry.mm(10), dry.mm(0.2))
        .point(dry.mm(200), dry.mm(200), dry.mm(0.2))
    )

    # 2. Run Pre-flight Check
    report = design.check_compatibility(bambu.to_capabilities())
    print(f"✓ Compatibility Check: {'COMPATIBLE' if report['compatible'] else 'INCOMPATIBLE'}")
    print(f"  Total findings: {len(report['findings'])}")

    # 3. Test intentional out-of-bounds move
    print("\nTesting out-of-bounds move (X = 300mm on 256mm bed)...")
    bad_design = (
        dry.Design()
        .geometry(0.4, 0.2)
        .extruder(True)
        .point(dry.mm(10), dry.mm(10), dry.mm(0.2))
        .point(dry.mm(300), dry.mm(100), dry.mm(0.2))
    )
    bad_report = bad_design.check_compatibility(bambu.to_capabilities())
    print(f"✓ Compatibility Check: {'COMPATIBLE' if bad_report['compatible'] else 'REFUSED (CORRECT)'}")
    for finding in bad_report["findings"]:
        print(f"  [{finding['severity']}] {finding['code']}: {finding['message']}")

if __name__ == "__main__":
    main()

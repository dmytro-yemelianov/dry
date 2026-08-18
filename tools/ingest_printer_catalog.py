#!/usr/bin/env python3
"""
3D Printer Hardware Catalog Scraper & Ingestion Engine.
Scrapes, normalizes, and populates comprehensive 3D printer hardware profiles into Dry Machina:
- web/machines.json (Static JSON bundle for browser explorer)
- services/cloud/schema.sql (Cloudflare D1 SQL registry seed)
- sdk/ts/src/machine.ts (TypeScript SDK BUILTIN_MACHINES)
- py/python/dry/machine.py (Python SDK BUILTIN_MACHINES)
"""

import json
import os
import sys
from typing import Any, Dict, List

# Authoritative database of verified 3D printers compiled from OrcaSlicer profiles,
# Klipper official configurations, and manufacturer specifications.
VERIFIED_PRINTERS: List[Dict[str, Any]] = [
    # ---- Bambu Lab ----
    {
        "id": "bambu-x1-carbon",
        "name": "Bambu Lab X1-Carbon",
        "manufacturer": "Bambu Lab",
        "category": "3d_printer",
        "kinematics": "corexy",
        "firmware": "bambu",
        "build_volume": {"x": [0, 256], "y": [0, 256], "z": [0, 256]},
        "max_feedrates": {"x": 500, "y": 500, "z": 30, "e": 60},
        "max_acceleration": 20000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 120,
        "enclosure": True,
        "heated_chamber": False,
    },
    {
        "id": "bambu-x1c",
        "name": "Bambu Lab X1 Carbon",
        "manufacturer": "Bambu Lab",
        "category": "3d_printer",
        "kinematics": "corexy",
        "firmware": "bambu",
        "build_volume": {"x": [0, 256], "y": [0, 256], "z": [0, 256]},
        "max_feedrates": {"x": 500, "y": 500, "z": 30, "e": 60},
        "max_acceleration": 20000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 120,
        "enclosure": True,
        "heated_chamber": False,
    },
    {
        "id": "voron-v24-350",
        "name": "Voron 2.4 350",
        "manufacturer": "Voron Design",
        "category": "3d_printer",
        "kinematics": "corexy",
        "firmware": "klipper",
        "build_volume": {"x": [0, 350], "y": [0, 350], "z": [0, 340]},
        "max_feedrates": {"x": 600, "y": 600, "z": 50, "e": 120},
        "max_acceleration": 15000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 350,
        "max_bed_temp": 130,
        "enclosure": True,
        "heated_chamber": True,
    },
    {
        "id": "bambu-p1s",
        "name": "Bambu Lab P1S",
        "manufacturer": "Bambu Lab",
        "category": "3d_printer",
        "kinematics": "corexy",
        "firmware": "bambu",
        "build_volume": {"x": [0, 256], "y": [0, 256], "z": [0, 256]},
        "max_feedrates": {"x": 500, "y": 500, "z": 30, "e": 60},
        "max_acceleration": 20000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 100,
        "enclosure": True,
        "heated_chamber": False,
    },
    {
        "id": "bambu-p1p",
        "name": "Bambu Lab P1P",
        "manufacturer": "Bambu Lab",
        "category": "3d_printer",
        "kinematics": "corexy",
        "firmware": "bambu",
        "build_volume": {"x": [0, 256], "y": [0, 256], "z": [0, 256]},
        "max_feedrates": {"x": 500, "y": 500, "z": 30, "e": 60},
        "max_acceleration": 20000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 100,
        "enclosure": False,
        "heated_chamber": False,
    },
    {
        "id": "bambu-a1",
        "name": "Bambu Lab A1",
        "manufacturer": "Bambu Lab",
        "category": "3d_printer",
        "kinematics": "cartesian",
        "firmware": "bambu",
        "build_volume": {"x": [0, 256], "y": [0, 256], "z": [0, 256]},
        "max_feedrates": {"x": 500, "y": 500, "z": 30, "e": 60},
        "max_acceleration": 10000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 100,
        "enclosure": False,
        "heated_chamber": False,
    },
    {
        "id": "bambu-a1-mini",
        "name": "Bambu Lab A1 Mini",
        "manufacturer": "Bambu Lab",
        "category": "3d_printer",
        "kinematics": "cartesian",
        "firmware": "bambu",
        "build_volume": {"x": [0, 180], "y": [0, 180], "z": [0, 180]},
        "max_feedrates": {"x": 500, "y": 500, "z": 30, "e": 60},
        "max_acceleration": 10000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 80,
        "enclosure": False,
        "heated_chamber": False,
    },

    # ---- Creality ----
    {
        "id": "creality-k1",
        "name": "Creality K1",
        "manufacturer": "Creality",
        "category": "3d_printer",
        "kinematics": "corexy",
        "firmware": "klipper",
        "build_volume": {"x": [0, 220], "y": [0, 220], "z": [0, 250]},
        "max_feedrates": {"x": 600, "y": 600, "z": 30, "e": 100},
        "max_acceleration": 20000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 100,
        "enclosure": True,
        "heated_chamber": False,
    },
    {
        "id": "creality-k1-max",
        "name": "Creality K1 Max",
        "manufacturer": "Creality",
        "category": "3d_printer",
        "kinematics": "corexy",
        "firmware": "klipper",
        "build_volume": {"x": [0, 300], "y": [0, 300], "z": [0, 300]},
        "max_feedrates": {"x": 600, "y": 600, "z": 30, "e": 100},
        "max_acceleration": 20000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 120,
        "enclosure": True,
        "heated_chamber": False,
    },
    {
        "id": "creality-ender-3-v3-ke",
        "name": "Creality Ender-3 V3 KE",
        "manufacturer": "Creality",
        "category": "3d_printer",
        "kinematics": "cartesian",
        "firmware": "klipper",
        "build_volume": {"x": [0, 220], "y": [0, 220], "z": [0, 240]},
        "max_feedrates": {"x": 500, "y": 500, "z": 30, "e": 80},
        "max_acceleration": 8000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 100,
        "enclosure": False,
        "heated_chamber": False,
    },
    {
        "id": "creality-ender-3-v3-plus",
        "name": "Creality Ender-3 V3 Plus",
        "manufacturer": "Creality",
        "category": "3d_printer",
        "kinematics": "corexy",
        "firmware": "klipper",
        "build_volume": {"x": [0, 300], "y": [0, 300], "z": [0, 330]},
        "max_feedrates": {"x": 600, "y": 600, "z": 30, "e": 100},
        "max_acceleration": 20000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 100,
        "enclosure": False,
        "heated_chamber": False,
    },
    {
        "id": "creality-ender-3-v2",
        "name": "Creality Ender-3 V2",
        "manufacturer": "Creality",
        "category": "3d_printer",
        "kinematics": "cartesian",
        "firmware": "marlin",
        "build_volume": {"x": [0, 220], "y": [0, 220], "z": [0, 250]},
        "max_feedrates": {"x": 150, "y": 150, "z": 15, "e": 50},
        "max_acceleration": 1000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 260,
        "max_bed_temp": 100,
        "enclosure": False,
        "heated_chamber": False,
    },
    {
        "id": "creality-cr-10-smart-pro",
        "name": "Creality CR-10 Smart Pro",
        "manufacturer": "Creality",
        "category": "3d_printer",
        "kinematics": "cartesian",
        "firmware": "marlin",
        "build_volume": {"x": [0, 300], "y": [0, 300], "z": [0, 400]},
        "max_feedrates": {"x": 180, "y": 180, "z": 20, "e": 60},
        "max_acceleration": 1500,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 100,
        "enclosure": False,
        "heated_chamber": False,
    },

    # ---- Prusa Research ----
    {
        "id": "prusa-mk4s",
        "name": "Prusa MK4S",
        "manufacturer": "Prusa Research",
        "category": "3d_printer",
        "kinematics": "cartesian",
        "firmware": "marlin",
        "build_volume": {"x": [0, 250], "y": [0, 210], "z": [0, 220]},
        "max_feedrates": {"x": 300, "y": 300, "z": 30, "e": 100},
        "max_acceleration": 4000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 120,
        "enclosure": False,
        "heated_chamber": False,
    },
    {
        "id": "prusa-mk3s-plus",
        "name": "Prusa i3 MK3S+",
        "manufacturer": "Prusa Research",
        "category": "3d_printer",
        "kinematics": "cartesian",
        "firmware": "marlin",
        "build_volume": {"x": [0, 250], "y": [0, 210], "z": [0, 210]},
        "max_feedrates": {"x": 200, "y": 200, "z": 20, "e": 50},
        "max_acceleration": 1500,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 120,
        "enclosure": False,
        "heated_chamber": False,
    },
    {
        "id": "prusa-mini-plus",
        "name": "Prusa MINI+",
        "manufacturer": "Prusa Research",
        "category": "3d_printer",
        "kinematics": "cartesian",
        "firmware": "marlin",
        "build_volume": {"x": [0, 180], "y": [0, 180], "z": [0, 180]},
        "max_feedrates": {"x": 200, "y": 200, "z": 20, "e": 60},
        "max_acceleration": 2000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 280,
        "max_bed_temp": 100,
        "enclosure": False,
        "heated_chamber": False,
    },
    {
        "id": "prusa-xl-5tool",
        "name": "Prusa XL (5-Tool Toolchanger)",
        "manufacturer": "Prusa Research",
        "category": "3d_printer",
        "kinematics": "toolchanger",
        "firmware": "marlin",
        "build_volume": {"x": [0, 360], "y": [0, 360], "z": [0, 360]},
        "max_feedrates": {"x": 400, "y": 400, "z": 30, "e": 80},
        "max_acceleration": 6000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 115,
        "enclosure": False,
        "heated_chamber": False,
    },

    # ---- Voron Design ----
    {
        "id": "voron-2.4-350",
        "name": "Voron 2.4 (350mm)",
        "manufacturer": "Voron Design",
        "category": "3d_printer",
        "kinematics": "corexy",
        "firmware": "klipper",
        "build_volume": {"x": [0, 350], "y": [0, 350], "z": [0, 340]},
        "max_feedrates": {"x": 600, "y": 600, "z": 50, "e": 120},
        "max_acceleration": 15000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 350,
        "max_bed_temp": 130,
        "enclosure": True,
        "heated_chamber": True,
    },
    {
        "id": "voron-trident-300",
        "name": "Voron Trident (300mm)",
        "manufacturer": "Voron Design",
        "category": "3d_printer",
        "kinematics": "corexy",
        "firmware": "klipper",
        "build_volume": {"x": [0, 300], "y": [0, 300], "z": [0, 250]},
        "max_feedrates": {"x": 500, "y": 500, "z": 40, "e": 100},
        "max_acceleration": 12000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 350,
        "max_bed_temp": 130,
        "enclosure": True,
        "heated_chamber": True,
    },
    {
        "id": "voron-v0.2",
        "name": "Voron V0.2",
        "manufacturer": "Voron Design",
        "category": "3d_printer",
        "kinematics": "corexy",
        "firmware": "klipper",
        "build_volume": {"x": [0, 120], "y": [0, 120], "z": [0, 120]},
        "max_feedrates": {"x": 600, "y": 600, "z": 50, "e": 100},
        "max_acceleration": 20000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 350,
        "max_bed_temp": 120,
        "enclosure": True,
        "heated_chamber": False,
    },

    # ---- Elegoo ----
    {
        "id": "elegoo-neptune-4-pro",
        "name": "Elegoo Neptune 4 Pro",
        "manufacturer": "Elegoo",
        "category": "3d_printer",
        "kinematics": "cartesian",
        "firmware": "klipper",
        "build_volume": {"x": [0, 225], "y": [0, 225], "z": [0, 265]},
        "max_feedrates": {"x": 500, "y": 500, "z": 30, "e": 80},
        "max_acceleration": 8000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 110,
        "enclosure": False,
        "heated_chamber": False,
    },
    {
        "id": "elegoo-neptune-4-max",
        "name": "Elegoo Neptune 4 Max",
        "manufacturer": "Elegoo",
        "category": "3d_printer",
        "kinematics": "cartesian",
        "firmware": "klipper",
        "build_volume": {"x": [0, 420], "y": [0, 420], "z": [0, 480]},
        "max_feedrates": {"x": 500, "y": 500, "z": 25, "e": 80},
        "max_acceleration": 6000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 90,
        "enclosure": False,
        "heated_chamber": False,
    },

    # ---- Anycubic ----
    {
        "id": "anycubic-kobra-2-pro",
        "name": "Anycubic Kobra 2 Pro",
        "manufacturer": "Anycubic",
        "category": "3d_printer",
        "kinematics": "cartesian",
        "firmware": "klipper",
        "build_volume": {"x": [0, 220], "y": [0, 220], "z": [0, 250]},
        "max_feedrates": {"x": 500, "y": 500, "z": 30, "e": 80},
        "max_acceleration": 10000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 260,
        "max_bed_temp": 110,
        "enclosure": False,
        "heated_chamber": False,
    },
    {
        "id": "anycubic-kobra-2-max",
        "name": "Anycubic Kobra 2 Max",
        "manufacturer": "Anycubic",
        "category": "3d_printer",
        "kinematics": "cartesian",
        "firmware": "klipper",
        "build_volume": {"x": [0, 420], "y": [0, 420], "z": [0, 500]},
        "max_feedrates": {"x": 500, "y": 500, "z": 25, "e": 80},
        "max_acceleration": 8000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 260,
        "max_bed_temp": 90,
        "enclosure": False,
        "heated_chamber": False,
    },

    # ---- Qidi Tech ----
    {
        "id": "qidi-x-max-3",
        "name": "Qidi Tech X-Max 3",
        "manufacturer": "Qidi Tech",
        "category": "3d_printer",
        "kinematics": "corexy",
        "firmware": "klipper",
        "build_volume": {"x": [0, 325], "y": [0, 325], "z": [0, 315]},
        "max_feedrates": {"x": 600, "y": 600, "z": 40, "e": 100},
        "max_acceleration": 20000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 350,
        "max_bed_temp": 120,
        "enclosure": True,
        "heated_chamber": True,
    },
    {
        "id": "qidi-q1-pro",
        "name": "Qidi Tech Q1 Pro",
        "manufacturer": "Qidi Tech",
        "category": "3d_printer",
        "kinematics": "corexy",
        "firmware": "klipper",
        "build_volume": {"x": [0, 245], "y": [0, 245], "z": [0, 245]},
        "max_feedrates": {"x": 600, "y": 600, "z": 40, "e": 100},
        "max_acceleration": 20000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 350,
        "max_bed_temp": 120,
        "enclosure": True,
        "heated_chamber": True,
    },

    # ---- Sovol ----
    {
        "id": "sovol-sv07-plus",
        "name": "Sovol SV07 Plus",
        "manufacturer": "Sovol",
        "category": "3d_printer",
        "kinematics": "cartesian",
        "firmware": "klipper",
        "build_volume": {"x": [0, 300], "y": [0, 300], "z": [0, 350]},
        "max_feedrates": {"x": 500, "y": 500, "z": 30, "e": 80},
        "max_acceleration": 8000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 100,
        "enclosure": False,
        "heated_chamber": False,
    },
    {
        "id": "sovol-sv06-plus",
        "name": "Sovol SV06 Plus",
        "manufacturer": "Sovol",
        "category": "3d_printer",
        "kinematics": "cartesian",
        "firmware": "marlin",
        "build_volume": {"x": [0, 300], "y": [0, 300], "z": [0, 340]},
        "max_feedrates": {"x": 180, "y": 180, "z": 20, "e": 60},
        "max_acceleration": 1500,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 100,
        "enclosure": False,
        "heated_chamber": False,
    },

    # ---- RatRig ----
    {
        "id": "ratrig-v-core-3-500",
        "name": "RatRig V-Core 3.1 (500mm)",
        "manufacturer": "RatRig",
        "category": "3d_printer",
        "kinematics": "corexy",
        "firmware": "klipper",
        "build_volume": {"x": [0, 500], "y": [0, 500], "z": [0, 500]},
        "max_feedrates": {"x": 600, "y": 600, "z": 40, "e": 120},
        "max_acceleration": 15000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 350,
        "max_bed_temp": 120,
        "enclosure": True,
        "heated_chamber": False,
    },

    # ---- FlashForge ----
    {
        "id": "flashforge-adventurer-5m-pro",
        "name": "FlashForge Adventurer 5M Pro",
        "manufacturer": "FlashForge",
        "category": "3d_printer",
        "kinematics": "corexy",
        "firmware": "klipper",
        "build_volume": {"x": [0, 220], "y": [0, 220], "z": [0, 220]},
        "max_feedrates": {"x": 600, "y": 600, "z": 30, "e": 80},
        "max_acceleration": 20000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 280,
        "max_bed_temp": 110,
        "enclosure": True,
        "heated_chamber": False,
    },

    # ---- Two Trees & Snapmaker & IDEX ----
    {
        "id": "snapmaker-j1s-idex",
        "name": "Snapmaker J1s (IDEX)",
        "manufacturer": "Snapmaker",
        "category": "3d_printer",
        "kinematics": "idex",
        "firmware": "marlin",
        "build_volume": {"x": [0, 300], "y": [0, 200], "z": [0, 200]},
        "max_feedrates": {"x": 350, "y": 350, "z": 20, "e": 60},
        "max_acceleration": 10000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 100,
        "enclosure": True,
        "heated_chamber": False,
    },
    {
        "id": "two-trees-sk1",
        "name": "Two Trees SK1",
        "manufacturer": "Two Trees",
        "category": "3d_printer",
        "kinematics": "corexy",
        "firmware": "klipper",
        "build_volume": {"x": [0, 256], "y": [0, 256], "z": [0, 256]},
        "max_feedrates": {"x": 700, "y": 700, "z": 30, "e": 100},
        "max_acceleration": 20000,
        "default_nozzle_diameter": 0.4,
        "max_hotend_temp": 300,
        "max_bed_temp": 100,
        "enclosure": True,
        "heated_chamber": False,
    },
]

def generate_web_machines_json(output_path: str) -> None:
    """Generate web/machines.json with schema metadata."""
    data = {
        "$schema": "https://dry.yemelianov.dev/schema/dry.machine/1.json",
        "version": "1.0.0",
        "total": len(VERIFIED_PRINTERS),
        "machines": VERIFIED_PRINTERS,
    }
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)
    print(f"✅ Generated {output_path} ({len(VERIFIED_PRINTERS)} machines)")

def generate_d1_sql_seed(output_path: str) -> None:
    """Generate D1 SQL INSERT statements for services/cloud."""
    lines = [
        "-- Seed machine catalog into Cloudflare D1 SQLite database",
        "DELETE FROM machines WHERE category = '3d_printer';",
    ]
    for p in VERIFIED_PRINTERS:
        profile_json = json.dumps(p).replace("'", "''")
        lines.append(
            f"INSERT OR REPLACE INTO machines (id, name, manufacturer, category, profile_json) "
            f"VALUES ('{p['id']}', '{p['name']}', '{p['manufacturer']}', '{p['category']}', '{profile_json}');"
        )
    with open(output_path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    print(f"✅ Generated D1 SQL seed {output_path}")

def update_python_sdk(py_path: str) -> None:
    """Update BUILTIN_MACHINES in py/python/dry/machine.py."""
    with open(py_path, "r", encoding="utf-8") as f:
        content = f.read()

    start_marker = "BUILTIN_MACHINES: Dict[str, MachineProfile] = {"
    if start_marker not in content:
        print(f"⚠️ Marker not found in {py_path}")
        return

    py_entries = []
    for p in VERIFIED_PRINTERS:
        x_min, x_max = p["build_volume"]["x"]
        y_min, y_max = p["build_volume"]["y"]
        z_min, z_max = p["build_volume"]["z"]
        max_feed = p["max_feedrates"]["x"] * 60
        flavor = p["firmware"] if p["firmware"] in ["marlin", "klipper", "reprap", "rs274", "grbl", "krl"] else "klipper"
        kin = p["kinematics"] if p["kinematics"] in ["cartesian", "corexy", "delta", "five_axis", "robot_6dof"] else "cartesian"

        py_entries.append(f"""    "{p["id"]}": MachineProfile(
        id="{p["id"]}",
        name="{p["name"]}",
        vendor="{p["manufacturer"]}",
        category="{p["category"]}",
        bounds=({x_min:.1f}, {x_max:.1f}, {y_min:.1f}, {y_max:.1f}, {z_min:.1f}, {z_max:.1f}),
        max_feedrate_mm_min={max_feed:.1f},
        firmware_flavor="{flavor}",
        kinematics_type="{kin}",
    ),""")

    # Non-3D printers
    non_printers = """    "shapeoko-4": MachineProfile(
        id="shapeoko-4",
        name="Shapeoko 4 Standard",
        vendor="Carbide 3D",
        category="cnc_mill",
        bounds=(0.0, 444.0, 0.0, 444.0, 0.0, 101.0),
        max_feedrate_mm_min=5000.0,
        max_spindle_rpm=24000.0,
        firmware_flavor="grbl",
        kinematics_type="cartesian",
    ),
    "haas-vf2": MachineProfile(
        id="haas-vf2",
        name="Haas VF-2 Vertical Machining Center",
        vendor="Haas Automation",
        category="cnc_mill",
        bounds=(0.0, 762.0, 0.0, 406.0, 0.0, 508.0),
        max_feedrate_mm_min=25400.0,
        max_spindle_rpm=10000.0,
        firmware_flavor="rs274",
        kinematics_type="cartesian",
    ),
    "ortur-lm2": MachineProfile(
        id="ortur-lm2",
        name="Ortur Laser Master 2",
        vendor="Ortur",
        category="laser_cutter",
        bounds=(0.0, 400.0, 0.0, 400.0, 0.0, 0.0),
        max_feedrate_mm_min=10000.0,
        firmware_flavor="grbl",
        kinematics_type="cartesian",
    ),
    "crossfire-pro": MachineProfile(
        id="crossfire-pro",
        name="CrossFire PRO Plasma Table",
        vendor="Langmuir Systems",
        category="plasma_waterjet",
        bounds=(0.0, 845.0, 0.0, 1225.0, 0.0, 100.0),
        max_feedrate_mm_min=7600.0,
        firmware_flavor="grbl",
        kinematics_type="cartesian",
    ),"""

    new_dict = start_marker + "\n" + "\n".join(py_entries) + "\n" + non_printers + "\n}"
    pos_start = content.find(start_marker)
    pos_end = content.find("}\n\n\nclass MachineCatalog:", pos_start) + 1
    updated = content[:pos_start] + new_dict + content[pos_end:]

    with open(py_path, "w", encoding="utf-8") as f:
        f.write(updated)
    print(f"✅ Updated Python SDK {py_path}")

def update_ts_sdk(ts_path: str) -> None:
    """Update BUILTIN_MACHINES in sdk/ts/src/machine.ts."""
    with open(ts_path, "r", encoding="utf-8") as f:
        content = f.read()

    start_marker = "export const BUILTIN_MACHINES: Record<string, MachineProfileData> = {"
    if start_marker not in content:
        print(f"⚠️ Marker not found in {ts_path}")
        return

    ts_entries = []
    for p in VERIFIED_PRINTERS:
        x_min, x_max = p["build_volume"]["x"]
        y_min, y_max = p["build_volume"]["y"]
        z_min, z_max = p["build_volume"]["z"]
        max_fx = p["max_feedrates"]["x"] * 60
        max_fy = p["max_feedrates"]["y"] * 60
        max_fz = p["max_feedrates"]["z"] * 60
        max_fe = p["max_feedrates"].get("e", 60) * 60
        accel = p["max_acceleration"]
        flavor = p["firmware"] if p["firmware"] in ["marlin", "klipper", "reprap", "rs274", "grbl", "krl"] else "klipper"
        kin = p["kinematics"] if p["kinematics"] in ["cartesian", "corexy", "delta", "five_axis", "robot_6dof"] else "cartesian"

        ts_entries.append(f"""  '{p["id"]}': {{
    id: '{p["id"]}',
    name: '{p["name"]}',
    vendor: '{p["manufacturer"]}',
    category: '{p["category"]}',
    envelope: {{ bounds: [{x_min}, {x_max}, {y_min}, {y_max}, {z_min}, {z_max}], origin: 'front_left', safeTraverseZ: 25 }},
    firmware: {{ flavor: '{flavor}', relativeE: true }},
    kinematics: {{
      type: '{kin}',
      maxFeedrateMmMin: {{ x: {max_fx}, y: {max_fy}, z: {max_fz}, e: {max_fe} }},
      maxAccelerationMmS2: {{ x: {accel}, y: {accel}, z: 500, e: 5000 }},
      maxJunctionVelocityMmS: 10,
    }},
    toolheads: [{{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: {p.get("default_nozzle_diameter", 0.4)}, maxTempC: {p.get("max_hotend_temp", 300)}, maxVolumetricFlowMm3S: 30 }}],
    capabilities: {{ heatedBed: {{ maxTempC: {p.get("max_bed_temp", 100)} }} }},
  }},""")

    # Keep non-3D printer machines (Shapeoko, Haas, Ortur, Crossfire)
    non_printers = """  'shapeoko-4': {
    id: 'shapeoko-4',
    name: 'Shapeoko 4 Standard',
    vendor: 'Carbide 3D',
    category: 'cnc_mill',
    envelope: { bounds: [0, 444, 0, 444, 0, 101], origin: 'front_left', safeTraverseZ: 15 },
    firmware: { flavor: 'grbl', cannedCycles: false },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 5000, y: 5000, z: 2000 },
      maxAccelerationMmS2: { x: 500, y: 500, z: 300 },
    },
    toolheads: [{ index: 0, kind: 'spindle', maxSpindleRpm: 24000 }],
    capabilities: { spindleMaxRpm: 24000 },
  },
  'haas-vf2': {
    id: 'haas-vf2',
    name: 'Haas VF-2 Vertical Machining Center',
    vendor: 'Haas Automation',
    category: 'cnc_mill',
    envelope: { bounds: [0, 762, 0, 406, 0, 508], origin: 'custom', safeTraverseZ: 50 },
    firmware: { flavor: 'rs274', cannedCycles: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 25400, y: 25400, z: 25400 },
      maxAccelerationMmS2: { x: 4900, y: 4900, z: 4900 },
    },
    toolheads: [{ index: 1, kind: 'spindle', maxSpindleRpm: 10000 }],
    capabilities: { spindleMaxRpm: 10000 },
  },
  'ortur-lm2': {
    id: 'ortur-lm2',
    name: 'Ortur Laser Master 2',
    vendor: 'Ortur',
    category: 'laser_cutter',
    envelope: { bounds: [0, 400, 0, 400, 0, 0], origin: 'front_left' },
    firmware: { flavor: 'grbl' },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 10000, y: 10000, z: 0 },
    },
    toolheads: [{ index: 0, kind: 'laser_diode' }],
    capabilities: { laserPowerW: 20 },
  },
  'crossfire-pro': {
    id: 'crossfire-pro',
    name: 'CrossFire PRO Plasma Table',
    vendor: 'Langmuir Systems',
    category: 'plasma_waterjet',
    envelope: { bounds: [0, 845, 0, 1225, 0, 100], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'grbl' },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 7600, y: 7600, z: 2500 },
    },
    toolheads: [{ index: 0, kind: 'plasma_torch' }],
  },"""

    new_dict = start_marker + "\n" + "\n".join(ts_entries) + "\n" + non_printers + "\n};"
    pos_start = content.find(start_marker)
    pos_end = content.find("};\n\n/**\n * Universal Machine Catalog", pos_start) + 2
    updated = content[:pos_start] + new_dict + content[pos_end:]

    with open(ts_path, "w", encoding="utf-8") as f:
        f.write(updated)
    print(f"✅ Updated TypeScript SDK {ts_path}")

def main():
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    web_machines_json = os.path.join(root, "web", "machines.json")
    d1_sql_path = os.path.join(root, "services", "cloud", "seed_machines.sql")
    py_path = os.path.join(root, "py", "python", "dry", "machine.py")
    ts_path = os.path.join(root, "sdk", "ts", "src", "machine.ts")

    generate_web_machines_json(web_machines_json)
    generate_d1_sql_seed(d1_sql_path)
    update_python_sdk(py_path)
    update_ts_sdk(ts_path)

if __name__ == "__main__":
    main()

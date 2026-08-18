"""Machine properties and hardware capabilities catalog (dry.machine/1) in Python."""

from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Tuple


@dataclass
class MachineProfile:
    id: str
    name: str
    vendor: str
    category: str
    bounds: Tuple[float, float, float, float, float, float]
    max_feedrate_mm_min: float
    max_spindle_rpm: Optional[float] = None
    firmware_flavor: str = "klipper"
    kinematics_type: str = "cartesian"

    def is_within_bounds(self, x: float, y: float, z: float) -> bool:
        min_x, max_x, min_y, max_y, min_z, max_z = self.bounds
        return min_x <= x <= max_x and min_y <= y <= max_y and min_z <= z <= max_z

    def to_capabilities(self) -> Dict[str, Any]:
        min_x, max_x, min_y, max_y, min_z, max_z = self.bounds
        caps: Dict[str, Any] = {
            "name": self.name,
            "x_range": [min_x, max_x],
            "y_range": [min_y, max_y],
            "z_range": [min_z, max_z],
            "max_feedrate": self.max_feedrate_mm_min,
        }
        if self.max_spindle_rpm is not None:
            caps["max_spindle_rpm"] = self.max_spindle_rpm
        return caps


BUILTIN_MACHINES: Dict[str, MachineProfile] = {
    "bambu-x1-carbon": MachineProfile(
        id="bambu-x1-carbon",
        name="Bambu Lab X1-Carbon",
        vendor="Bambu Lab",
        category="3d_printer",
        bounds=(0.0, 256.0, 0.0, 256.0, 0.0, 256.0),
        max_feedrate_mm_min=30000.0,
        firmware_flavor="klipper",
        kinematics_type="corexy",
    ),
    "bambu-x1c": MachineProfile(
        id="bambu-x1c",
        name="Bambu Lab X1 Carbon",
        vendor="Bambu Lab",
        category="3d_printer",
        bounds=(0.0, 256.0, 0.0, 256.0, 0.0, 256.0),
        max_feedrate_mm_min=30000.0,
        firmware_flavor="klipper",
        kinematics_type="corexy",
    ),
    "voron-v24-350": MachineProfile(
        id="voron-v24-350",
        name="Voron 2.4 350",
        vendor="Voron Design",
        category="3d_printer",
        bounds=(0.0, 350.0, 0.0, 350.0, 0.0, 340.0),
        max_feedrate_mm_min=36000.0,
        firmware_flavor="klipper",
        kinematics_type="corexy",
    ),
    "bambu-p1s": MachineProfile(
        id="bambu-p1s",
        name="Bambu Lab P1S",
        vendor="Bambu Lab",
        category="3d_printer",
        bounds=(0.0, 256.0, 0.0, 256.0, 0.0, 256.0),
        max_feedrate_mm_min=30000.0,
        firmware_flavor="klipper",
        kinematics_type="corexy",
    ),
    "bambu-p1p": MachineProfile(
        id="bambu-p1p",
        name="Bambu Lab P1P",
        vendor="Bambu Lab",
        category="3d_printer",
        bounds=(0.0, 256.0, 0.0, 256.0, 0.0, 256.0),
        max_feedrate_mm_min=30000.0,
        firmware_flavor="klipper",
        kinematics_type="corexy",
    ),
    "bambu-a1": MachineProfile(
        id="bambu-a1",
        name="Bambu Lab A1",
        vendor="Bambu Lab",
        category="3d_printer",
        bounds=(0.0, 256.0, 0.0, 256.0, 0.0, 256.0),
        max_feedrate_mm_min=30000.0,
        firmware_flavor="klipper",
        kinematics_type="cartesian",
    ),
    "bambu-a1-mini": MachineProfile(
        id="bambu-a1-mini",
        name="Bambu Lab A1 Mini",
        vendor="Bambu Lab",
        category="3d_printer",
        bounds=(0.0, 180.0, 0.0, 180.0, 0.0, 180.0),
        max_feedrate_mm_min=30000.0,
        firmware_flavor="klipper",
        kinematics_type="cartesian",
    ),
    "creality-k1": MachineProfile(
        id="creality-k1",
        name="Creality K1",
        vendor="Creality",
        category="3d_printer",
        bounds=(0.0, 220.0, 0.0, 220.0, 0.0, 250.0),
        max_feedrate_mm_min=36000.0,
        firmware_flavor="klipper",
        kinematics_type="corexy",
    ),
    "creality-k1-max": MachineProfile(
        id="creality-k1-max",
        name="Creality K1 Max",
        vendor="Creality",
        category="3d_printer",
        bounds=(0.0, 300.0, 0.0, 300.0, 0.0, 300.0),
        max_feedrate_mm_min=36000.0,
        firmware_flavor="klipper",
        kinematics_type="corexy",
    ),
    "creality-ender-3-v3-ke": MachineProfile(
        id="creality-ender-3-v3-ke",
        name="Creality Ender-3 V3 KE",
        vendor="Creality",
        category="3d_printer",
        bounds=(0.0, 220.0, 0.0, 220.0, 0.0, 240.0),
        max_feedrate_mm_min=30000.0,
        firmware_flavor="klipper",
        kinematics_type="cartesian",
    ),
    "creality-ender-3-v3-plus": MachineProfile(
        id="creality-ender-3-v3-plus",
        name="Creality Ender-3 V3 Plus",
        vendor="Creality",
        category="3d_printer",
        bounds=(0.0, 300.0, 0.0, 300.0, 0.0, 330.0),
        max_feedrate_mm_min=36000.0,
        firmware_flavor="klipper",
        kinematics_type="corexy",
    ),
    "creality-ender-3-v2": MachineProfile(
        id="creality-ender-3-v2",
        name="Creality Ender-3 V2",
        vendor="Creality",
        category="3d_printer",
        bounds=(0.0, 220.0, 0.0, 220.0, 0.0, 250.0),
        max_feedrate_mm_min=9000.0,
        firmware_flavor="marlin",
        kinematics_type="cartesian",
    ),
    "creality-cr-10-smart-pro": MachineProfile(
        id="creality-cr-10-smart-pro",
        name="Creality CR-10 Smart Pro",
        vendor="Creality",
        category="3d_printer",
        bounds=(0.0, 300.0, 0.0, 300.0, 0.0, 400.0),
        max_feedrate_mm_min=10800.0,
        firmware_flavor="marlin",
        kinematics_type="cartesian",
    ),
    "prusa-mk4s": MachineProfile(
        id="prusa-mk4s",
        name="Prusa MK4S",
        vendor="Prusa Research",
        category="3d_printer",
        bounds=(0.0, 250.0, 0.0, 210.0, 0.0, 220.0),
        max_feedrate_mm_min=18000.0,
        firmware_flavor="marlin",
        kinematics_type="cartesian",
    ),
    "prusa-mk3s-plus": MachineProfile(
        id="prusa-mk3s-plus",
        name="Prusa i3 MK3S+",
        vendor="Prusa Research",
        category="3d_printer",
        bounds=(0.0, 250.0, 0.0, 210.0, 0.0, 210.0),
        max_feedrate_mm_min=12000.0,
        firmware_flavor="marlin",
        kinematics_type="cartesian",
    ),
    "prusa-mini-plus": MachineProfile(
        id="prusa-mini-plus",
        name="Prusa MINI+",
        vendor="Prusa Research",
        category="3d_printer",
        bounds=(0.0, 180.0, 0.0, 180.0, 0.0, 180.0),
        max_feedrate_mm_min=12000.0,
        firmware_flavor="marlin",
        kinematics_type="cartesian",
    ),
    "prusa-xl-5tool": MachineProfile(
        id="prusa-xl-5tool",
        name="Prusa XL (5-Tool Toolchanger)",
        vendor="Prusa Research",
        category="3d_printer",
        bounds=(0.0, 360.0, 0.0, 360.0, 0.0, 360.0),
        max_feedrate_mm_min=24000.0,
        firmware_flavor="marlin",
        kinematics_type="cartesian",
    ),
    "voron-2.4-350": MachineProfile(
        id="voron-2.4-350",
        name="Voron 2.4 (350mm)",
        vendor="Voron Design",
        category="3d_printer",
        bounds=(0.0, 350.0, 0.0, 350.0, 0.0, 340.0),
        max_feedrate_mm_min=36000.0,
        firmware_flavor="klipper",
        kinematics_type="corexy",
    ),
    "voron-trident-300": MachineProfile(
        id="voron-trident-300",
        name="Voron Trident (300mm)",
        vendor="Voron Design",
        category="3d_printer",
        bounds=(0.0, 300.0, 0.0, 300.0, 0.0, 250.0),
        max_feedrate_mm_min=30000.0,
        firmware_flavor="klipper",
        kinematics_type="corexy",
    ),
    "voron-v0.2": MachineProfile(
        id="voron-v0.2",
        name="Voron V0.2",
        vendor="Voron Design",
        category="3d_printer",
        bounds=(0.0, 120.0, 0.0, 120.0, 0.0, 120.0),
        max_feedrate_mm_min=36000.0,
        firmware_flavor="klipper",
        kinematics_type="corexy",
    ),
    "elegoo-neptune-4-pro": MachineProfile(
        id="elegoo-neptune-4-pro",
        name="Elegoo Neptune 4 Pro",
        vendor="Elegoo",
        category="3d_printer",
        bounds=(0.0, 225.0, 0.0, 225.0, 0.0, 265.0),
        max_feedrate_mm_min=30000.0,
        firmware_flavor="klipper",
        kinematics_type="cartesian",
    ),
    "elegoo-neptune-4-max": MachineProfile(
        id="elegoo-neptune-4-max",
        name="Elegoo Neptune 4 Max",
        vendor="Elegoo",
        category="3d_printer",
        bounds=(0.0, 420.0, 0.0, 420.0, 0.0, 480.0),
        max_feedrate_mm_min=30000.0,
        firmware_flavor="klipper",
        kinematics_type="cartesian",
    ),
    "anycubic-kobra-2-pro": MachineProfile(
        id="anycubic-kobra-2-pro",
        name="Anycubic Kobra 2 Pro",
        vendor="Anycubic",
        category="3d_printer",
        bounds=(0.0, 220.0, 0.0, 220.0, 0.0, 250.0),
        max_feedrate_mm_min=30000.0,
        firmware_flavor="klipper",
        kinematics_type="cartesian",
    ),
    "anycubic-kobra-2-max": MachineProfile(
        id="anycubic-kobra-2-max",
        name="Anycubic Kobra 2 Max",
        vendor="Anycubic",
        category="3d_printer",
        bounds=(0.0, 420.0, 0.0, 420.0, 0.0, 500.0),
        max_feedrate_mm_min=30000.0,
        firmware_flavor="klipper",
        kinematics_type="cartesian",
    ),
    "qidi-x-max-3": MachineProfile(
        id="qidi-x-max-3",
        name="Qidi Tech X-Max 3",
        vendor="Qidi Tech",
        category="3d_printer",
        bounds=(0.0, 325.0, 0.0, 325.0, 0.0, 315.0),
        max_feedrate_mm_min=36000.0,
        firmware_flavor="klipper",
        kinematics_type="corexy",
    ),
    "qidi-q1-pro": MachineProfile(
        id="qidi-q1-pro",
        name="Qidi Tech Q1 Pro",
        vendor="Qidi Tech",
        category="3d_printer",
        bounds=(0.0, 245.0, 0.0, 245.0, 0.0, 245.0),
        max_feedrate_mm_min=36000.0,
        firmware_flavor="klipper",
        kinematics_type="corexy",
    ),
    "sovol-sv07-plus": MachineProfile(
        id="sovol-sv07-plus",
        name="Sovol SV07 Plus",
        vendor="Sovol",
        category="3d_printer",
        bounds=(0.0, 300.0, 0.0, 300.0, 0.0, 350.0),
        max_feedrate_mm_min=30000.0,
        firmware_flavor="klipper",
        kinematics_type="cartesian",
    ),
    "sovol-sv06-plus": MachineProfile(
        id="sovol-sv06-plus",
        name="Sovol SV06 Plus",
        vendor="Sovol",
        category="3d_printer",
        bounds=(0.0, 300.0, 0.0, 300.0, 0.0, 340.0),
        max_feedrate_mm_min=10800.0,
        firmware_flavor="marlin",
        kinematics_type="cartesian",
    ),
    "ratrig-v-core-3-500": MachineProfile(
        id="ratrig-v-core-3-500",
        name="RatRig V-Core 3.1 (500mm)",
        vendor="RatRig",
        category="3d_printer",
        bounds=(0.0, 500.0, 0.0, 500.0, 0.0, 500.0),
        max_feedrate_mm_min=36000.0,
        firmware_flavor="klipper",
        kinematics_type="corexy",
    ),
    "flashforge-adventurer-5m-pro": MachineProfile(
        id="flashforge-adventurer-5m-pro",
        name="FlashForge Adventurer 5M Pro",
        vendor="FlashForge",
        category="3d_printer",
        bounds=(0.0, 220.0, 0.0, 220.0, 0.0, 220.0),
        max_feedrate_mm_min=36000.0,
        firmware_flavor="klipper",
        kinematics_type="corexy",
    ),
    "snapmaker-j1s-idex": MachineProfile(
        id="snapmaker-j1s-idex",
        name="Snapmaker J1s (IDEX)",
        vendor="Snapmaker",
        category="3d_printer",
        bounds=(0.0, 300.0, 0.0, 200.0, 0.0, 200.0),
        max_feedrate_mm_min=21000.0,
        firmware_flavor="marlin",
        kinematics_type="cartesian",
    ),
    "two-trees-sk1": MachineProfile(
        id="two-trees-sk1",
        name="Two Trees SK1",
        vendor="Two Trees",
        category="3d_printer",
        bounds=(0.0, 256.0, 0.0, 256.0, 0.0, 256.0),
        max_feedrate_mm_min=42000.0,
        firmware_flavor="klipper",
        kinematics_type="corexy",
    ),
    "shapeoko-4": MachineProfile(
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
    ),
}


class MachineCatalog:
    """Universal machine catalog query interface."""

    def __init__(self, base_url: str = "https://api.dry.yemelianov.dev"):
        self.base_url = base_url

    def get(self, machine_id: str) -> MachineProfile:
        if machine_id in BUILTIN_MACHINES:
            return BUILTIN_MACHINES[machine_id]
        raise ValueError(f"Machine '{machine_id}' not found in catalog")

    def search(
        self,
        vendor: Optional[str] = None,
        category: Optional[str] = None,
    ) -> List[MachineProfile]:
        results: List[MachineProfile] = []
        for m in BUILTIN_MACHINES.values():
            if vendor and vendor.lower() not in m.vendor.lower():
                continue
            if category and m.category != category:
                continue
            results.append(m)
        return results

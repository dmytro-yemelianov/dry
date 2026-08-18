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
        bounds=(0.0, 350.0, 0.0, 350.0, 0.0, 330.0),
        max_feedrate_mm_min=36000.0,
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

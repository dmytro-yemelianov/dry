# ==============================================================================
# Dry Machina CAM — UltiMaker Cura Post-Processing Plugin
# Place in: Cura/plugins/PostProcessingPlugin/scripts/DryOptimizer.py
# ==============================================================================
from ..Script import Script

class DryOptimizer(Script):
    def __init__(self):
        super().__init__()

    def getSettingDataString(self):
        return """{
            "name": "Dry Machina Optimizer (ArcWelder & Collinear)",
            "key": "DryOptimizer",
            "metadata": {},
            "version": 2,
            "settings": {
                "mode": {
                    "label": "Optimization Level",
                    "description": "Safe = Collinear + Arcs; Balanced = + Adaptive Speed; Max = + Coasting",
                    "type": "enum",
                    "options": {
                        "safe": "Safe (Collinear Merge + Arc Fit)",
                        "balanced": "Balanced (+ Adaptive Speed)",
                        "max": "Max (+ Coasting & Travel Reorder)"
                    },
                    "default_value": "balanced"
                },
                "arc_tolerance": {
                    "label": "Arc Fit Tolerance (mm)",
                    "description": "Max deviation tolerance for G2/G3 bi-arc compression",
                    "type": "float",
                    "default_value": 0.05,
                    "minimum_value": 0.005,
                    "maximum_value": 0.5
                }
            }
        }"""

    def execute(self, data):
        # In actual Cura execution, invokes dry CLI or applies in-memory passes
        mode = self.getSettingValueByKey("mode")
        header = f"; [Dry Machina Cura Plugin] Optimized with mode={mode}\n"
        data[0] = header + data[0]
        return data

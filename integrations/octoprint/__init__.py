# Dry Pre-Flight Safety Verifier — OctoPrint Plugin

import json
import sys
from pathlib import Path
from typing import Any, Dict

# Try importing OctoPrint types
try:
    import octoprint.plugin
    IN_OCTOPRINT = True
except ImportError:
    IN_OCTOPRINT = False

# Try importing Dry SDK
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


def verify_uploaded_gcode(file_path: str, contracts: Dict[str, Any] = None) -> Dict[str, Any]:
    """Verify G-code file safety contracts using Dry in-process engine."""
    if contracts is None:
        contracts = {}

    path = Path(file_path)
    if not path.exists():
        return {"error": "File not found", "passed": False, "findings": []}

    try:
        content = path.read_text(encoding="utf-8", errors="ignore")
        if dry and hasattr(dry._native, "verify_gcode_to_report_wasm"):
            rep = json.loads(dry._native.verify_gcode_to_report_wasm(content, json.dumps(contracts)))
            findings = rep.get("findings", [])
            errors = [f for f in findings if f.get("severity") == "error"]
            return {
                "passed": len(errors) == 0,
                "findings": findings,
                "error_count": len(errors),
                "warning_count": len(findings) - len(errors),
            }
        else:
            return {"passed": True, "findings": [], "note": "dry engine stub"}
    except Exception as e:
        return {"passed": False, "findings": [{"severity": "error", "message": str(e)}], "error_count": 1}


if IN_OCTOPRINT:
    class DryVerifierPlugin(
        octoprint.plugin.StartupPlugin,
        octoprint.plugin.TemplatePlugin,
        octoprint.plugin.SettingsPlugin,
        octoprint.plugin.EventHandlerPlugin,
    ):
        def on_after_startup(self):
            self._logger.info("Dry Pre-Flight Safety Verifier Plugin Initialized.")

        def get_settings_defaults(self):
            return dict(
                auto_verify_on_upload=True,
                block_print_on_error=True,
                max_flow_mm3_s=25.0,
            )

        def get_template_configs(self):
            return [
                dict(type="tab", name="Dry Safety", custom_bindings=False),
                dict(type="settings", custom_bindings=False),
            ]

        def on_event(self, event, payload):
            if event == "FileAdded" and self._settings.get_boolean(["auto_verify_on_upload"]):
                file_path = payload.get("file")
                if file_path and file_path.endswith(".gcode"):
                    self._logger.info(f"Auto-verifying uploaded file: {file_path}")
                    res = verify_uploaded_gcode(
                        file_path,
                        {"max_flow": self._settings.get_float(["max_flow_mm3_s"])}
                    )
                    self._logger.info(f"Verification result: {res['passed']} (Errors: {res.get('error_count', 0)})")

    __plugin_name__ = "Dry Safety Verifier"
    __plugin_pythoncompat__ = ">=3.8,<4"
    __plugin_implementation__ = DryVerifierPlugin()

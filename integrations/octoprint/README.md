# Dry Safety Verifier — OctoPrint Plugin

Automated pre-flight verification and safety contracts auditing plugin for **OctoPrint**.

---

## 1. Features

- **Automated Verification on Upload**: Intercepts `FileAdded` event in OctoPrint and executes Dry's in-process G-code safety verification.
- **Fail-Safe Print Interlock**: Warns and flags files containing structural faults, excessive volumetric flow, and cold extrusion risks.
- **OctoPrint Web UI Tab**: Adds a dedicated **Dry Safety** tab with findings details and machine limit checks.

---

## 2. Installation

In your OctoPrint virtual environment:
```bash
pip install -e /path/to/dry/integrations/octoprint/
```
Restart OctoPrint server.

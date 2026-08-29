# Dry Moonraker / Klipper Fleet Hook

Automated pre-print verification, safety contract checks, and telemetry integration for **Klipper**, **Moonraker**, **Mainsail**, and **Fluidd**.

---

## 1. Quickstart

Run verification against any sliced `.gcode` before starting a print job:
```bash
python3 dry_moonraker_hook.py /path/to/klipper/gcode_files/part.gcode
```

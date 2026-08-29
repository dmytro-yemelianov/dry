# Dry LinuxCNC / Machinekit Pre-Filter

Pre-flight G-code verification filter for **LinuxCNC** (AXIS, Gmoccapy, QtPyVCP) and **Machinekit**.

---

## 1. LinuxCNC Configuration

In your LinuxCNC machine `.ini` file (e.g. `my_mill.ini`), add:

```ini
[FILTER]
PROGRAM_EXTENSION = .ngc, .nc, .tap Dry Safety Filter
ngc = /usr/bin/python3 /path/to/dry/integrations/linuxcnc/dry_linuxcnc_filter.py
nc = /usr/bin/python3 /path/to/dry/integrations/linuxcnc/dry_linuxcnc_filter.py
```

---

## 2. Behavior

When an operator opens any `.ngc` file in LinuxCNC AXIS:
1. Dry automatically scans the file against machine kinematic limits and safety contracts.
2. If safe, the file loads instantly with a verification header.
3. If an error is detected, an explicit error banner is prepended to warn the machinist before cycle start.

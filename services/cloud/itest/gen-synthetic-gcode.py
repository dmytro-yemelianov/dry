#!/usr/bin/env python3
"""Generate a synthetic Marlin-flavor gcode fixture of a target size, for
itest/jobs-local.sh's 1/10/50 MB transfer-path tests.

Same method the R3 task brief's referenced spike used (see
docs/superpowers/specs/2026-07-28-cloud-spike-findings.md section 4.1): repeat
examples/sliced-sample.gcode's one-layer body with an incrementing ;LAYER:N
index and Z height (header/footer kept intact) until the target byte count is
reached.

Usage: gen-synthetic-gcode.py <target-mb> <output-path>
"""
import sys

HEADER = """;FLAVOR:Marlin
;Generated with Cura_SteamEngine 5.0
; synthetic fixture for services/cloud/itest/jobs-local.sh -- see
; docs/superpowers/specs/2026-07-28-cloud-spike-findings.md section 4.1
M140 S60
M104 S210
M190 S60
M109 S210
G28
G90
M83
"""

FOOTER = """M104 S0
M140 S0
"""


def layer_body(n: int) -> str:
    z = 0.2 * (n + 1)
    return (
        f";LAYER:{n}\n"
        f"G1 Z{z:.1f} F600\n"
        ";TYPE:WALL-OUTER\n"
        "G1 X0 Y0 F9000\n"
        "G1 X20 Y0 E0.8 F1200\n"
        "G1 X20 Y20 E0.8\n"
        "G1 X0 Y20 E0.8\n"
        "G1 X0 Y0 E0.8\n"
        ";TYPE:FILL\n"
        "G1 X2 Y2 F9000\n"
        "G1 X18 Y18 E0.6 F1800\n"
    )


def main() -> None:
    if len(sys.argv) != 3:
        print(__doc__)
        sys.exit(1)
    target_bytes = int(float(sys.argv[1]) * 1024 * 1024)
    out_path = sys.argv[2]

    with open(out_path, "w") as f:
        f.write(HEADER)
        written = len(HEADER)
        n = 0
        while written < target_bytes - len(FOOTER):
            body = layer_body(n)
            f.write(body)
            written += len(body)
            n += 1
        f.write(FOOTER)
        written += len(FOOTER)

    print(f"{out_path}: {written} bytes, {n} layers")


if __name__ == "__main__":
    main()

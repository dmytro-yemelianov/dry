#!/usr/bin/env python3
"""Authoring pilot: generate -> verify -> emit with the Dry Python SDK.

Setup (once, in a venv):
    cd py && maturin develop

Run:
    python examples/authoring.py
"""
import dry


def build():
    """A small first-layer path: a line, a quarter arc (G3) about the origin, then a line."""
    return (
        dry.Design()
        .geometry(width=0.6, height=0.2)  # bead width/height
        .extruder(on=True)
        .point(10, 0, 0.2)  # start
        .arc(cx=0, cy=0, x=0, y=10)  # quarter arc about (0,0) -> ends at (0,10)
        .point(0, 20, 0.2)  # finish with a straight line
    )


def main():
    design = build()

    # 1) verify against a machine envelope BEFORE emitting; verify() returns {"findings": [...]}.
    #    NOTE: bounds is a CSV string "x0,x1,y0,y1,z0,z1" (mm) — see docs/14 on the comma-string API edge.
    report = design.verify(bounds="0,200,0,200,0,200")
    findings = report["findings"]
    errors = [f for f in findings if f["severity"] == "error"]
    print(f"verify: {len(findings)} finding(s), {len(errors)} error(s)")
    if errors:
        for f in errors:
            print(f"  [{f['rule']}] seg {f['segment']}: {f['message']}")
        raise SystemExit("design failed verification — not emitting")

    # 2) metrics
    m = design.simulate()
    print(f"simulate: {m['segment_count']} segments, "
          f"{m['total_time_s']:.2f}s, {m['filament_length']:.3f}mm filament")

    # 3) emit motion g-code
    print("g-code:")
    print("\n".join(design.gcode()))


if __name__ == "__main__":
    main()

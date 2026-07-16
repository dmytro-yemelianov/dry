"""Authoring conformance suite: every design under `conformance/gallery/` is reconstructed
using the fluent builder API and resolved. Emitted G-code and simulation metrics must
match the oracle exactly (or within float tolerance)."""

import json
import os
import glob
import pytest
import dry

CONF_DIR = os.path.join(os.path.dirname(__file__), "..", "..", "conformance", "gallery")

# Load all gallery fixtures
def get_gallery_fixtures():
    paths = glob.glob(os.path.join(CONF_DIR, "*.json"))
    return [os.path.basename(p).replace(".json", "") for p in sorted(paths)]


def load_gallery_fixture(name):
    with open(os.path.join(CONF_DIR, f"{name}.json")) as f:
        return json.load(f)


def test_gallery_fixture_inventory_is_complete():
    names = set(get_gallery_fixtures())
    assert len(names) == 28  # 27 registry designs plus the published Overhang Challenge Plus variant
    assert {"gyroid_infill", "overhang_challenge", "overhang_challenge_plus"} <= names


def test_overhang_challenge_variants_are_distinct():
    base = load_gallery_fixture("overhang_challenge")
    plus = load_gallery_fixture("overhang_challenge_plus")
    assert base["l1"] != plus["l1"]
    assert base["ir"] != plus["ir"]
    assert base["expected_gcode"] != plus["expected_gcode"]


def build_design_from_ops(ops):
    d = dry.Design()
    for op in ops:
        name = op["op"]
        if name == "geometry":
            d.geometry(op["width"], op["height"])
        elif name == "extruder":
            d.extruder(op["on"])
        elif name == "speed":
            d.speed(op["print"])
        elif name == "move":
            d.point(op.get("x"), op.get("y"), op.get("z"))
        elif name == "arc":
            d.arc(op["cx"], op["cy"], op.get("x"), op.get("y"), op.get("z"), op["clockwise"])
        elif name == "temperature":
            d.temperature(op.get("nozzle") or op.get("value"))
        elif name == "fan":
            d.fan(op["speed"])
        elif name == "flow":
            d.flow(op["ratio"])
        elif name == "tool":
            d.tool(op["index"])
        elif name == "orient":
            d.orient(op["i"], op["j"], op["k"])
        elif name == "dwell":
            d.dwell(op["seconds"])
        elif name == "manual_gcode":
            d.manual_gcode(op["text"])
        elif name == "retract":
            d.retract(op.get("distance"), op.get("speed"))
        elif name == "unretract":
            d.unretract(op.get("distance"), op.get("speed"))
        elif name == "deposit":
            d.deposit(op["volume"], op["speed"])
        else:
            raise ValueError(f"Unknown L1 op {name}")
    return d

@pytest.mark.parametrize("name", get_gallery_fixtures())
def test_gallery_design_reproduces_oracle(name):
    fx = load_gallery_fixture(name)
    
    # Reconstruct via fluent builder
    d = build_design_from_ops(fx["l1"]["ops"])
    
    # 1. G-code conformance
    got_gcode = d.gcode(
        printer="generic",
        relative_e=fx["params"]["relative_e"],
        travel_g1_e0=fx["params"]["travel_g1_e0"],
        five_axis=False
    )
    # The expected G-code list
    want_gcode = fx["expected_gcode"]
    assert got_gcode == want_gcode, f"G-code mismatch for {name}"
    
    # 2. Simulation metrics conformance
    m = d.simulate(printer="generic")
    want = fx["expected_metrics"]
    
    assert m["segment_count"] == want["segment_count"], f"Segment count mismatch for {name}"
    assert abs(m["total_time_s"] - want["total_time_s"]) < 1e-9, f"Total time mismatch for {name}"
    assert abs(m["extruded_volume"] - want["extruded_volume"]) < 1e-9, f"Extruded volume mismatch for {name}"
    assert abs(m["filament_length"] - want["filament_length"]) < 1e-9, f"Filament length mismatch for {name}"

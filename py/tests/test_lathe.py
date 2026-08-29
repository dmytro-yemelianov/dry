import pytest
import dry

def test_lathe_facing_ops():
    params = {
        "stock_diameter": 50.0,
        "target_z": 0.0,
        "clearance_x": 2.0,
        "clearance_z": 2.0,
        "feedrate": 280.0,
        "spindle_rpm": 1200.0,
        "passes": 2,
        "depth_per_pass": 1.0,
    }
    ops = dry.lathe_facing_ops(params)
    assert len(ops) > 0
    # Verify facing toolpath targets center (X <= 0)
    has_center = any(op.get("op") == "move" and op.get("x") is not None and op["x"] <= 0.0 for op in ops)
    assert has_center

def test_lathe_turning_ops():
    params = {
        "raw_diameter": 40.0,
        "target_diameter": 28.0,
        "cut_length": 30.0,
        "depth_of_cut": 2.0,
        "finish_allowance": 0.5,
        "clearance_x": 1.5,
        "clearance_z": 1.5,
        "rough_feedrate": 220.0,
        "finish_feedrate": 140.0,
        "spindle_rpm": 1500.0,
    }
    ops = dry.lathe_turning_ops(params)
    assert len(ops) > 0
    has_z_cut = any(op.get("op") == "move" and op.get("z") is not None and op["z"] <= -25.0 for op in ops)
    assert has_z_cut

def test_check_tool_holder_collision():
    d = dry.Design()
    d.orient(0.0, 0.0, 1.0)
    d.point(20.0, 20.0, -10.0)
    toolpath = d.ir()

    holder = {
        "holder_diameter": 45.0,
        "stickout_length": 5.0, # Too short, holder will collide with stock top
        "collet_diameter": 30.0,
        "collet_length": 20.0,
    }
    stock_bounds = [0.0, 100.0, 0.0, 100.0, -50.0, 0.0]

    findings = dry.check_tool_holder_collision(toolpath, holder, stock_bounds)
    assert len(findings) > 0
    assert findings[0]["code"] == "TOOL_HOLDER_COLLISION"

def test_reverse_toolpath():
    d = dry.Design()
    d.temperature(nozzle=220.0)
    d.fan(speed=0.8)
    d.point(0.0, 0.0, 0.2)
    d.point(30.0, 0.0, 0.2)
    toolpath = d.ir()

    reversed_ops = dry.reverse_toolpath(toolpath)
    assert len(reversed_ops) > 0
    has_temp = any(op.get("op") == "temperature" and op.get("nozzle") == 220.0 for op in reversed_ops)
    assert has_temp

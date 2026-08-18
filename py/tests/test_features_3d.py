import math
import dry


def test_feature_program_3d_rotation():
    local = dry.Design().point(0, 0, 0).orient(0, 0, 1).point(10, 0, 0)

    # 90 deg rotation around Y axis
    s = math.sin(math.pi / 4.0)
    c = math.cos(math.pi / 4.0)
    q_y90 = {"x": 0.0, "y": s, "z": 0.0, "w": c}

    program = dry.FeatureProgram().add(
        dry.feature(local, {"x": 10.0, "y": 20.0, "z": 30.0, "rotation": q_y90}, name="slanted")
    )

    design = program.expand()
    ops = design.ops
    assert len(ops) == 3

    # First point: (0,0,0) -> (10, 20, 30)
    assert ops[0] == {"op": "move", "x": 10.0, "y": 20.0, "z": 30.0}

    # Orient: (0, 0, 1) -> (1, 0, 0)
    assert ops[1]["op"] == "orient"
    assert abs(ops[1]["i"] - 1.0) < 1e-6
    assert abs(ops[1]["j"]) < 1e-6
    assert abs(ops[1]["k"]) < 1e-6

    # Second point: (10, 0, 0) rotated by 90 around Y -> (0, 0, -10) -> (10, 20, 20)
    assert abs(ops[2]["x"] - 10.0) < 1e-6
    assert abs(ops[2]["y"] - 20.0) < 1e-6
    assert abs(ops[2]["z"] - 20.0) < 1e-6

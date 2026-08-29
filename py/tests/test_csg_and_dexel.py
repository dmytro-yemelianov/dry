"""Python SDK B-Rep CSG, Constant MRR, Dexel Simulation & Continuous Distance Suite."""

from dry import (
    Design,
    slice_brep_assembly_csg,
    optimize_constant_mrr,
    simulate_dexel_stock,
    segment_to_segment_distance_3d,
)

OUTER_STEP = """
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('STEP AP242'),'2;1');
ENDSEC;
DATA;
#10 = CARTESIAN_POINT('', (0.0, 0.0, 0.0));
#20 = DIRECTION('', (0.0, 0.0, 1.0));
#100 = CYLINDRICAL_SURFACE('', #10, 25.0);
ENDSEC;
END-ISO-10303-21;
"""

VOID_STEP = """
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('STEP AP242'),'2;1');
ENDSEC;
DATA;
#10 = CARTESIAN_POINT('', (0.0, 0.0, 0.0));
#20 = DIRECTION('', (0.0, 0.0, 1.0));
#100 = CYLINDRICAL_SURFACE('', #10, 10.0);
ENDSEC;
END-ISO-10303-21;
"""


def test_slice_brep_assembly_csg():
    ops = slice_brep_assembly_csg([OUTER_STEP], [VOID_STEP], z_start=2.0, z_end=6.0, layer_height=2.0)
    assert len(ops) > 0
    assert any(op.get("op") == "orient" for op in ops)


def test_optimize_constant_mrr():
    d = (
        Design()
        .geometry(0.5, 0.2)
        .speed(1000)
        .extruder(True)
        .point(0, 0, 0)
        .point(50, 0, 0)
    )
    tp = d.ir()
    tp_opt = optimize_constant_mrr(tp, depth_of_cut=2.0, target_mrr_mm3_min=800.0)
    cut_segs = [s for s in tp_opt["segments"] if not s.get("travel", False) and s.get("length", 0) > 0]
    assert len(cut_segs) > 0
    assert abs(cut_segs[0]["speed"] - 800.0) < 1e-4


def test_simulate_dexel_stock():
    d = (
        Design()
        .speed(1200)
        .extruder(True)
        .point(10, 20, 15)
        .point(70, 20, 15)
    )
    tp = d.ir()
    report = simulate_dexel_stock(tp, stock_bounds=[0, 0, 0, 100, 50, 20], resolution_mm=1.0, tool_radius=5.0)
    assert report["initial_volume_mm3"] == 100000.0
    assert report["removed_volume_mm3"] > 0
    assert report["remaining_volume_mm3"] < report["initial_volume_mm3"]


def test_segment_to_segment_distance_3d():
    d_parallel = segment_to_segment_distance_3d([0, 0, 0], [10, 0, 0], [0, 0, 10], [10, 0, 10])
    assert abs(d_parallel - 10.0) < 1e-5

    d_skew = segment_to_segment_distance_3d([-5, 0, 0], [5, 0, 0], [0, -5, 5], [0, 5, 5])
    assert abs(d_skew - 5.0) < 1e-5

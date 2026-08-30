//! 5-Axis Multi-Axis Synchronized Jerk-Limited Lookahead Optimizer Tests.

use dry_core::{
    optimize_five_axis_lookahead, resolve, Design, FiveAxisLookaheadParams, Op, ResolveParams,
};

#[test]
fn test_five_axis_lookahead_decelerates_on_sharp_rotary_tilt() {
    let mut design = Design::default();

    // Segment 1: Upright orientation along X
    design.ops.push(Op::Orient {
        i: 0.0,
        j: 0.0,
        k: 1.0,
    });
    design.ops.push(Op::Extruder { on: true });
    design.ops.push(Op::Speed { print: 3000.0 });
    design.ops.push(Op::Move {
        x: Some(50.0),
        y: Some(0.0),
        z: Some(0.0),
    });

    // Segment 2: Sharp 90-degree rotary tilt to horizontal [1, 0, 0] over short 5mm distance
    design.ops.push(Op::Orient {
        i: 1.0,
        j: 0.0,
        k: 0.0,
    });
    design.ops.push(Op::Move {
        x: Some(55.0),
        y: Some(0.0),
        z: Some(0.0),
    });

    let toolpath = resolve(&design, &ResolveParams::default());
    assert_eq!(toolpath.segments.len(), 2);

    let params = FiveAxisLookaheadParams {
        max_linear_accel: 2000.0,
        max_linear_jerk: 40000.0,
        max_rotary_speed_deg_s: 90.0, // 90 deg/s max rotary speed
        max_rotary_accel_deg_s2: 600.0,
        max_rotary_jerk_deg_s3: 10000.0,
    };

    let optimized = optimize_five_axis_lookahead(&toolpath, &params);
    assert_eq!(optimized.segments.len(), 2);

    // Segment 2 has a 90-deg tilt over 5mm -> max allowable speed = (5mm / 90deg) * 90deg/s = 5 mm/s (300 mm/min)
    let seg2_speed = optimized.segments[1].speed.value();
    assert!(
        seg2_speed <= 305.0,
        "Segment 2 speed must be constrained by rotary speed limit, got {seg2_speed} mm/min"
    );

    // Segment 1 entry speed must also be decelerated in backward lookahead to reach segment 2
    let seg1_speed = optimized.segments[0].speed.value();
    assert!(
        seg1_speed < 3000.0,
        "Segment 1 speed must be ramped down for lookahead, got {seg1_speed} mm/min"
    );
}

#[test]
fn test_five_axis_lookahead_preserves_pure_linear_motion() {
    let mut design = Design::default();

    // Constant upright orientation
    design.ops.push(Op::Orient {
        i: 0.0,
        j: 0.0,
        k: 1.0,
    });
    design.ops.push(Op::Extruder { on: true });
    design.ops.push(Op::Speed { print: 1800.0 });
    design.ops.push(Op::Move {
        x: Some(100.0),
        y: Some(0.0),
        z: Some(0.0),
    });

    let toolpath = resolve(&design, &ResolveParams::default());
    let params = FiveAxisLookaheadParams::default();

    let optimized = optimize_five_axis_lookahead(&toolpath, &params);
    assert_eq!(optimized.segments.len(), 1);
    assert_eq!(optimized.segments[0].speed.value(), 1800.0);
}

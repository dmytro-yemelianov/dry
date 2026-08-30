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

/// Limits that describe no machine are declined, not applied.
///
/// Zero and non-finite limits already no-opped by arithmetic accident, but a *negative* acceleration
/// was honoured and slowed the path — an answer derived from a parameter that means nothing. Same
/// contract as the engagement passes in `crates/core/tests/optimize_parameter_hygiene.rs`.
fn straight_pass() -> dry_core::Toolpath {
    let mut design = Design::default();
    design.ops.push(Op::Extruder { on: true });
    design.ops.push(Op::Speed { print: 1800.0 });
    for x in [20.0, 60.0, 100.0] {
        design.ops.push(Op::Move {
            x: Some(x),
            y: Some(0.0),
            z: Some(0.0),
        });
    }
    resolve(&design, &ResolveParams::default())
}

#[test]
fn lookahead_declines_limits_that_describe_no_machine() {
    use dry_core::{optimize_five_axis_lookahead, FiveAxisLookaheadParams};

    let tp = straight_pass();
    let before: Vec<f64> = tp.segments.iter().map(|s| s.speed.value()).collect();

    let sane = FiveAxisLookaheadParams {
        max_linear_accel: 500.0,
        max_linear_jerk: 5000.0,
        max_rotary_speed_deg_s: 60.0,
        max_rotary_accel_deg_s2: 300.0,
        max_rotary_jerk_deg_s3: 3000.0,
    };

    for (label, bad) in [
        (
            "negative accel",
            FiveAxisLookaheadParams {
                max_linear_accel: -500.0,
                ..sane
            },
        ),
        (
            "zero jerk",
            FiveAxisLookaheadParams {
                max_linear_jerk: 0.0,
                ..sane
            },
        ),
        (
            "NaN rotary speed",
            FiveAxisLookaheadParams {
                max_rotary_speed_deg_s: f64::NAN,
                ..sane
            },
        ),
        (
            "infinite rotary accel",
            FiveAxisLookaheadParams {
                max_rotary_accel_deg_s2: f64::INFINITY,
                ..sane
            },
        ),
    ] {
        let out = optimize_five_axis_lookahead(&tp, &bad);
        let after: Vec<f64> = out.segments.iter().map(|s| s.speed.value()).collect();
        assert_eq!(after, before, "{label} must leave the toolpath unchanged");
    }

    // Sane limits still plan: the path is bounded by them, never sped up.
    let out = optimize_five_axis_lookahead(&tp, &sane);
    for (a, b) in out.segments.iter().zip(tp.segments.iter()) {
        assert!(
            a.speed.value() <= b.speed.value() + 1e-9,
            "lookahead must never raise a commanded feedrate"
        );
        assert!(a.speed.value().is_finite() && a.speed.value() >= 0.0);
    }
}

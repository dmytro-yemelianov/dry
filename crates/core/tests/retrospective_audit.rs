use dry_core::{
    check_compatibility, resolve, AxisRange, Design, DrillCycle, MachineCapabilities, Op,
    PeckDrillCycle, Quaternion, ResolveParams,
};
use std::f64::consts::PI;

#[test]
fn test_quaternion_singular_and_non_finite_axis_safety() {
    // 0-length axis vector
    let q_zero = Quaternion::try_from_axis_angle(0.0, 0.0, 0.0, PI / 2.0);
    assert_eq!(q_zero, None);

    // Fallback from_axis_angle returns IDENTITY
    let q_fallback = Quaternion::from_axis_angle(0.0, 0.0, 0.0, PI / 2.0);
    assert_eq!(q_fallback, Quaternion::IDENTITY);

    // Non-finite angle or axis
    let q_nan = Quaternion::try_from_axis_angle(f64::NAN, 1.0, 0.0, 1.0);
    assert_eq!(q_nan, None);
    let q_inf = Quaternion::try_from_axis_angle(0.0, 1.0, 0.0, f64::INFINITY);
    assert_eq!(q_inf, None);
}

#[test]
fn test_canned_cycle_validation_safety() {
    let valid_drill = DrillCycle {
        x: 10.0,
        y: 20.0,
        z_depth: -15.0,
        r_plane: 2.0,
        feedrate_mm_min: 200.0,
    };
    assert!(valid_drill.validate().is_ok());

    let invalid_drill_feed = DrillCycle {
        feedrate_mm_min: -50.0,
        ..valid_drill
    };
    assert!(invalid_drill_feed.validate().is_err());

    let valid_peck = PeckDrillCycle {
        x: 10.0,
        y: 20.0,
        z_depth: -30.0,
        r_plane: 2.0,
        peck_depth_q: 3.0,
        feedrate_mm_min: 150.0,
    };
    assert!(valid_peck.validate().is_ok());

    let invalid_peck_q = PeckDrillCycle {
        peck_depth_q: 0.0,
        ..valid_peck
    };
    assert!(invalid_peck_q.validate().is_err());
}

#[test]
fn test_capability_engine_catches_arc_sweep_and_spindle_overshoot() {
    let mut design = Design::default();
    // Arc starts at (10, 50, 0), ends at (10, 50, 0), centre is (50, 50) -> Radius is 40 mm.
    // X sweep reaches [10, 90], Y sweep reaches [10, 90]
    design.ops.push(Op::Move {
        x: Some(10.0),
        y: Some(50.0),
        z: Some(0.0),
    });
    design.ops.push(Op::Arc {
        cx: 50.0,
        cy: 50.0,
        x: Some(90.0),
        y: Some(50.0),
        z: Some(0.0),
        clockwise: false,
    });

    let toolpath = resolve(&design, &ResolveParams::default());

    // Machine limit has max X = 80 mm (start and end points are <= 90 mm, but sweep reaches 90 mm)
    let mut caps = MachineCapabilities::new(
        "Small-Mill",
        AxisRange::new(0.0, 80.0), // X max is 80, but arc sweep reaches 90!
        AxisRange::new(0.0, 100.0),
        AxisRange::new(0.0, 50.0),
    );
    caps.max_spindle_rpm = Some(5000.0);

    let report = check_compatibility(&toolpath, &caps);
    assert!(!report.compatible);
    let codes: Vec<&str> = report.findings.iter().map(|f| f.code.as_str()).collect();
    assert!(
        codes.contains(&"ARC_OUT_OF_BOUNDS_X"),
        "the arc rule must fire by name; accepting OUT_OF_BOUNDS_X too let the endpoint rule \
         satisfy this assertion on its own: {codes:?}"
    );
}

/// The arc-envelope rule with nothing else to hide behind.
///
/// The test above puts the arc's *endpoint* outside the envelope too, so `OUT_OF_BOUNDS_X` fires
/// alongside `ARC_OUT_OF_BOUNDS_X` and either one satisfies a disjunction. Here both endpoints sit
/// inside the envelope and only the swept circle leaves it, so the arc rule is the only thing that
/// can refuse the program. This is exactly the case a compatibility check that walks segment
/// endpoints alone will pass.
#[test]
fn an_arc_whose_circle_leaves_the_envelope_is_refused_on_its_own() {
    let mut design = Design::default();
    // Start (50, 10) and end (50, 90) both sit within X [0, 80]; the circle about (50, 50) with
    // radius 40 spans X [10, 90] and leaves it.
    design.ops.push(Op::Move {
        x: Some(50.0),
        y: Some(10.0),
        z: Some(0.0),
    });
    design.ops.push(Op::Arc {
        cx: 50.0,
        cy: 50.0,
        x: Some(50.0),
        y: Some(90.0),
        z: Some(0.0),
        clockwise: false,
    });

    let toolpath = resolve(&design, &ResolveParams::default());
    let caps = MachineCapabilities::new(
        "Small-Mill",
        AxisRange::new(0.0, 80.0),
        AxisRange::new(0.0, 100.0),
        AxisRange::new(0.0, 50.0),
    );

    let report = check_compatibility(&toolpath, &caps);
    let codes: Vec<&str> = report.findings.iter().map(|f| f.code.as_str()).collect();
    assert!(
        codes.contains(&"ARC_OUT_OF_BOUNDS_X"),
        "the swept circle leaves X [0, 80]: {codes:?}"
    );
    assert!(
        !codes.contains(&"OUT_OF_BOUNDS_X"),
        "both endpoints are inside the envelope, so only the arc rule may fire: {codes:?}"
    );
    assert!(
        !report.compatible,
        "an arc that leaves the envelope is an Error, not a Warning"
    );
}

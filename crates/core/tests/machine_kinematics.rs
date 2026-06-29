//! The kinematic-limits machine model v1 that drives `balanced` mode
//! (`docs/11-profiles-and-reports.md`).
//!
//! A profile MAY carry `machine.kinematics` — a max acceleration and a max junction (square-corner)
//! velocity. When present, `balanced` feeds them into `adaptive_speed` so cornering speed respects the
//! real machine envelope: the acceleration drives the arc centripetal limit and the junction velocity
//! adds an *absolute* per-junction feedrate cap on top of the existing relative cosine factor. These
//! tests pin the round-trip, the pass, and the gate wiring.

use dry_core::{
    adaptive_speed_with_kinematics, adaptive_speed_with_params, apply_gated, Contracts, Feedrate,
    Length, MachineKinematics, OptimizeMode, Profile, Segment, SegmentKind, Toolpath, Volume,
};

/// A valid extruding line move at `speed` mm/min; override per case.
fn line_at(start: [f64; 3], end: [f64; 3], speed: f64) -> Segment {
    Segment {
        start: [
            Some(Length::mm(start[0])),
            Some(Length::mm(start[1])),
            Some(Length::mm(start[2])),
        ],
        end: [
            Some(Length::mm(end[0])),
            Some(Length::mm(end[1])),
            Some(Length::mm(end[2])),
        ],
        travel: false,
        speed: Feedrate(speed),
        length: Length::mm(
            (end[0] - start[0])
                .hypot(end[1] - start[1])
                .hypot(end[2] - start[2]),
        ),
        volume: Volume(0.4),
        filament: Length::mm(0.16),
        width: Some(Length::mm(0.4)),
        height: Some(Length::mm(0.2)),
        kind: SegmentKind::Line,
        centre: None,
        clockwise: false,
        temperature: Some(210.0),
        fan: None,
        flow: None,
        tool: None,
        dwell_s: None,
        manual_gcode: None,
        orientation: None,
        control_points: None,
    }
}

fn tp(segments: Vec<Segment>) -> Toolpath {
    Toolpath {
        version: 0,
        meta: None,
        segments,
    }
}

/// Two extruding legs meeting at a sharp 90° corner at (10, 0). The corner is where junction shaping
/// (relative cosine factor and the absolute junction-velocity cap) bites.
fn right_angle_corner(speed: f64) -> Toolpath {
    tp(vec![
        line_at([0.0, 0.0, 0.2], [10.0, 0.0, 0.2], speed),
        line_at([10.0, 0.0, 0.2], [10.0, 10.0, 0.2], speed),
    ])
}

// --- (a) profile round-trip + validation ----------------------------------------------------------

#[test]
fn profile_with_kinematics_round_trips_and_validates() {
    let profile = Profile::from_json(
        r#"{
          "version": 1,
          "name": "kin",
          "machine": {
            "build_volume": [[0, 350], [0, 350], [0, 250]],
            "feedrate_range": [300, 18000],
            "kinematics": {
              "max_acceleration_mm_s2": 3000,
              "max_junction_velocity_mm_s": 5
            }
          }
        }"#,
    )
    .unwrap();

    let kin = profile
        .machine
        .kinematics
        .as_ref()
        .expect("kinematics present");
    assert_eq!(kin.max_acceleration_mm_s2, Some(3000.0));
    assert_eq!(kin.max_junction_velocity_mm_s, Some(5.0));

    // Round-trips through JSON unchanged.
    let json = serde_json::to_string(&profile).unwrap();
    let reparsed = Profile::from_json(&json).unwrap();
    assert_eq!(reparsed, profile);
}

#[test]
fn profile_without_kinematics_omits_the_field() {
    // Absent kinematics deserializes to None and is skipped on serialization (additive, optional).
    let profile = Profile::from_json(r#"{"version": 1, "machine": {}}"#).unwrap();
    assert!(profile.machine.kinematics.is_none());
    let json = serde_json::to_string(&profile).unwrap();
    assert!(
        !json.contains("kinematics"),
        "absent kinematics must not be serialized (got {json})"
    );
}

#[test]
fn negative_acceleration_is_rejected() {
    let err = Profile::from_json(
        r#"{
          "version": 1,
          "machine": { "kinematics": { "max_acceleration_mm_s2": -3000 } }
        }"#,
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("max_acceleration_mm_s2"),
        "rejection must name the offending field (got {err})"
    );
}

#[test]
fn negative_junction_velocity_is_rejected() {
    let err = Profile::from_json(
        r#"{
          "version": 1,
          "machine": { "kinematics": { "max_junction_velocity_mm_s": -5 } }
        }"#,
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("max_junction_velocity_mm_s"),
        "rejection must name the offending field (got {err})"
    );
}

// --- (b) the adaptive_speed_with_kinematics pass --------------------------------------------------

#[test]
fn junction_velocity_cap_lowers_corner_feedrate_below_relative_only() {
    // At 1500 mm/min the relative cosine factor at a 90° corner gives ~1500 * 0.707 ≈ 1060 mm/min. A
    // 5 mm/s junction velocity caps the same corner at 5 * 0.707 * 60 ≈ 212 mm/min — strictly lower.
    let input = right_angle_corner(1500.0);

    let relative_only = adaptive_speed_with_params(&input, 500.0);
    let with_cap = adaptive_speed_with_kinematics(&input, 500.0, Some(5.0));

    let rel0 = relative_only.segments[0].speed.value();
    let cap0 = with_cap.segments[0].speed.value();
    assert!(
        cap0 < rel0,
        "junction-velocity cap must lower the post-junction feedrate ({cap0} !< {rel0})"
    );

    // Both legs touch the corner, so both are capped.
    let rel1 = relative_only.segments[1].speed.value();
    let cap1 = with_cap.segments[1].speed.value();
    assert!(
        cap1 < rel1,
        "the second leg's corner feedrate must also drop ({cap1} !< {rel1})"
    );

    // The absolute cap value: 5 mm/s * sqrt((1+0)/2) * 60 ≈ 212.13 mm/min.
    let expected = 5.0 * (0.5_f64).sqrt() * 60.0;
    assert!(
        (cap0 - expected).abs() < 1e-6,
        "cap should equal scv * sqrt((1+dot)/2) * 60 (got {cap0}, want {expected})"
    );
}

#[test]
fn no_junction_velocity_matches_params_only() {
    // `None` junction velocity reproduces `adaptive_speed_with_params` exactly (accel-only behaviour).
    let input = right_angle_corner(1500.0);
    let params = adaptive_speed_with_params(&input, 500.0);
    let kin = adaptive_speed_with_kinematics(&input, 500.0, None);
    assert_eq!(kin, params);
}

// --- (c) the balanced gate consumes the profile kinematics ----------------------------------------

#[test]
fn balanced_gate_lowers_corner_feedrate_with_kinematics() {
    let input = right_angle_corner(1500.0);
    let kinematics = MachineKinematics {
        max_acceleration_mm_s2: Some(500.0),
        max_junction_velocity_mm_s: Some(5.0),
    };

    // No machine contracts: shaping never introduces a new error, so both rewrites are accepted.
    let without = apply_gated(&input, &Contracts::default(), OptimizeMode::Balanced, None);
    let with = apply_gated(
        &input,
        &Contracts::default(),
        OptimizeMode::Balanced,
        Some(&kinematics),
    );
    assert!(without.accepted && with.accepted);

    let corner_without = without.toolpath.segments[0].speed.value();
    let corner_with = with.toolpath.segments[0].speed.value();
    assert!(
        corner_with < corner_without,
        "a kinematics-bearing profile must lower the balanced corner feedrate ({corner_with} !< {corner_without})"
    );
}

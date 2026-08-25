//! The kernel stands alone: resolve → simulate → emit, with no verifier and no analysis layer.
//! If this compiles, layer 1 is genuinely separable (plan Task 4, spec §1.1).

use std::collections::BTreeSet;

use kmet_kernel::optimize::apply_gated_with;
use kmet_kernel::{
    resolve, simulate, Design, EmitParams, Feedrate, Length, MachineKinematics, OptimizeMode,
    ResolveParams, Segment, SegmentKind, Toolpath, Volume,
};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

#[test]
fn resolve_simulate_emit_without_verify_or_trace() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    assert!(!tp.segments.is_empty());

    let m = simulate(&tp);
    assert!(m.total_time_s.value() > 0.0);

    #[allow(deprecated)]
    let g = kmet_kernel::emit(&tp, &EmitParams::default());
    assert!(g.iter().any(|line| line.contains("G1")));
}

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
        power: None,
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

/// `apply_gated_with`'s `mode` and `kinematics` are the parameters the gate inversion newly exposes,
/// and the goldens provably cannot see them: mutating `pipeline_for`'s
/// `Balanced => balanced_pipeline(tp, kinematics)` to pass `None` leaves `report_goldens`,
/// `rewrite_safe_gate` and `rewrite_balanced_max_gate` all green and fails exactly one test in the
/// repository — `balanced_gate_lowers_corner_feedrate_with_kinematics` in
/// `crates/core/tests/machine_kinematics.rs`, which travels with `apply_gated` to `kmet-verify`
/// (plan Task 5). This is that assertion, over that test's fixture, against the mechanism instead of
/// the wrapper: the policy closure is always empty, so nothing but the routing is under test.
#[test]
fn apply_gated_with_routes_kinematics_into_balanced() {
    let input = right_angle_corner(1500.0);
    let kinematics = MachineKinematics {
        max_acceleration_mm_s2: Some(500.0),
        max_junction_velocity_mm_s: Some(5.0),
    };
    let no_errors = |_: &Toolpath| BTreeSet::new();

    let without = apply_gated_with(&input, OptimizeMode::Balanced, None, no_errors);
    let with = apply_gated_with(&input, OptimizeMode::Balanced, Some(&kinematics), no_errors);
    assert!(without.accepted && with.accepted);

    let corner_without = without.toolpath.segments[0].speed.value();
    let corner_with = with.toolpath.segments[0].speed.value();
    assert!(
        corner_with < corner_without,
        "a kinematics-bearing profile must lower the balanced corner feedrate ({corner_with} !< {corner_without})"
    );
}

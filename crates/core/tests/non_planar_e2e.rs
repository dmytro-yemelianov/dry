//! P5.1 acceptance gate: a non-planar design lowers/simulates/emits correctly.
//!
//! Exercises the full pipeline: L1 Orient ops → resolve → simulate → emit (3-axis and 5-axis AB/BC)
//! to prove the toolframe orientation channel works end-to-end for non-planar designs.

// These exercise the deprecated infallible `emit()` on purpose: it is still the entry point the
// in-tree call sites use, and refusing the whole program is part of what is under test here.
#![allow(deprecated)]

use dry_core::{
    emit, resolve, resolve_checked, simulate, Design, EmitParams, Kinematics, ResolveParams,
};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

/// A design with multiple Orient ops interspersed with moves: orient to +X, move, orient to +Y, move,
/// orient back to +Z, move. This exercises orientation propagation, replacement and segment emission.
fn non_planar_design() -> Design {
    design(
        r#"[
            {"op":"geometry","width":0.6,"height":0.2},
            {"op":"extruder","on":true},
            {"op":"speed","print":1000},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"orient","i":1.0,"j":0.0,"k":0.0},
            {"op":"move","x":10,"y":0,"z":0.2},
            {"op":"orient","i":0.0,"j":1.0,"k":0.0},
            {"op":"move","x":20,"y":10,"z":0.2},
            {"op":"orient","i":0.0,"j":0.0,"k":1.0},
            {"op":"move","x":30,"y":10,"z":0.2}
        ]"#,
    )
}

#[test]
fn non_planar_design_resolves_successfully() {
    let d = non_planar_design();
    let result = resolve_checked(&d, &ResolveParams::default());
    assert!(
        result.is_ok(),
        "non-planar design must resolve: {:?}",
        result.err()
    );
}

#[test]
fn non_planar_segments_carry_orientation() {
    let tp = resolve(&non_planar_design(), &ResolveParams::default());
    // The first move (to origin) has no orient yet → None; subsequent moves carry their orient.
    // Move 0: (0,0,0) → (0,0,0.2) with no orient set → None
    // Move 1: (0,0,0.2) → (10,0,0.2) with orient [1,0,0]
    // Move 2: (10,0,0.2) → (20,10,0.2) with orient [0,1,0]
    // Move 3: (20,10,0.2) → (30,10,0.2) with orient [0,0,1]
    assert!(
        tp.segments.len() >= 3,
        "expected at least 3 motion segments"
    );

    // Find segments that carry orientation (skip any travel positioning)
    let oriented: Vec<_> = tp
        .segments
        .iter()
        .filter(|s| s.orientation.is_some())
        .collect();
    assert!(
        oriented.len() >= 3,
        "expected at least 3 oriented segments, got {}",
        oriented.len()
    );

    // Check that the last orient in the sequence is [0,0,1] (+Z)
    let last = oriented.last().unwrap();
    let [i, j, k] = last.orientation.unwrap();
    assert!(
        (i - 0.0).abs() < 1e-12 && (j - 0.0).abs() < 1e-12 && (k - 1.0).abs() < 1e-12,
        "last oriented segment should carry [0,0,1], got [{i},{j},{k}]"
    );
}

#[test]
fn non_planar_simulation_yields_valid_metrics() {
    let tp = resolve(&non_planar_design(), &ResolveParams::default());
    let metrics = simulate(&tp);
    assert!(metrics.segment_count > 0, "simulation must produce metrics");
    assert!(
        metrics.total_time_s.0 > 0.0,
        "simulation time must be positive"
    );
    assert!(
        metrics.extruded_volume.0 > 0.0,
        "total volume must be positive for an extruding design"
    );
}

#[test]
fn non_planar_emit_three_axis_drops_orientation() {
    let tp = resolve(&non_planar_design(), &ResolveParams::default());
    let gcode = emit(&tp, &EmitParams::default());
    // 3-axis emit must not contain rotary words.
    for line in &gcode {
        assert!(
            !line.contains(" A") && !line.contains(" B") && !line.contains(" C"),
            "3-axis emit must carry no rotary words: {line}"
        );
    }
    // Must still contain valid motion commands.
    assert!(
        gcode.iter().any(|l| l.starts_with("G1")),
        "3-axis emit must contain G1 motion"
    );
}

#[test]
fn non_planar_emit_five_axis_ab_produces_rotary_words() {
    let tp = resolve(&non_planar_design(), &ResolveParams::default());
    let params = EmitParams {
        five_axis: true,
        kinematics: Kinematics::Ab {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [0.0, 0.0],
        },
        ..EmitParams::default()
    };
    let gcode = emit(&tp, &params);
    // Tool along +X → A0, B90; tool along +Y → A90, B0.
    assert!(
        gcode.iter().any(|l| l.contains("A90")),
        "5-axis AB emit must produce A90 for +Y orientation: {gcode:?}"
    );
    assert!(
        gcode.iter().any(|l| l.contains("B90")),
        "5-axis AB emit must produce B90 for +X orientation: {gcode:?}"
    );
}

#[test]
fn non_planar_emit_five_axis_bc_produces_rotary_words() {
    let tp = resolve(&non_planar_design(), &ResolveParams::default());
    let params = EmitParams {
        five_axis: true,
        kinematics: Kinematics::Bc {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [0.0, 0.0],
        },
        ..EmitParams::default()
    };
    let gcode = emit(&tp, &params);
    // 5-axis BC emit must produce B and/or C rotary words for non-vertical orientations.
    let has_b = gcode.iter().any(|l| l.contains(" B") || l.starts_with("B"));
    let has_c = gcode.iter().any(|l| l.contains(" C") || l.starts_with("C"));
    assert!(
        has_b || has_c,
        "5-axis BC emit must produce B/C rotary words for tilted orientations: {gcode:?}"
    );
}

#[test]
fn non_planar_step_nc_includes_toolframe() {
    let tp = resolve(&non_planar_design(), &ResolveParams::default());
    let xml = dry_core::emit_step_nc(&tp, &EmitParams::default())
        .expect("a finite toolpath is representable as STEP-NC");
    // The STEP-NC output must include toolframe elements for oriented segments.
    assert!(
        xml.contains("<toolframe"),
        "STEP-NC output must include <toolframe> for non-planar designs"
    );
    // Check that at least one toolframe has i="1" (for the +X orientation).
    assert!(
        xml.contains("i=\"1\""),
        "STEP-NC output must record the +X toolframe orientation"
    );
}

//! `verify` checks a resolved toolpath against machine-safety contracts and structural invariants,
//! returning a `Report` of located findings. These are Dry's own clean-room contracts (not a
//! reproduction of any oracle's text): each is a well-specified property of a safe toolpath.

use dry_core::{
    resolve, resolve_checked, verify, Contracts, Design, ResolveParams, SegmentKind, Severity,
};

fn design_json(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

// A clean 10mm square inside a generous build volume passes with no findings.
#[test]
fn a_clean_toolpath_has_no_findings() {
    let d = design_json(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2},
            {"op":"move","x":10,"y":10,"z":0.2},{"op":"move","x":0,"y":10,"z":0.2},
            {"op":"move","x":0,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let report = verify(&tp, &Contracts::default());
    assert!(
        report.ok(),
        "clean toolpath should pass: {:?}",
        report.findings
    );
    assert_eq!(report.findings.len(), 0);
}

// Moves outside the declared build volume are flagged (one finding per offending segment), as Errors.
#[test]
fn out_of_bounds_moves_are_flagged() {
    let d = design_json(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":50,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let c = Contracts {
        bounds: Some([[0.0, 20.0], [0.0, 20.0], [0.0, 20.0]]),
        ..Contracts::default()
    };
    let report = verify(&tp, &c);
    assert!(!report.ok());
    let bounds: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule == "bounds")
        .collect();
    assert_eq!(bounds.len(), 1, "one out-of-bounds segment");
    assert_eq!(bounds[0].segment, Some(1));
    assert_eq!(bounds[0].severity, Severity::Error);
}

#[test]
fn arc_bounds_check_the_curve_not_just_the_endpoint() {
    let d = design_json(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":5,"y":5,"z":0.2},
            {"op":"arc","cx":0,"cy":5,"x":5,"y":5,"z":null,"clockwise":false}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let c = Contracts {
        bounds: Some([[0.0, 10.0], [0.0, 10.0], [0.0, 1.0]]),
        ..Contracts::default()
    };
    let report = verify(&tp, &c);

    assert!(report
        .findings
        .iter()
        .any(|f| f.rule == "bounds" && f.segment == Some(1)));
}

#[test]
fn authored_arc_endpoint_must_stay_on_the_same_radius() {
    let d = design_json(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":10,"y":0,"z":0.2},
            {"op":"arc","cx":0,"cy":0,"x":1,"y":1,"z":null,"clockwise":false}]"#,
    );
    let err = resolve_checked(&d, &ResolveParams::default()).unwrap_err();
    assert!(err.to_string().contains("endpoint radius differs"));
}

#[test]
fn verifier_flags_invalid_arc_radius_in_ir() {
    let d = design_json(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":10,"y":0,"z":0.2},
            {"op":"arc","cx":0,"cy":0,"x":0,"y":10,"z":null,"clockwise":false}]"#,
    );
    let mut tp = resolve(&d, &ResolveParams::default());
    let arc = tp
        .segments
        .iter_mut()
        .find(|segment| segment.kind == SegmentKind::Arc)
        .unwrap();
    arc.end[0] = Some(dry_core::Length::mm(1.0));
    arc.end[1] = Some(dry_core::Length::mm(1.0));
    let report = verify(&tp, &Contracts::default());
    assert!(report.findings.iter().any(|f| f.rule == "arc-radius"));
}

// A volumetric-flow ceiling is enforced: a fast, fat bead exceeds it.
#[test]
fn excessive_flow_is_flagged() {
    let d = design_json(
        r#"[{"op":"geometry","width":1.0,"height":0.4},{"op":"extruder","on":true},
            {"op":"speed","print":6000},
            {"op":"move","x":0,"y":0,"z":0.4},{"op":"move","x":40,"y":0,"z":0.4}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let c = Contracts {
        max_flow: Some(5.0),
        ..Contracts::default()
    };
    let report = verify(&tp, &c);
    assert!(report.findings.iter().any(|f| f.rule == "max-flow"));
}

// Monotonic-Z (e.g. vase mode): a downward Z move is flagged when the contract requires it.
#[test]
fn z_decrease_is_flagged_when_monotonic_required() {
    let d = design_json(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.4},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let c = Contracts {
        monotonic_z: true,
        ..Contracts::default()
    };
    let report = verify(&tp, &c);
    let z: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule == "monotonic-z")
        .collect();
    assert_eq!(z.len(), 1);
    assert_eq!(z[0].segment, Some(1));
}

// Structural invariant (always on): a travel must not deposit material.
#[test]
fn a_travel_that_extrudes_is_a_structural_error() {
    // hand-build a toolpath whose travel has a non-zero volume (an internal inconsistency).
    let d = design_json(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let mut tp = resolve(&d, &ResolveParams::default());
    tp.segments[1].travel = true; // now a "travel" still carrying filament
    let report = verify(&tp, &Contracts::default());
    assert!(report.findings.iter().any(|f| f.rule == "travel-extrudes"));
}

#[test]
fn non_finite_orientation_is_a_structural_error() {
    let d = design_json(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let mut tp = resolve(&d, &ResolveParams::default());
    tp.segments[1].orientation = Some([f64::NAN, 0.0, 1.0]);

    let report = verify(&tp, &Contracts::default());
    assert!(report
        .findings
        .iter()
        .any(|f| f.rule == "finite" && f.segment == Some(1)));
}

// The report serialises (for the CLI `--json` and the bindings).
#[test]
fn report_serialises_to_json() {
    let d = design_json(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let c = Contracts {
        bounds: Some([[0.0, 1.0], [0.0, 1.0], [0.0, 1.0]]),
        ..Contracts::default()
    };
    let json = serde_json::to_string(&verify(&tp, &c)).unwrap();
    assert!(json.contains("\"rule\""));
    assert!(json.contains("bounds"));
}

//! `verify` checks a resolved toolpath against machine-safety contracts and structural invariants,
//! returning a `Report` of located findings. These are Dry's own clean-room contracts (not a
//! reproduction of any oracle's text): each is a well-specified property of a safe toolpath.

use drymachina_contracts::{Contracts, RuleId, Severity};
use drymachina_kernel::{
    import_gcode, resolve, resolve_checked, Design, GcodeImportParams, Length, ResolveParams,
    SegmentKind,
};
use drymachina_verify::verify;

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
    // "Clean" is a claim about coverage as much as about findings: this square is checked by the
    // full structural set, not merely by the five rules Contracts::default() reached before H1.3.
    assert_eq!(report.segments_inspected, tp.segments.len());
    for rule in [
        RuleId::Continuity,
        RuleId::SegmentLength,
        RuleId::NegativeQuantity,
        RuleId::FilamentConsistency,
    ] {
        assert!(report.evaluated(rule), "{} was not in force", rule.as_str());
    }
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
    arc.end[0] = Some(Length::mm(1.0));
    arc.end[1] = Some(Length::mm(1.0));
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

// Structural invariant (always on): a travel must not deposit material. Advisory, not fatal — the
// flag disagreeing with the volume is a modelling inconsistency, and on the firmware these programs
// run a `G0` carrying an `E` word extrudes exactly as commanded (see `RuleId::default_severity`).
#[test]
fn a_travel_that_extrudes_is_a_structural_warning() {
    // hand-build a toolpath whose travel has a non-zero volume (an internal inconsistency).
    let d = design_json(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let mut tp = resolve(&d, &ResolveParams::default());
    tp.segments[1].travel = true; // now a "travel" still carrying filament
    let report = verify(&tp, &Contracts::default());
    let travel: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule == "travel-extrudes")
        .collect();
    assert_eq!(travel.len(), 1);
    assert_eq!(travel[0].severity, Severity::Warning);
    // Reported and located, but it does not fail the gate.
    assert!(report.ok());
    assert_eq!(report.error_count(), 0);
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

#[test]
fn excessive_retraction_speed_is_flagged() {
    let tp = import_gcode("G1 E-4.5 F6000\n", &GcodeImportParams::default()).unwrap();
    let c = Contracts {
        max_retraction_speed: Some(3000.0),
        ..Contracts::default()
    };
    let report = verify(&tp, &c);
    assert!(!report.ok());
    assert!(report.findings.iter().any(|f| f.rule == "retraction-speed"));
}

#[test]
fn stationary_deposit_is_not_a_retraction_prime() {
    let d = design_json(
        r#"[{"op":"geometry","width":0.6,"height":0.2},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"deposit","volume":5.0,"speed":6000.0}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let c = Contracts {
        max_retraction_speed: Some(3000.0),
        ..Contracts::default()
    };
    let report = verify(&tp, &c);
    assert!(!report.findings.iter().any(|f| f.rule == "retraction-speed"));
}

#[test]
fn excessive_retraction_distance_is_flagged() {
    let tp = import_gcode("G1 E-4.5 F2000\n", &GcodeImportParams::default()).unwrap();
    let c = Contracts {
        max_retraction_distance: Some(2.0),
        ..Contracts::default()
    };
    let report = verify(&tp, &c);
    assert!(!report.ok());
    assert!(report
        .findings
        .iter()
        .any(|f| f.rule == "retraction-distance"));
}

#[test]
fn travel_without_retraction_is_flagged() {
    let tp = import_gcode(
        "M83\nG1 X10 E0.5 F1200\nG1 X60 F9000\nG1 X70 E0.5 F1200\n",
        &GcodeImportParams::default(),
    )
    .unwrap();
    let c = Contracts {
        max_travel_without_retract: Some(30.0),
        ..Contracts::default()
    };
    let report = verify(&tp, &c);
    // travel-without-retraction is a process/quality advisory: a warning, not an error. (The imported
    // fixture also raises unrelated `bead` errors because it carries no width/height, so we check the
    // target finding's severity rather than the whole report.)
    let finding = report
        .findings
        .iter()
        .find(|f| f.rule == "travel-without-retraction")
        .expect("flagged travel-without-retraction");
    assert_eq!(finding.severity, Severity::Warning);
}

#[test]
fn first_layer_height_out_of_range_is_flagged() {
    let tp = import_gcode(
        "M83\nG1 X10 Y0 Z0.35 E0.5 F1200\nG1 X20 Y0 Z0.35 E0.5 F1200\n",
        &GcodeImportParams::default(),
    )
    .unwrap();
    let c = Contracts {
        first_layer_height_range: Some([0.1, 0.3]),
        ..Contracts::default()
    };
    let report = verify(&tp, &c);
    let finding = report
        .findings
        .iter()
        .find(|f| f.rule == "first-layer-height")
        .expect("flagged first-layer-height");
    assert_eq!(finding.severity, Severity::Warning);
}

#[test]
fn first_layer_speed_out_of_range_is_flagged() {
    let tp = import_gcode(
        "M83\nG1 X10 Y0 Z0.2 E0.5 F3000\nG1 X20 Y0 Z0.2 E0.5 F3000\n",
        &GcodeImportParams::default(),
    )
    .unwrap();
    let c = Contracts {
        first_layer_speed_range: Some([500.0, 2000.0]),
        ..Contracts::default()
    };
    let report = verify(&tp, &c);
    let finding = report
        .findings
        .iter()
        .find(|f| f.rule == "first-layer-speed")
        .expect("flagged first-layer-speed");
    assert_eq!(finding.severity, Severity::Warning);
}

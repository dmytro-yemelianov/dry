//! `verify` checks a resolved toolpath against machine-safety contracts and structural invariants,
//! returning a `Report` of located findings. These are Dry's own clean-room contracts (not a
//! reproduction of any oracle's text): each is a well-specified property of a safe toolpath.

use dry_core::{
    import_gcode, resolve, resolve_checked, verify, Contracts, Design, Feedrate, GcodeImportParams,
    KinematicContracts, Length, ResolveParams, RuleId, Segment, SegmentKind, Severity, Toolpath,
    Volume,
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

// #277: a stationary filament move is neither a deposition nor a travel, and the two per-segment
// state machines used to let it fall through both arms and inherit the previous segment's state.

/// `travel-without-retraction` contracts used by the #277 fail-open regression tests below.
fn travel_contracts(max: f64) -> Contracts {
    Contracts {
        max_travel_without_retract: Some(max),
        ..Contracts::default()
    }
}

#[test]
fn a_stationary_prime_between_prints_raises_no_junction_finding() {
    // print → stationary prime (`G1 E1`: zero geometric length, the tool stays where it is) →
    // print at a sharp corner. The tool is at rest during the prime, so the second print starts
    // from zero XY velocity — there is no junction to measure across the stop. The prime leaves the
    // position unchanged, so `junction_contiguous` cannot screen the pair; only the state reset
    // can. Before #277 this fired a false `junction-velocity`.
    let tp = import_gcode(
        // The opening `G0` establishes the machine position: without it the first `G1` starts from
        // nowhere (`start = None`), has no computable length, and is not a deposition — the prime
        // would then be tested against a `prev_*` that was never recorded.
        "M83\nG0 X0 Y0 Z0.2 F3000\nG1 X10 Y0 E0.5 F1500\nG1 E1 F2400\nG1 Y10 E0.5 F1500\n",
        &GcodeImportParams::default(),
    )
    .unwrap();
    let c = Contracts {
        kinematics: Some(KinematicContracts {
            max_junction_velocity_mm_s: Some(20.0),
            ..KinematicContracts::default()
        }),
        ..Contracts::default()
    };
    let report = verify(&tp, &c);
    assert!(!report
        .findings
        .iter()
        .any(|f| f.rule == "junction-velocity"));
}

#[test]
fn directly_contiguous_prints_still_raise_the_junction_finding() {
    // Positive control for `a_stationary_prime_between_prints_raises_no_junction_finding`: the same
    // corner without the prime in between must still fire — a scope fix that only stops a rule
    // firing is indistinguishable from deleting it. 1500 mm/min = 25 mm/s into a 90° turn at a
    // 20 mm/s square-corner velocity is over the limit, exactly as the kinematics golden records.
    let tp = import_gcode(
        // Opening `G0` for the same reason as the negative test above.
        "M83\nG0 X0 Y0 Z0.2 F3000\nG1 X10 Y0 E0.5 F1500\nG1 Y10 E0.5 F1500\n",
        &GcodeImportParams::default(),
    )
    .unwrap();
    let c = Contracts {
        kinematics: Some(KinematicContracts {
            max_junction_velocity_mm_s: Some(20.0),
            ..KinematicContracts::default()
        }),
        ..Contracts::default()
    };
    let report = verify(&tp, &c);
    assert!(report
        .findings
        .iter()
        .any(|f| f.rule == "junction-velocity"));
}

/// A motion segment for the hand-authored-IR tests: a straight line from `start` to `end` with the
/// given classification and filament delta. Width/height/temperature set so the `bead` and
/// `cold-extrusion` structural rules stay quiet.
fn motion_seg(start: [f64; 3], end: [f64; 3], travel: bool, filament: f64, volume: f64) -> Segment {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let dz = end[2] - start[2];
    let length = (dx * dx + dy * dy + dz * dz).sqrt();
    Segment {
        start: start.map(|v| Some(Length::mm(v))),
        end: end.map(|v| Some(Length::mm(v))),
        travel,
        speed: Feedrate(9000.0),
        length: Length::mm(length),
        volume: Volume(volume),
        filament: Length::mm(filament),
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

#[test]
fn a_traversing_unretract_clears_the_retracted_state() {
    // Hand-authored IR can construct a segment no in-tree producer emits (#277): filament forward,
    // zero volume, positive length, not flagged travel — a de-retraction that traverses. It used
    // to fall through every arm of the retraction state machine and inherit `retracted == true`
    // from the retract before it, so the long travel below stayed silent (fail-open).
    let tp = Toolpath {
        version: 0,
        meta: None,
        segments: vec![
            motion_seg([0.0, 0.0, 0.2], [10.0, 0.0, 0.2], false, 0.33, 0.8), // print
            motion_seg([10.0, 0.0, 0.2], [10.0, 0.0, 0.2], false, -2.0, 0.0), // stationary retract
            motion_seg([10.0, 0.0, 0.2], [20.0, 0.0, 0.2], true, 0.0, 0.0),  // short travel
            motion_seg([20.0, 0.0, 0.2], [30.0, 0.0, 0.2], false, 2.0, 0.0), // traversing unretract
            motion_seg([30.0, 0.0, 0.2], [70.0, 0.0, 0.2], true, 0.0, 0.0), // long travel → must fire
        ],
    };
    let report = verify(&tp, &travel_contracts(30.0));
    assert!(report
        .findings
        .iter()
        .any(|f| f.rule == "travel-without-retraction"));
}

#[test]
fn an_imported_travel_purge_clears_the_retracted_state() {
    // OrcaSlicer writes its purge/prime lines as `G0` with an `E` word: `travel: true`, a recovered
    // volume, positive filament. The filament really is pushed forward, so the state must say
    // unretracted and the following long travel must fire — before #277 the purge silently kept the
    // retracted state and the rule stayed quiet (fail-open, pre-existing since the corpus import).
    let tp = import_gcode(
        "M83\nG1 X10 Y0 E0.5 F1200\nG1 E-2 F2400\nG0 X40 Y0 E1 F9000\nG1 X90 Y0 F9000\n",
        &GcodeImportParams::default(),
    )
    .unwrap();
    let report = verify(&tp, &travel_contracts(30.0));
    assert!(report
        .findings
        .iter()
        .any(|f| f.rule == "travel-without-retraction"));
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

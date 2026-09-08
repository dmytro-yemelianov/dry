use dry_core::apt::{
    import_apt, import_apt_with_limits, import_apt_with_map, AptErrorCode, AptImportLimits,
    AptImportParams, AptUnits, UnknownMajorWordPolicy,
};
use dry_core::ir::SegmentKind;

#[test]
fn apt_import_units_and_scaling() {
    let apt_mm = r#"
PARTNO / TEST_MM
UNITS / MM
FEDRAT / MMPM, 1200.0
GOTO / 10.0, 20.0, 30.0
FINI
"#;
    let path = import_apt(
        apt_mm,
        &AptImportParams {
            version: 0,
            assume_units: None,
            unknown_major_words: UnknownMajorWordPolicy::Refuse,
            arc_tolerance_rel: 1e-4,
        },
    )
    .expect("import mm");

    assert_eq!(path.segments.len(), 1);
    let seg = &path.segments[0];
    assert_eq!(seg.end[0].map(|l| l.value()), Some(10.0));
    assert_eq!(seg.end[1].map(|l| l.value()), Some(20.0));
    assert_eq!(seg.end[2].map(|l| l.value()), Some(30.0));

    let apt_inches = r#"
PARTNO / TEST_INCH
UNITS / INCHES
FEDRAT / IPM, 50.0
GOTO / 1.0, 2.0, 3.0
FINI
"#;
    let path_in = import_apt(
        apt_inches,
        &AptImportParams {
            version: 0,
            assume_units: None,
            unknown_major_words: UnknownMajorWordPolicy::Refuse,
            arc_tolerance_rel: 1e-4,
        },
    )
    .expect("import inches");

    assert_eq!(path_in.segments.len(), 1);
    let seg_in = &path_in.segments[0];
    assert!((seg_in.end[0].unwrap().value() - 25.4).abs() < 1e-6);
    assert!((seg_in.end[1].unwrap().value() - 50.8).abs() < 1e-6);
    assert!((seg_in.end[2].unwrap().value() - 76.2).abs() < 1e-6);
    // 50 IPM * 25.4 = 1270 mm/min
    assert!((seg_in.speed.value() - 1270.0).abs() < 1e-6);
}

#[test]
fn apt_import_rapid_one_shot() {
    let apt_rapid = r#"
UNITS / MM
FEDRAT / MMPM, 600.0
RAPID
GOTO / 10.0, 0.0, 0.0
GOTO / 20.0, 0.0, 0.0
FINI
"#;
    let path = import_apt(
        apt_rapid,
        &AptImportParams {
            version: 0,
            assume_units: None,
            unknown_major_words: UnknownMajorWordPolicy::Refuse,
            arc_tolerance_rel: 1e-4,
        },
    )
    .unwrap();

    assert_eq!(path.segments.len(), 2);
    assert!(path.segments[0].travel);
    assert_eq!(path.segments[0].speed.value(), 0.0);

    assert!(!path.segments[1].travel);
    assert_eq!(path.segments[1].speed.value(), 600.0);
}

#[test]
fn apt_import_multax_tracking() {
    let apt_multax = r#"
UNITS / MM
FEDRAT / MMPM, 1000.0
MULTAX / ON
GOTO / 10.0, 20.0, 30.0, 0.0, 0.70710678, 0.70710678
MULTAX / OFF
GOTO / 10.0, 20.0, 40.0
FINI
"#;
    let path = import_apt(
        apt_multax,
        &AptImportParams {
            version: 0,
            assume_units: None,
            unknown_major_words: UnknownMajorWordPolicy::Refuse,
            arc_tolerance_rel: 1e-4,
        },
    )
    .unwrap();

    assert_eq!(path.segments.len(), 2);
    let o1 = path.segments[0].orientation.expect("multax on orientation");
    assert!((o1[0]).abs() < 1e-6);
    assert!((o1[1] - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    assert!((o1[2] - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-6);

    // After MULTAX/OFF, 3-axis GOTO resets tool axis to [0, 0, 1] which gives None
    assert_eq!(path.segments[1].orientation, None);
}

#[test]
fn apt_import_multax_off_rejects_six_coordinates() {
    let bad_apt = r#"
UNITS / MM
MULTAX / OFF
GOTO / 10.0, 20.0, 30.0, 0.0, 0.0, 1.0
FINI
"#;
    let err = import_apt(
        bad_apt,
        &AptImportParams {
            version: 0,
            assume_units: None,
            unknown_major_words: UnknownMajorWordPolicy::Refuse,
            arc_tolerance_rel: 1e-4,
        },
    )
    .unwrap_err();

    assert_eq!(err.code, AptErrorCode::MultaxInconsistent);
}

#[test]
fn apt_import_movarc_planar_reconstruction() {
    // Start at (10, 0, 0), arc center at (0, 0, 0), radius 10, CCW normal (0, 0, 1), sweep 90 deg -> end at (0, 10, 0)
    let apt_arc = r#"
UNITS / MM
FEDRAT / MMPM, 500.0
GOTO / 10.0, 0.0, 0.0
MOVARC / 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 10.0, 90.0
GOTO / 0.0, 10.0, 0.0
FINI
"#;
    let path = import_apt(
        apt_arc,
        &AptImportParams {
            version: 0,
            assume_units: None,
            unknown_major_words: UnknownMajorWordPolicy::Refuse,
            arc_tolerance_rel: 1e-3,
        },
    )
    .unwrap();

    assert_eq!(path.segments.len(), 2);
    let arc_seg = &path.segments[1];
    assert_eq!(arc_seg.kind, SegmentKind::Arc);
    assert_eq!(
        arc_seg.centre.map(|[x, y]| (x.value(), y.value())),
        Some((0.0, 0.0))
    );
    assert!(!arc_seg.clockwise);
    assert_eq!(arc_seg.end[0].map(|l| l.value()), Some(0.0));
    assert_eq!(arc_seg.end[1].map(|l| l.value()), Some(10.0));
}

#[test]
fn apt_import_cycle_drill_expansion() {
    let apt_drill = r#"
UNITS / MM
FEDRAT / MMPM, 1000.0
GOTO / 0.0, 0.0, 50.0
CYCLE / DRILL, 20.0, MMPM, 150.0, RAPTO, 2.0, DWELL, 1.5
GOTO / 15.0, 25.0, 10.0
CYCLE / OFF
FINI
"#;
    let path = import_apt(
        apt_drill,
        &AptImportParams {
            version: 0,
            assume_units: None,
            unknown_major_words: UnknownMajorWordPolicy::Refuse,
            arc_tolerance_rel: 1e-4,
        },
    )
    .unwrap();

    // 1 initial move + 4 cycle moves:
    // (1) initial GOTO (0, 0, 50)
    // (2) traverse to clearance: (15, 25, 12) [travel = true]
    // (3) feed plunge down to bottom: (15, 25, -10) at 150 mm/min
    // (4) dwell: stationary dwell 1.5s
    // (5) rapid retract to clearance: (15, 25, 12) [travel = true]
    assert_eq!(path.segments.len(), 5);
    assert_eq!(path.segments[0].kind, SegmentKind::Line);

    // Segment 1: traverse to clearance (15, 25, 10 + 2 = 12)
    assert!(path.segments[1].travel);
    assert_eq!(path.segments[1].end[2].unwrap().value(), 12.0);

    // Segment 2: feed plunge to bottom (15, 25, 10 - 20 = -10)
    assert!(!path.segments[2].travel);
    assert_eq!(path.segments[2].end[2].unwrap().value(), -10.0);
    assert_eq!(path.segments[2].speed.value(), 150.0);

    // Segment 3: dwell
    assert_eq!(path.segments[3].kind, SegmentKind::Dwell);
    assert_eq!(path.segments[3].dwell_s, Some(1.5));

    // Segment 4: rapid retract to clearance
    assert!(path.segments[4].travel);
    assert_eq!(path.segments[4].end[2].unwrap().value(), 12.0);
}

#[test]
fn apt_import_limits_enforced() {
    let apt_huge = "GOTO / 1.0, 2.0, 3.0\n".repeat(20);
    let limits = AptImportLimits {
        max_source_bytes: 100,
        max_statements: 1000,
        max_line_bytes: 4096,
        max_continuation_lines: 64,
        max_values_per_statement: 100,
        max_cycle_holes: 1000,
    };
    let err = import_apt_with_limits(
        &apt_huge,
        &AptImportParams {
            version: 0,
            assume_units: Some(AptUnits::Mm),
            unknown_major_words: UnknownMajorWordPolicy::Refuse,
            arc_tolerance_rel: 1e-4,
        },
        &limits,
    )
    .unwrap_err();

    assert_eq!(err.code, AptErrorCode::LimitExceeded);
}

#[test]
fn apt_import_unknown_major_words_policy() {
    let apt_src = r#"
UNITS / MM
CUSTOMCMD / 123, 456
GOTO / 10.0, 20.0, 30.0
FINI
"#;

    // Refuse policy errors
    let err = import_apt(
        apt_src,
        &AptImportParams {
            version: 0,
            assume_units: None,
            unknown_major_words: UnknownMajorWordPolicy::Refuse,
            arc_tolerance_rel: 1e-4,
        },
    )
    .unwrap_err();
    assert_eq!(err.code, AptErrorCode::MajorWordUnknown);

    // Preserve policy retains unmodeled statement and succeeds
    let imported = import_apt_with_map(
        apt_src,
        &AptImportParams {
            version: 0,
            assume_units: None,
            unknown_major_words: UnknownMajorWordPolicy::Preserve,
            arc_tolerance_rel: 1e-4,
        },
    )
    .unwrap();
    assert_eq!(imported.toolpath.segments.len(), 1);
    assert_eq!(imported.unmodeled_statements.len(), 2);
    assert_eq!(imported.unmodeled_statements[0].major, "CUSTOMCMD");
    assert_eq!(imported.unmodeled_statements[1].major, "FINI");
}

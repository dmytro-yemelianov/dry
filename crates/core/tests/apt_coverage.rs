use dry_core::apt::{
    import_apt, import_apt_with_limits, import_apt_with_map, AptErrorCode, AptImportLimits,
    AptImportParams, UnknownMajorWordPolicy,
};
use dry_core::emit::{emit_apt_to_writer, AptFrame, EmitParams};
use dry_core::ir::{Segment, SegmentKind};
use dry_core::units::{Feedrate, Length, Volume};

fn dummy_seg() -> Segment {
    Segment {
        start: [None, None, None],
        end: [None, None, None],
        travel: false,
        speed: Feedrate(100.0),
        length: Length(0.0),
        volume: Volume(0.0),
        filament: Length(0.0),
        width: None,
        height: None,
        kind: SegmentKind::Line,
        centre: None,
        clockwise: false,
        temperature: None,
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
fn test_apt_emit_rejections_and_special_segments() {
    let params = EmitParams {
        apt_frame: AptFrame {
            partno: Some("PART_TEST".to_string()),
            machin: Some("MCH_TEST".to_string()),
            decimals: 4,
        },
        ..Default::default()
    };

    // 1. Manual Gcode rejection
    let mut seg_manual = dummy_seg();
    seg_manual.kind = SegmentKind::ManualGcode;
    seg_manual.manual_gcode = Some("G4 P100".to_string());

    let mut buf = Vec::new();
    let err = emit_apt_to_writer(vec![Ok(seg_manual)], &params, &mut buf);
    assert!(err.is_err());
    assert!(format!("{err:?}").contains("manual-gcode"));

    // 2. Retract / Unretract / Deposit rejections
    for kind in [
        SegmentKind::Retract,
        SegmentKind::Unretract,
        SegmentKind::Deposit,
    ] {
        let mut seg_extrusion = dummy_seg();
        seg_extrusion.kind = kind;
        let mut buf = Vec::new();
        let err = emit_apt_to_writer(vec![Ok(seg_extrusion)], &params, &mut buf);
        assert!(err.is_err());
        assert!(format!("{err:?}").contains("poseless"));
    }

    // 3. Power negative or non-finite rejection
    let mut seg_neg_power = dummy_seg();
    seg_neg_power.power = Some(-50.0);
    let mut buf = Vec::new();
    let err = emit_apt_to_writer(vec![Ok(seg_neg_power)], &params, &mut buf);
    assert!(err.is_err());

    // 4. Power = 0.0 emits SPINDL / OFF, and positive emits SPINDL / RPM, ..., CLW
    // and tool emits LOADTL
    let mut seg_spindle = dummy_seg();
    seg_spindle.tool = Some(3);
    seg_spindle.power = Some(1200.0);
    seg_spindle.end = [Some(Length(5.0)), Some(Length(5.0)), Some(Length(0.0))];

    let mut seg_spindle_off = dummy_seg();
    seg_spindle_off.tool = Some(3);
    seg_spindle_off.power = Some(0.0);
    seg_spindle_off.end = [Some(Length(10.0)), Some(Length(10.0)), Some(Length(0.0))];

    let mut seg_dwell = dummy_seg();
    seg_dwell.kind = SegmentKind::Dwell;
    seg_dwell.dwell_s = Some(1.5);

    let mut buf = Vec::new();
    let res = emit_apt_to_writer(
        vec![Ok(seg_spindle), Ok(seg_spindle_off), Ok(seg_dwell)],
        &params,
        &mut buf,
    );
    assert!(res.is_ok());
    let apt_txt = String::from_utf8(buf).unwrap();
    assert!(apt_txt.contains("PARTNO / PART_TEST"));
    assert!(apt_txt.contains("MACHIN / MCH_TEST"));
    assert!(apt_txt.contains("LOADTL / 3"));
    assert!(apt_txt.contains("SPINDL / RPM, 1200.0000, CLW"));
    assert!(apt_txt.contains("SPINDL / OFF"));
    assert!(apt_txt.contains("DELAY / 1.5000"));
}

#[test]
fn test_apt_import_limits_enforcement() {
    let apt_src = "UNITS / MM\nGOTO / 1.0, 2.0, 3.0\nFINI\n";

    // 1. max_source_bytes
    let limits = AptImportLimits {
        max_source_bytes: 10,
        ..Default::default()
    };
    let err = import_apt_with_limits(apt_src, &AptImportParams::default(), &limits);
    assert!(err.is_err());
    assert_eq!(err.unwrap_err().code, AptErrorCode::LimitExceeded);

    // 2. max_line_bytes
    let long_line = format!("PPRINT / {}\nUNITS / MM\nFINI\n", "A".repeat(50));
    let limits_line = AptImportLimits {
        max_line_bytes: 20,
        ..Default::default()
    };
    let err = import_apt_with_limits(&long_line, &AptImportParams::default(), &limits_line);
    assert!(err.is_err());
    assert_eq!(err.unwrap_err().code, AptErrorCode::LimitExceeded);

    // 3. max_statements
    let limits_stmt = AptImportLimits {
        max_statements: 1,
        ..Default::default()
    };
    let err = import_apt_with_limits(apt_src, &AptImportParams::default(), &limits_stmt);
    assert!(err.is_err());
    assert_eq!(err.unwrap_err().code, AptErrorCode::LimitExceeded);
}

#[test]
fn test_apt_unknown_major_words_and_dwell_cycle() {
    // Unknown major words with Preserve vs Refuse
    let apt_unknown = "UNITS / MM\nCUSTOMWORD / 1, 2, 3\nGOTO / 0, 0, 0\nFINI\n";
    let params_refuse = AptImportParams {
        unknown_major_words: UnknownMajorWordPolicy::Refuse,
        ..Default::default()
    };
    let err = import_apt(apt_unknown, &params_refuse);
    assert!(err.is_err());
    assert_eq!(err.unwrap_err().code, AptErrorCode::MajorWordUnknown);

    let params_preserve = AptImportParams {
        unknown_major_words: UnknownMajorWordPolicy::Preserve,
        ..Default::default()
    };
    let imported =
        import_apt_with_map(apt_unknown, &params_preserve).expect("preserve should succeed");
    assert!(imported
        .unmodeled_statements
        .iter()
        .any(|s| s.major == "CUSTOMWORD"));

    // Drill cycle with DWELL
    let apt_drill_dwell = r#"
UNITS / MM
FEDRAT / MMPM, 500.0
GOTO / 0.0, 0.0, 50.0
CYCLE / DRILL, 15.0, MMPM, 250.0, RAPTO, 3.0, DWELL, 2.0
GOTO / 10.0, 10.0, 0.0
CYCLE / OFF
FINI
"#;
    let imported_drill = import_apt(apt_drill_dwell, &AptImportParams::default()).unwrap();
    let kinds: Vec<_> = imported_drill.segments.iter().map(|s| s.kind).collect();
    assert!(kinds.contains(&SegmentKind::Dwell));
}

#[test]
fn test_apt_error_code_display_and_coverage() {
    let codes = [
        AptErrorCode::Syntax,
        AptErrorCode::ContinuationUnterminated,
        AptErrorCode::NonFinite,
        AptErrorCode::GotoArity,
        AptErrorCode::LimitExceeded,
        AptErrorCode::UnitMissing,
        AptErrorCode::UnitUnknown,
        AptErrorCode::UnitLate,
        AptErrorCode::FeedNegative,
        AptErrorCode::FeedUnitUnsupported,
        AptErrorCode::MultaxInconsistent,
        AptErrorCode::TlaxisDegenerate,
        AptErrorCode::ArcNormalDegenerate,
        AptErrorCode::ArcNormalUnsupported,
        AptErrorCode::ArcFullTurn,
        AptErrorCode::ArcOffCircle,
        AptErrorCode::ArcSweepMismatch,
        AptErrorCode::ArcRadiusNonpositive,
        AptErrorCode::ArcEndpointMissing,
        AptErrorCode::RapidArc,
        AptErrorCode::CycleUnsupported,
        AptErrorCode::CycleMinorWord,
        AptErrorCode::CycleDomain,
        AptErrorCode::CycleOffInactive,
        AptErrorCode::CycleStatementInside,
        AptErrorCode::CycleClearanceViolation,
        AptErrorCode::LoadtlDomain,
        AptErrorCode::SpindleDomain,
        AptErrorCode::SpindleOnWithoutRpm,
        AptErrorCode::SpindleDirectionUnsupported,
        AptErrorCode::IrbcamChannelCollision,
        AptErrorCode::CutcomOn,
        AptErrorCode::InsertOpaque,
        AptErrorCode::MajorWordHazardous,
        AptErrorCode::MajorWordUnknown,
        AptErrorCode::IrbcamFeedUnstated,
        AptErrorCode::IrbcamTravelSpeedUnstated,
    ];
    for code in codes {
        let disp = format!("{code}");
        assert_eq!(disp, code.as_str());
    }
}

#[test]
fn test_apt_lift_more_statements_and_readers() {
    let apt_rich = r#"
PARTNO / RICH_TEST
MACHIN / MILL_5AXIS
UNITS / MM
LOADTL / 2
SPINDL / RPM, 3000.0, CLW
COOLNT / ON
FEDRAT / MMPM, 800.0
MULTAX / ON
GOTO / 0.0, 0.0, 10.0, 0.0, 0.0, 1.0
GOTO / 50.0, 0.0, 10.0, 0.0, 0.7071067811865476, 0.7071067811865476
DELAY / 0.5
COOLNT / OFF
SPINDL / OFF
MULTAX / OFF
FINI
"#;

    let res = import_apt(apt_rich, &AptImportParams::default()).expect("rich import");
    assert_eq!(res.segments.len(), 3); // 2 moves + 1 delay
    assert_eq!(res.segments[0].tool, Some(2));
    assert_eq!(res.segments[0].power, Some(3000.0));
    let o = res.segments[1].orientation.unwrap();
    assert!((o[1] - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    assert_eq!(res.segments[2].kind, SegmentKind::Dwell);

    // Test import_apt_reader
    use dry_core::apt::import_apt_reader;
    let reader_res =
        import_apt_reader(apt_rich.as_bytes(), &AptImportParams::default()).expect("reader import");
    assert_eq!(reader_res.segments.len(), 3);

    // Test units missing error
    let apt_unit_missing = r#"
FEDRAT / MMPM, 100.0
GOTO / 1.0, 1.0, 1.0
UNITS / MM
FINI
"#;
    let err_missing = import_apt(apt_unit_missing, &AptImportParams::default());
    assert!(err_missing.is_err());
    assert_eq!(err_missing.unwrap_err().code, AptErrorCode::UnitMissing);

    // Test feed negative error
    let apt_feed_neg = r#"
UNITS / MM
FEDRAT / MMPM, -100.0
FINI
"#;
    let err_feed = import_apt(apt_feed_neg, &AptImportParams::default());
    assert!(err_feed.is_err());
    assert_eq!(err_feed.unwrap_err().code, AptErrorCode::FeedNegative);
}

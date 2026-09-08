use dry_core::codec::CodecError;
use dry_core::emit::{
    emit_stream_to_writer, DwellPolicy, EmitParams, ExtrusionCarry, FirmwareFlavor, IrbcamFrame,
    IrbcamLayout, RapidEncoding,
};
use dry_core::ir::{Segment, SegmentKind, Toolpath};
use dry_core::units::{Feedrate, Length, Volume};

fn make_seg(
    start: [f64; 3],
    end: [f64; 3],
    kind: SegmentKind,
    travel: bool,
    speed: f64,
) -> Segment {
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
        length: Length::mm(
            ((end[0] - start[0]).powi(2)
                + (end[1] - start[1]).powi(2)
                + (end[2] - start[2]).powi(2))
            .sqrt(),
        ),
        kind,
        travel,
        speed: Feedrate(speed),
        volume: Volume(0.0),
        filament: Length::ZERO,
        width: None,
        height: None,
        tool: Some(1),
        dwell_s: None,
        centre: None,
        clockwise: false,
        fan: None,
        power: Some(1000.0),
        temperature: None,
        flow: None,
        manual_gcode: None,
        orientation: None,
        control_points: None,
    }
}

#[test]
fn irbcam_json_emits_valid_structure() {
    let mut seg1 = make_seg(
        [0.0, 0.0, 0.0],
        [10.0, 20.0, 30.0],
        SegmentKind::Line,
        true,
        3000.0,
    );
    seg1.orientation = Some([0.0, 0.0, 1.0]);

    let mut seg2 = make_seg(
        [10.0, 20.0, 30.0],
        [50.0, 20.0, 30.0],
        SegmentKind::Line,
        false,
        600.0,
    );
    seg2.orientation = Some([
        0.0,
        std::f64::consts::FRAC_1_SQRT_2,
        std::f64::consts::FRAC_1_SQRT_2,
    ]);

    let toolpath = Toolpath {
        version: 0,
        meta: None,
        segments: vec![seg1, seg2],
    };

    let params = EmitParams {
        flavor: FirmwareFlavor::Irbcam,
        irbcam_frame: IrbcamFrame {
            layout: IrbcamLayout::Json,
            rapid: RapidEncoding::MinusOne,
            dwell: DwellPolicy::Refuse,
            extrusion: ExtrusionCarry::Refuse,
            ..IrbcamFrame::default()
        },
        ..EmitParams::default()
    };

    let mut out = Vec::new();
    emit_stream_to_writer(toolpath.segments.into_iter().map(Ok), &params, &mut out)
        .expect("emit irbcam json");

    let s = String::from_utf8(out).expect("utf-8 json");
    assert!(s.contains("\"targets\": ["));
    assert!(s.contains("\"toolNumber\":"));
    assert!(s.contains("\"spindleSpeed\":"));
}

#[test]
fn irbcam_csv_emits_valid_header_and_rows() {
    let seg1 = make_seg(
        [0.0, 0.0, 0.0],
        [10.0, 20.0, 30.0],
        SegmentKind::Line,
        false,
        1200.0,
    );
    let toolpath = Toolpath {
        version: 0,
        meta: None,
        segments: vec![seg1],
    };

    let params = EmitParams {
        flavor: FirmwareFlavor::IrbcamCsv,
        irbcam_frame: IrbcamFrame {
            layout: IrbcamLayout::Csv,
            rapid: RapidEncoding::Explicit,
            dwell: DwellPolicy::Refuse,
            extrusion: ExtrusionCarry::Refuse,
            ..IrbcamFrame::default()
        },
        ..EmitParams::default()
    };

    let mut out = Vec::new();
    emit_stream_to_writer(toolpath.segments.into_iter().map(Ok), &params, &mut out)
        .expect("emit irbcam csv");

    let s = String::from_utf8(out).expect("utf-8 csv");
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines[0], "x,y,z,rz1,ry,rz2,velocity,type");
    assert_eq!(lines.len(), 2);
    assert!(lines[1].starts_with("10.000000,20.000000,30.000000,"));
}

#[test]
fn irbcam_arc_emits_midpoint_and_endpoint() {
    let mut arc_seg = make_seg(
        [10.0, 0.0, 0.0],
        [0.0, 10.0, 0.0],
        SegmentKind::Arc,
        false,
        600.0,
    );
    arc_seg.centre = Some([Length::mm(0.0), Length::mm(0.0)]);
    arc_seg.clockwise = false;

    let toolpath = Toolpath {
        version: 0,
        meta: None,
        segments: vec![arc_seg],
    };

    let params = EmitParams {
        flavor: FirmwareFlavor::IrbcamCsv,
        irbcam_frame: IrbcamFrame {
            layout: IrbcamLayout::Csv,
            rapid: RapidEncoding::Explicit,
            dwell: DwellPolicy::Refuse,
            extrusion: ExtrusionCarry::Refuse,
            ..IrbcamFrame::default()
        },
        ..EmitParams::default()
    };

    let mut out = Vec::new();
    emit_stream_to_writer(toolpath.segments.into_iter().map(Ok), &params, &mut out)
        .expect("emit arc csv");

    let s = String::from_utf8(out).expect("utf-8 csv");
    let lines: Vec<&str> = s.lines().collect();
    // Header + midpoint + endpoint = 3 lines
    assert_eq!(lines.len(), 3);
    // Midpoint has type 1
    assert!(lines[1].ends_with(",1"));
    // Endpoint has type 0
    assert!(lines[2].ends_with(",0"));
}

#[test]
fn irbcam_dwell_refusal_when_configured() {
    let mut dwell_seg = make_seg(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
        SegmentKind::Dwell,
        false,
        0.0,
    );
    dwell_seg.dwell_s = Some(2.5);

    let toolpath = Toolpath {
        version: 0,
        meta: None,
        segments: vec![dwell_seg],
    };

    let params = EmitParams {
        flavor: FirmwareFlavor::Irbcam,
        irbcam_frame: IrbcamFrame {
            layout: IrbcamLayout::Json,
            rapid: RapidEncoding::MinusOne,
            dwell: DwellPolicy::Refuse,
            extrusion: ExtrusionCarry::Refuse,
            ..IrbcamFrame::default()
        },
        ..EmitParams::default()
    };

    let mut out = Vec::new();
    let err = emit_stream_to_writer(toolpath.segments.into_iter().map(Ok), &params, &mut out)
        .expect_err("must refuse dwell");
    match err {
        CodecError::Other(msg) => {
            assert!(msg.contains("dwell"));
        }
        other => panic!("expected dwell error, got {other:?}"),
    }
}

fn reference_segments() -> Vec<Segment> {
    let mut seg1 = make_seg(
        [0.0, 0.0, 0.0],
        [10.0, 0.0, 5.0],
        SegmentKind::Line,
        true,
        3000.0,
    );
    seg1.orientation = Some([0.0, 0.0, 1.0]);

    let mut seg2 = make_seg(
        [10.0, 0.0, 5.0],
        [20.0, 0.0, 5.0],
        SegmentKind::Line,
        false,
        1200.0,
    );
    seg2.orientation = Some([0.0, 0.0, 1.0]);

    let mut seg3 = make_seg(
        [20.0, 0.0, 5.0],
        [30.0, 10.0, 5.0],
        SegmentKind::Arc,
        false,
        1200.0,
    );
    seg3.centre = Some([Length::mm(20.0), Length::mm(10.0)]);
    seg3.orientation = Some([0.6, 0.0, 0.8]);
    seg3.clockwise = false;

    let mut seg4 = make_seg(
        [30.0, 10.0, 5.0],
        [30.0, 10.0, 5.0],
        SegmentKind::Dwell,
        false,
        0.0,
    );
    seg4.dwell_s = Some(1.5);

    let mut seg5 = make_seg(
        [30.0, 10.0, 5.0],
        [30.0, 20.0, 5.0],
        SegmentKind::Line,
        false,
        600.0,
    );
    seg5.orientation = Some([0.0, -1.0, 0.0]);

    vec![seg1, seg2, seg3, seg4, seg5]
}

#[test]
fn irbcam_and_apt_structural_goldens_do_not_drift() {
    let segs = reference_segments();

    // 1. IRBCAM JSON
    let json_params = EmitParams {
        flavor: FirmwareFlavor::Irbcam,
        irbcam_frame: IrbcamFrame {
            layout: IrbcamLayout::Json,
            rapid: RapidEncoding::MinusOne,
            dwell: DwellPolicy::Drop,
            extrusion: ExtrusionCarry::Refuse,
            decimals: 6,
            ..IrbcamFrame::default()
        },
        ..EmitParams::default()
    };
    let mut json_buf = Vec::new();
    emit_stream_to_writer(segs.iter().cloned().map(Ok), &json_params, &mut json_buf).unwrap();
    let json_str = String::from_utf8(json_buf).unwrap();
    let json_golden_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/reports/robot/reference-irbcam.json"
    );
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(json_golden_path, &json_str).unwrap();
    }
    let golden_json = std::fs::read_to_string(json_golden_path)
        .expect("reference-irbcam.json golden exists (run UPDATE_GOLDEN=1)");
    assert_eq!(json_str, golden_json);

    // 2. IRBCAM CSV
    let csv_params = EmitParams {
        flavor: FirmwareFlavor::IrbcamCsv,
        irbcam_frame: IrbcamFrame {
            layout: IrbcamLayout::Csv,
            rapid: RapidEncoding::MinusOne,
            dwell: DwellPolicy::Drop,
            extrusion: ExtrusionCarry::Refuse,
            decimals: 6,
            ..IrbcamFrame::default()
        },
        ..EmitParams::default()
    };
    let mut csv_buf = Vec::new();
    emit_stream_to_writer(segs.iter().cloned().map(Ok), &csv_params, &mut csv_buf).unwrap();
    let csv_str = String::from_utf8(csv_buf).unwrap();
    let csv_golden_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/reports/robot/reference-irbcam.csv"
    );
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(csv_golden_path, &csv_str).unwrap();
    }
    let golden_csv = std::fs::read_to_string(csv_golden_path)
        .expect("reference-irbcam.csv golden exists (run UPDATE_GOLDEN=1)");
    assert_eq!(csv_str, golden_csv);

    // 3. APT
    let apt_params = EmitParams {
        flavor: FirmwareFlavor::Apt,
        apt_frame: dry_core::emit::AptFrame {
            partno: Some("REFERENCE_ROBOT".to_string()),
            machin: Some("DRY_5AX".to_string()),
            decimals: 6,
        },
        ..EmitParams::default()
    };
    let mut apt_buf = Vec::new();
    emit_stream_to_writer(segs.iter().cloned().map(Ok), &apt_params, &mut apt_buf).unwrap();
    let apt_str = String::from_utf8(apt_buf).unwrap();
    let apt_golden_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/reports/robot/reference-apt.apt"
    );
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(apt_golden_path, &apt_str).unwrap();
    }
    let golden_apt = std::fs::read_to_string(apt_golden_path)
        .expect("reference-apt.apt golden exists (run UPDATE_GOLDEN=1)");
    assert_eq!(apt_str, golden_apt);
}

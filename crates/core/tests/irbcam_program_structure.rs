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

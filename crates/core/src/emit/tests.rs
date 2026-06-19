use super::gcode::num;

#[test]
fn number_format_matches_fullcontrol() {
    assert_eq!(num(1000.0), "1000");
    assert_eq!(num(0.2), "0.2");
    assert_eq!(num(0.0), "0");
    assert_eq!(num(0.498902), "0.498902");
    assert_eq!(num(10.0), "10");
    assert_eq!(num(-1.5), "-1.5");
}

#[test]
fn test_travel_g1_e0() {
    use super::{emit, EmitParams};
    use crate::ir::{Segment, SegmentKind, Toolpath};
    use crate::units::{Feedrate, Length, Volume};

    let tp = Toolpath {
        version: 0,
        meta: None,
        segments: vec![Segment {
            start: [None, None, None],
            end: [Some(Length::mm(10.0)), None, None],
            travel: true,
            speed: Feedrate(1000.0),
            length: Length::mm(10.0),
            volume: Volume::ZERO,
            filament: Length::ZERO,
            width: None,
            height: None,
            kind: SegmentKind::Line,
            centre: None,
            clockwise: false,
            temperature: None,
            fan: None,
            flow: None,
            tool: None,
            dwell_s: None,
            orientation: None,
            control_points: None,
        }],
    };

    let gcode_default = emit(&tp, &EmitParams::default());
    assert_eq!(gcode_default[0], "G0 F1000 X10");

    let gcode_e0 = emit(
        &tp,
        &EmitParams {
            travel_g1_e0: true,
            ..EmitParams::default()
        },
    );
    assert_eq!(gcode_e0[0], "G0 F1000 X10 E0");

    let gcode_abs_e0 = emit(
        &tp,
        &EmitParams {
            relative_e: false,
            travel_g1_e0: true,
            ..EmitParams::default()
        },
    );
    assert_eq!(gcode_abs_e0[0], "G0 F1000 X10 E0");
}

#[test]
fn travel_arcs_emit_arc_commands_without_extrusion() {
    use super::{emit, EmitParams};
    use crate::ir::{Segment, SegmentKind, Toolpath};
    use crate::units::{Feedrate, Length, Volume};

    let base = Segment {
        start: [None, None, None],
        end: [
            Some(Length::mm(10.0)),
            Some(Length::mm(0.0)),
            Some(Length::mm(0.2)),
        ],
        travel: true,
        speed: Feedrate(8000.0),
        length: Length::mm(10.0),
        volume: Volume::ZERO,
        filament: Length::ZERO,
        width: None,
        height: None,
        kind: SegmentKind::Line,
        centre: None,
        clockwise: false,
        temperature: None,
        fan: None,
        flow: None,
        tool: None,
        dwell_s: None,
        orientation: None,
        control_points: None,
    };
    let arc = Segment {
        start: base.end,
        end: [
            Some(Length::mm(0.0)),
            Some(Length::mm(10.0)),
            Some(Length::mm(0.2)),
        ],
        kind: SegmentKind::Arc,
        centre: Some([Length::mm(0.0), Length::mm(0.0)]),
        length: Length::mm(std::f64::consts::FRAC_PI_2 * 10.0),
        ..base.clone()
    };
    let tp = Toolpath {
        version: 0,
        meta: None,
        segments: vec![base, arc],
    };

    let gcode = emit(&tp, &EmitParams::default());
    assert_eq!(gcode[1], "G3 X0 Y10 I-10 J0");
    assert!(!gcode[1].contains('E'));
}

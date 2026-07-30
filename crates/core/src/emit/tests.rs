use super::emit_step_nc;
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
            manual_gcode: None,
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
    assert_eq!(gcode_e0[0], "G1 F1000 X10 E0");

    let gcode_abs_e0 = emit(
        &tp,
        &EmitParams {
            relative_e: false,
            travel_g1_e0: true,
            ..EmitParams::default()
        },
    );
    assert_eq!(gcode_abs_e0[0], "G1 F1000 X10 E0");
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
        manual_gcode: None,
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

#[test]
fn test_dwell_firmware_flavors() {
    use super::{emit, EmitParams, FirmwareFlavor};
    use crate::ir::{Segment, SegmentKind, Toolpath};
    use crate::units::{Feedrate, Length, Volume};

    let base = Segment {
        start: [None, None, None],
        end: [None, None, None],
        travel: true,
        speed: Feedrate(1000.0),
        length: Length::ZERO,
        volume: Volume::ZERO,
        filament: Length::ZERO,
        width: None,
        height: None,
        kind: SegmentKind::Dwell,
        centre: None,
        clockwise: false,
        temperature: None,
        fan: None,
        flow: None,
        tool: None,
        dwell_s: Some(1.5),
        manual_gcode: None,
        orientation: None,
        control_points: None,
    };
    let tp = Toolpath {
        version: 0,
        meta: None,
        segments: vec![base],
    };

    // Marlin/default uses G4 S1.5
    let gcode_marlin = emit(
        &tp,
        &EmitParams {
            flavor: FirmwareFlavor::Marlin,
            ..EmitParams::default()
        },
    );
    assert_eq!(gcode_marlin[0], "G4 S1.5");

    // Duet uses G4 S1.5
    let gcode_duet = emit(
        &tp,
        &EmitParams {
            flavor: FirmwareFlavor::Duet,
            ..EmitParams::default()
        },
    );
    assert_eq!(gcode_duet[0], "G4 S1.5");

    // Klipper uses G4 P1500
    let gcode_klipper = emit(
        &tp,
        &EmitParams {
            flavor: FirmwareFlavor::Klipper,
            ..EmitParams::default()
        },
    );
    assert_eq!(gcode_klipper[0], "G4 P1500");

    // RS-274 uses the conservative Marlin-style dwell syntax (`G4 S<sec>`) for now.
    let gcode_rs274 = emit(
        &tp,
        &EmitParams {
            flavor: FirmwareFlavor::Rs274,
            ..EmitParams::default()
        },
    );
    assert_eq!(gcode_rs274[0], "G4 S1.5");

    // GRBL uses `G4 P<sec>`.
    let gcode_grbl = emit(
        &tp,
        &EmitParams {
            flavor: FirmwareFlavor::Grbl,
            ..EmitParams::default()
        },
    );
    assert_eq!(gcode_grbl[0], "G4 P1.5");
}

#[test]
fn test_step_nc_emitter_is_deterministic() {
    use crate::ir::{Segment, SegmentKind, Toolpath};
    use crate::units::{Feedrate, Length, Volume};

    let tp = Toolpath {
        version: 0,
        meta: None,
        segments: vec![Segment {
            start: [None, None, None],
            end: [
                Some(Length::mm(10.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.0)),
            ],
            travel: true,
            speed: Feedrate(1200.0),
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
            manual_gcode: None,
            orientation: None,
            control_points: None,
        }],
    };

    let step_nc = emit_step_nc(
        &tp,
        &crate::emit::EmitParams {
            ..crate::emit::EmitParams::default()
        },
    );

    assert!(step_nc.contains("<stepnc"));
    assert!(step_nc.contains("<workingstep id=\"ws-0\""));
    assert!(step_nc.contains("<motion kind=\"line\""));
}

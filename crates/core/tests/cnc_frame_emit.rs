//! P5.3 — RS-274 program frame. `CncFrame` (Task 4) carries the work-coordinate system, tool,
//! spindle speed, and coolant state from `MachineProfile::cnc` onto `EmitParams`. The RS-274
//! emitter (Task 5) brackets the motion stream with a preamble (`G21 G17 G90`, `Gxx`, `Tn M6`,
//! `Sxxxx M3`, `M8`) and a postamble (`M9`, `M5`, `M30`) when a frame is present. Absent a frame,
//! or for any other firmware flavor, output must stay byte-identical to before this task.

use dry_core::{emit, resolve, CncFrame, Design, EmitParams, FirmwareFlavor, ResolveParams};

fn tiny_design() -> Design {
    serde_json::from_str(
        r#"{"ops":[
        {"op":"geometry","width":6.0,"height":2.0},
        {"op":"extruder","on":true},
        {"op":"speed","print":300},
        {"op":"move","x":0,"y":0,"z":-1},
        {"op":"move","x":10,"y":0,"z":-1}
    ]}"#,
    )
    .unwrap()
}

fn frame() -> CncFrame {
    CncFrame {
        wcs: Some(55),
        tool: Some(3),
        spindle_rpm: Some(12000.0),
        coolant: Some(true),
    }
}

#[test]
fn rs274_frame_brackets_the_program() {
    let tp = resolve(&tiny_design(), &ResolveParams::default());
    let p = EmitParams {
        flavor: FirmwareFlavor::Rs274,
        cnc_frame: Some(frame()),
        ..EmitParams::default()
    };
    let lines = emit(&tp, &p);
    let head: Vec<&str> = lines.iter().take(5).map(String::as_str).collect();
    assert_eq!(head, vec!["G21 G17 G90", "G55", "T3 M6", "S12000 M3", "M8"]);
    let n = lines.len();
    assert_eq!(
        &lines[n - 3..],
        &["M9".to_string(), "M5".to_string(), "M30".to_string()]
    );
}

#[test]
fn minimal_frame_omits_optional_words() {
    let tp = resolve(&tiny_design(), &ResolveParams::default());
    let p = EmitParams {
        flavor: FirmwareFlavor::Rs274,
        cnc_frame: Some(CncFrame::default()),
        ..EmitParams::default()
    };
    let lines = emit(&tp, &p);
    assert_eq!(&lines[..2], &["G21 G17 G90".to_string(), "G54".to_string()]);
    assert!(!lines.iter().any(|l| l.starts_with('T')
        || l.starts_with('S')
        || l == "M8"
        || l == "M9"
        || l == "M5"));
    assert_eq!(lines.last().unwrap(), "M30");
}

#[test]
fn no_frame_or_non_rs274_flavor_is_byte_identical_to_before() {
    let tp = resolve(&tiny_design(), &ResolveParams::default());
    let bare = emit(
        &tp,
        &EmitParams {
            flavor: FirmwareFlavor::Rs274,
            ..EmitParams::default()
        },
    );
    assert!(
        !bare.iter().any(|l| l == "G21 G17 G90" || l == "M30"),
        "None frame must not add lines"
    );
    let marlin_with_frame = emit(
        &tp,
        &EmitParams {
            flavor: FirmwareFlavor::Marlin,
            cnc_frame: Some(frame()),
            ..EmitParams::default()
        },
    );
    let marlin_bare = emit(&tp, &EmitParams::default());
    assert_eq!(
        marlin_with_frame, marlin_bare,
        "non-rs274 flavors ignore cnc_frame in this slice"
    );
}

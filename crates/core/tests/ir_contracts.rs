use dry_core::{resolve_checked, Design, ResolveParams, SegmentKind, Toolpath};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

#[test]
fn segment_kind_rejects_unknown_json_values() {
    let json = r#"{"version":0,"segments":[{"start":[null,null,null],"end":[1,null,null],
        "travel":true,"speed":1000,"length":1,"volume":0,"filament":0,"kind":"curve"}]}"#;
    let err = Toolpath::from_json(json).expect_err("unknown segment kind should fail");
    assert!(err.to_string().contains("unknown variant"));
}

#[test]
fn segment_kind_round_trips_as_lowercase_wire_string() {
    let json = r#"{"version":0,"segments":[{"start":[null,null,null],"end":[1,null,null],
        "travel":true,"speed":1000,"length":1,"volume":0,"filament":0,"kind":"line"}]}"#;
    let tp = Toolpath::from_json(json).unwrap();
    assert_eq!(tp.segments[0].kind, SegmentKind::Line);
    assert!(tp.to_json().contains(r#""kind":"line""#));
}

#[test]
fn resolve_checked_rejects_invalid_physical_inputs() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"fan","speed":255},
            {"op":"extruder","on":true},{"op":"move","x":0,"y":0,"z":0.2}]"#,
    );
    let err = resolve_checked(&d, &ResolveParams::default()).expect_err("fan is a 0..1 ratio");
    assert!(err.to_string().contains("0..1"));

    let d = design(r#"[{"op":"geometry","width":0,"height":0.2}]"#);
    let err = resolve_checked(&d, &ResolveParams::default()).expect_err("zero width is invalid");
    assert!(err.to_string().contains("width"));
}

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

/// `to_json` → `from_json` is bit-exact, which the IR v0 spec (`docs/10`, §9) requires: it defines
/// semantic equality as exact f64 bit-equality of every numeric quantity, so a reader that came back
/// one ULP off would not conform to a vector the same engine wrote.
///
/// This does not hold for free. `serde_json`'s default float parser is accurate only to within 1 ULP;
/// the bit-exact one is behind its `float_roundtrip` feature, which `crates/core/Cargo.toml` therefore
/// enables. Each value below is a real `resolve` output (the segment lengths and deposited volumes of
/// `conformance/vectors/five_axis_drape`) and each one decoded to the wrong neighbouring double before
/// that feature was turned on — so if the feature is ever dropped, this fires here rather than as an
/// unexplained corpus drift.
#[test]
fn json_floats_round_trip_bit_exactly() {
    for value in [
        11.045361017187261_f64,
        1.5484185480676727,
        1.2727922061357855,
        8.602325267042627,
        0.8265829478963671,
    ] {
        let json = format!(
            r#"{{"version":0,"segments":[{{"start":[null,null,null],"end":[{value},null,null],
            "travel":true,"speed":1000,"length":{value},"volume":0,"filament":0}}]}}"#
        );
        let tp = Toolpath::from_json(&json).expect("parses");
        assert_eq!(
            tp.segments[0].length.0.to_bits(),
            value.to_bits(),
            "{value} decoded to {} — serde_json's `float_roundtrip` feature is off",
            tp.segments[0].length.0
        );
        let back = Toolpath::from_json(&tp.to_json()).expect("re-parses");
        assert_eq!(back, tp, "toolpath JSON round-trip is not bit-exact");
    }
}

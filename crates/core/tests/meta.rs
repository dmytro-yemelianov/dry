//! P0.3+ — the self-describing IR header: optional provenance + declared invariants (`Meta`),
//! carried losslessly through both the JSON and the binary encodings. The byte-identity guard:
//! a toolpath WITHOUT a header must serialise exactly as before (no `meta` key).

use dry_core::{resolve, Design, Meta, ResolveParams, Toolpath};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

fn small_path() -> Toolpath {
    resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":5,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    )
}

#[test]
fn meta_round_trips_through_json_and_bytes() {
    let mut tp = small_path();
    tp.meta = Some(Meta {
        generator: Some("dry test".to_string()),
        units: Some("mm".to_string()),
        source_hash: Some("deadbeef".to_string()),
        invariants: vec!["bounds".to_string()],
    });

    let via_json = Toolpath::from_json(&tp.to_json()).unwrap();
    assert_eq!(via_json, tp);

    let via_bytes = Toolpath::from_bytes(&tp.to_bytes()).unwrap();
    assert_eq!(via_bytes, tp);
}

#[test]
fn no_meta_is_byte_identical_and_round_trips() {
    let tp = small_path();
    assert!(tp.meta.is_none());

    let json = tp.to_json();
    assert!(
        !json.contains("meta"),
        "a meta-less toolpath must not emit a meta key: {json}"
    );

    assert_eq!(Toolpath::from_json(&json).unwrap(), tp);
    assert_eq!(Toolpath::from_bytes(&tp.to_bytes()).unwrap(), tp);
}

#[test]
fn resolved_path_has_no_meta_by_default() {
    let tp = small_path();
    assert!(tp.meta.is_none());
    // and still round-trips both ways with the default (absent) header.
    assert_eq!(Toolpath::from_json(&tp.to_json()).unwrap(), tp);
    assert_eq!(Toolpath::from_bytes(&tp.to_bytes()).unwrap(), tp);
}

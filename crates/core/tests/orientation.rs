//! P2.x — the toolframe **orientation**: each motion carries a tool-direction unit vector (i,j,k);
//! `None` means identity (+Z), so 3-axis is the default and motion-only IR is unchanged. Orientation
//! makes non-planar / 5-axis a first-class IR property (the 5-axis *target lowering* is a later slice).

use dry_core::{resolve, verify, Contracts, Design, ResolveParams, Toolpath};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

#[test]
fn orientation_propagates_onto_segments() {
    // a tilted tool (36.87° from +Z): unit vector [0.6, 0, 0.8].
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"orient","i":0.6,"j":0.0,"k":0.8},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    assert_eq!(tp.segments[1].orientation, Some([0.6, 0.0, 0.8]));
}

#[test]
fn default_orientation_is_none_three_axis() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    assert_eq!(tp.segments[1].orientation, None);
}

#[test]
fn non_unit_orientation_is_flagged() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"orient","i":0.0,"j":0.0,"k":2.0},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let report = verify(&tp, &Contracts::default());
    assert!(report
        .findings
        .iter()
        .any(|f| f.rule == "orientation-not-unit"));
}

#[test]
fn unit_orientation_passes_verify() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"orient","i":0.6,"j":0.0,"k":0.8},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    assert!(!verify(&tp, &Contracts::default())
        .findings
        .iter()
        .any(|f| f.rule == "orientation-not-unit"));
}

#[test]
fn codec_round_trips_orientation() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"orient","i":0.6,"j":0.0,"k":0.8},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    assert_eq!(Toolpath::from_bytes(&tp.to_bytes()).unwrap(), tp);
}

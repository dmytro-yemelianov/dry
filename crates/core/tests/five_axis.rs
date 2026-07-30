//! P2.x — the 5-axis target emitter: when `five_axis` is set, the toolframe orientation drives
//! rotary `A`/`B`/`C` words (degrees). With default `EmitParams` this maps using the AB model
//! (`A = atan2(j, hypot(i, k))`, `B = atan2(i, k)`); the test remains valid for a single default
//! AB-path check, while profile/flag overrides are validated elsewhere.

use dry_core::{emit, resolve, Design, EmitParams, ResolveParams};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

fn five_axis() -> EmitParams {
    EmitParams {
        relative_e: true,
        travel_g1_e0: false,
        five_axis: true,
        ..EmitParams::default()
    }
}

#[test]
fn orientation_drives_a_b_words() {
    // tool tilted 36.87° toward +X: v = [0.6, 0, 0.8] → A = 0°, B = acos(0.8) = 36.869898°.
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"orient","i":0.6,"j":0.0,"k":0.8},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    let g = emit(&tp, &five_axis());
    assert!(
        g.iter().any(|l| l.contains("A0")),
        "expected A angle: {g:?}"
    );
    assert!(
        g.iter().any(|l| l.contains("B36.869898")),
        "expected B angle: {g:?}"
    );
}

#[test]
fn cardinal_tilts_map_to_90_degrees() {
    // tool along +X → A0 + B90; tool along +Y → A90.
    let bx = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"orient","i":1.0,"j":0.0,"k":0.0},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    let bx_out = emit(&bx, &five_axis());
    assert!(bx_out.iter().any(|l| l.contains("A0")));
    assert!(bx_out.iter().any(|l| l.contains("B90")));

    let ay = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"orient","i":0.0,"j":1.0,"k":0.0},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    let ay_out = emit(&ay, &five_axis());
    assert!(ay_out.iter().any(|l| l.contains("A90")));
    assert!(ay_out.iter().any(|l| l.contains("B0")));
}

#[test]
fn default_emit_is_three_axis_no_rotary() {
    // without five_axis, orientation is dropped — no A/B words at all.
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"orient","i":0.6,"j":0.0,"k":0.8},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    let g = emit(&tp, &EmitParams::default());
    assert!(
        g.iter().all(|l| !l.contains(" A") && !l.contains(" B")),
        "3-axis emit must carry no rotary words: {g:?}"
    );
}

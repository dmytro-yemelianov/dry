//! P2.x — the 5-axis target emitter: when `five_axis` is set, the toolframe orientation drives rotary
//! `A`/`B` words (degrees), derived from the tool-direction vector by a documented AB-head convention:
//! `B = atan2(i, k)` (lead in X-Z), `A = atan2(j, hypot(i, k))` (tilt toward Y). The default emit is
//! 3-axis (orientation dropped), so motion g-code is byte-identical to the oracle.

use dry_core::{emit, resolve, Design, EmitParams, ResolveParams};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

fn five_axis() -> EmitParams {
    EmitParams {
        relative_e: true,
        travel_g1_e0: false,
        five_axis: true,
    }
}

#[test]
fn orientation_drives_a_b_words() {
    // tool tilted 36.87° toward +X: v = [0.6, 0, 0.8] → B = atan2(0.6, 0.8) = 36.869898°, A = 0.
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
        g.iter().any(|l| l.contains("B36.869898")),
        "expected B angle: {g:?}"
    );
    assert!(g.iter().any(|l| l.contains("A0")), "expected A0: {g:?}");
}

#[test]
fn cardinal_tilts_map_to_90_degrees() {
    // tool along +X → B90; tool along +Y → A90.
    let bx = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"orient","i":1.0,"j":0.0,"k":0.0},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    assert!(emit(&bx, &five_axis()).iter().any(|l| l.contains("B90")));

    let ay = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"orient","i":0.0,"j":1.0,"k":0.0},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    assert!(emit(&ay, &five_axis()).iter().any(|l| l.contains("A90")));
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

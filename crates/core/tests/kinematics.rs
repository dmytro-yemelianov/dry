//! P2.x — selectable rotary kinematics for the 5-axis emitter. `EmitParams.kinematics` chooses how a
//! toolframe orientation (tool-direction unit vector) maps to two rotary words, each emitted only when
//! it changes:
//!
//! - **AB** (tilting head, default): `B = atan2(i, k)`, `A = atan2(j, hypot(i, k))` → words `A`,`B`.
//! - **AC** (A about X, C about Z): `C = atan2(j, i)`, `A = acos(k)` → words `A`,`C`.
//! - **BC** (B about Y, C about Z): `C = atan2(j, i)`, `B = acos(k)` → words `B`,`C`.
//!
//! Default kinematics is AB, so the default emit is byte-identical to the existing behaviour.

use dry_core::{emit, resolve, Design, EmitParams, Kinematics, ResolveParams};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

fn params(five_axis: bool, kinematics: Kinematics) -> EmitParams {
    EmitParams {
        relative_e: true,
        travel_g1_e0: false,
        five_axis,
        kinematics,
    }
}

/// Resolve a single oriented move along +X and emit g-code with the given kinematics.
fn oriented(i: f64, j: f64, k: f64, kinematics: Kinematics) -> Vec<String> {
    let tp = resolve(
        &design(&format!(
            r#"[{{"op":"geometry","width":0.6,"height":0.2}},{{"op":"extruder","on":true}},
                {{"op":"orient","i":{i},"j":{j},"k":{k}}},
                {{"op":"move","x":0,"y":0,"z":0.2}},{{"op":"move","x":10,"y":0,"z":0.2}}]"#,
        )),
        &ResolveParams::default(),
    );
    emit(&tp, &params(true, kinematics))
}

fn has(g: &[String], word: &str) -> bool {
    g.iter().any(|l| l.split(' ').any(|w| w == word))
}

#[test]
fn default_kinematics_is_ab() {
    assert_eq!(Kinematics::default(), Kinematics::Ab);
    assert_eq!(EmitParams::default().kinematics, Kinematics::Ab);
}

#[test]
fn tool_plus_x() {
    // [1,0,0]: AB → B90; AC → C0 A90; BC → C0 B90.
    let ab = oriented(1.0, 0.0, 0.0, Kinematics::Ab);
    assert!(has(&ab, "B90"), "AB +X expected B90: {ab:?}");
    assert!(has(&ab, "A0"), "AB +X expected A0: {ab:?}");

    let ac = oriented(1.0, 0.0, 0.0, Kinematics::Ac);
    assert!(has(&ac, "C0"), "AC +X expected C0: {ac:?}");
    assert!(has(&ac, "A90"), "AC +X expected A90: {ac:?}");

    let bc = oriented(1.0, 0.0, 0.0, Kinematics::Bc);
    assert!(has(&bc, "C0"), "BC +X expected C0: {bc:?}");
    assert!(has(&bc, "B90"), "BC +X expected B90: {bc:?}");
}

#[test]
fn tool_plus_y() {
    // [0,1,0]: AB → A90; AC → C90 A90; BC → C90 B90.
    let ab = oriented(0.0, 1.0, 0.0, Kinematics::Ab);
    assert!(has(&ab, "A90"), "AB +Y expected A90: {ab:?}");

    let ac = oriented(0.0, 1.0, 0.0, Kinematics::Ac);
    assert!(has(&ac, "C90"), "AC +Y expected C90: {ac:?}");
    assert!(has(&ac, "A90"), "AC +Y expected A90: {ac:?}");

    let bc = oriented(0.0, 1.0, 0.0, Kinematics::Bc);
    assert!(has(&bc, "C90"), "BC +Y expected C90: {bc:?}");
    assert!(has(&bc, "B90"), "BC +Y expected B90: {bc:?}");
}

#[test]
fn tool_tilted_ab() {
    // [0.6,0,0.8]: AB → B36.869898, A0 (unchanged existing behaviour).
    let ab = oriented(0.6, 0.0, 0.8, Kinematics::Ab);
    assert!(
        has(&ab, "B36.869898"),
        "AB tilt expected B36.869898: {ab:?}"
    );
    assert!(has(&ab, "A0"), "AB tilt expected A0: {ab:?}");
}

#[test]
fn tool_plus_z_is_all_zeros() {
    // identity (no orient) → +Z → every kinematics emits zero rotary words.
    for k in [Kinematics::Ab, Kinematics::Ac, Kinematics::Bc] {
        let tp = resolve(
            &design(
                r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                    {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
            ),
            &ResolveParams::default(),
        );
        let g = emit(&tp, &params(true, k));
        // the first move carries the (changed-from-none) zero words; assert they are exactly zero.
        match k {
            Kinematics::Ab => {
                assert!(
                    has(&g, "A0") && has(&g, "B0"),
                    "AB +Z expected A0 B0: {g:?}"
                );
            }
            Kinematics::Ac => {
                assert!(
                    has(&g, "A0") && has(&g, "C0"),
                    "AC +Z expected A0 C0: {g:?}"
                );
            }
            Kinematics::Bc => {
                assert!(
                    has(&g, "B0") && has(&g, "C0"),
                    "BC +Z expected B0 C0: {g:?}"
                );
            }
        }
    }
}

#[test]
fn default_emit_byte_identical_to_explicit_ab() {
    // an oriented design emitted with the default EmitParams (AB) must equal the explicit-AB output.
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"orient","i":0.6,"j":0.0,"k":0.8},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    let def = EmitParams {
        five_axis: true,
        ..EmitParams::default()
    };
    let explicit = params(true, Kinematics::Ab);
    assert_eq!(emit(&tp, &def), emit(&tp, &explicit));
}

#[test]
fn three_axis_emits_no_rotary() {
    // five_axis off ⇒ no rotary words regardless of kinematics.
    for k in [Kinematics::Ab, Kinematics::Ac, Kinematics::Bc] {
        let tp = resolve(
            &design(
                r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                    {"op":"orient","i":0.6,"j":0.0,"k":0.8},
                    {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
            ),
            &ResolveParams::default(),
        );
        let g = emit(&tp, &params(false, k));
        assert!(
            g.iter().all(|l| {
                l.split(' ')
                    .all(|w| !w.starts_with('A') && !w.starts_with('B') && !w.starts_with('C'))
            }),
            "3-axis must carry no rotary words ({k:?}): {g:?}"
        );
    }
}

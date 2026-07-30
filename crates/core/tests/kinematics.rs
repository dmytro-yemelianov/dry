//! P2.x — selectable rotary kinematics for the 5-axis emitter. `EmitParams.kinematics` chooses how a
//! toolframe orientation (tool-direction unit vector) maps to two rotary words, each emitted only when
//! it changes:
//!
//! - **AB** (tilting head, default): `B = atan2(i, k)`, `A = atan2(j, hypot(i, k))` → words `A`,`B`.
//! - **AC** (A about X, C about Z): `C = atan2(j, i)`, `A = acos(k)` → words `A`,`C`.
//! - **BC** (B about Y, C about Z): `C = atan2(j, i)`, `B = acos(k)` → words `B`,`C`.
//!
//! Default kinematics is AB, so the default emit is byte-identical to the existing behaviour.

use dry_core::{
    parse_gcode_lines, emit, import_gcode, resolve, GcodeImportParams, GcodeRecord, ResolveParams,
    Design, EmitParams, Kinematics, REFERENCE_FIVE_AXIS_MACHINE,
};

fn ab() -> Kinematics {
    Kinematics::Ab {
        pivot_offset: [0.0, 0.0, 0.0],
        rotary_offset: [0.0, 0.0],
    }
}
fn ac() -> Kinematics {
    Kinematics::Ac {
        pivot_offset: [0.0, 0.0, 0.0],
        rotary_offset: [0.0, 0.0],
    }
}
fn bc() -> Kinematics {
    Kinematics::Bc {
        pivot_offset: [0.0, 0.0, 0.0],
        rotary_offset: [0.0, 0.0],
    }
}

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

fn params(five_axis: bool, kinematics: Kinematics) -> EmitParams {
    EmitParams {
        relative_e: true,
        travel_g1_e0: false,
        five_axis,
        kinematics,
        ..EmitParams::default()
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
    assert_eq!(Kinematics::default(), ab());
    assert_eq!(EmitParams::default().kinematics, ab());
}

#[test]
fn reference_five_axis_machine_is_bc_and_emits_5_axis_motion() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"orient","i":1.0,"j":0.0,"k":0.0},
                {"op":"move","x":0,"y":0,"z":0.2},
                {"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    let g = emit(
        &tp,
        &EmitParams {
            five_axis: true,
            kinematics: REFERENCE_FIVE_AXIS_MACHINE,
            ..EmitParams::default()
        },
    );

    assert_eq!(
        REFERENCE_FIVE_AXIS_MACHINE,
        Kinematics::Bc {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [0.0, 0.0]
        }
    );
    assert!(has(&g, "B90"));
    assert!(has(&g, "C0"));
}

#[test]
fn reference_five_axis_emission_is_parseable_and_importable() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},
                {"op":"extruder","on":true},
                {"op":"orient","i":1.0,"j":0.0,"k":0.0},
                {"op":"move","x":10.0,"y":0.0,"z":0.2},
                {"op":"orient","i":0.0,"j":1.0,"k":0.0},
                {"op":"move","x":10.0,"y":10.0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    let g = emit(
        &tp,
        &EmitParams {
            five_axis: true,
            kinematics: REFERENCE_FIVE_AXIS_MACHINE,
            ..EmitParams::default()
        },
    );
    let parsed = parse_gcode_lines(&g.join("\n")).unwrap();
    let motion_count = parsed
        .iter()
        .filter(|line| matches!(line.record, GcodeRecord::Motion(_)))
        .count();
    assert_eq!(motion_count, 2);
    assert!(g.iter().any(|line| line.contains("B90")));
    assert!(g.iter().any(|line| line.contains("C90")));

    let imported = import_gcode(
        &g.join("\n"),
        &GcodeImportParams {
            relative_e: false,
            ..GcodeImportParams::default()
        },
    )
    .unwrap();
    assert_eq!(imported.segments.len(), 2);
    assert!(imported
        .segments
        .iter()
        .all(|segment| segment.speed.0 > 0.0));
}

#[test]
fn tool_plus_x() {
    // [1,0,0]: AB → B90; AC → C0 A90; BC → C0 B90.
    let ab = oriented(1.0, 0.0, 0.0, ab());
    assert!(has(&ab, "B90"), "AB +X expected B90: {ab:?}");
    assert!(has(&ab, "A0"), "AB +X expected A0: {ab:?}");

    let ac = oriented(1.0, 0.0, 0.0, ac());
    assert!(has(&ac, "C0"), "AC +X expected C0: {ac:?}");
    assert!(has(&ac, "A90"), "AC +X expected A90: {ac:?}");

    let bc = oriented(1.0, 0.0, 0.0, bc());
    assert!(has(&bc, "C0"), "BC +X expected C0: {bc:?}");
    assert!(has(&bc, "B90"), "BC +X expected B90: {bc:?}");
}

#[test]
fn tool_plus_y() {
    // [0,1,0]: AB → A90; AC → C90 A90; BC → C90 B90.
    let ab = oriented(0.0, 1.0, 0.0, ab());
    assert!(has(&ab, "A90"), "AB +Y expected A90: {ab:?}");

    let ac = oriented(0.0, 1.0, 0.0, ac());
    assert!(has(&ac, "C90"), "AC +Y expected C90: {ac:?}");
    assert!(has(&ac, "A90"), "AC +Y expected A90: {ac:?}");

    let bc = oriented(0.0, 1.0, 0.0, bc());
    assert!(has(&bc, "C90"), "BC +Y expected C90: {bc:?}");
    assert!(has(&bc, "B90"), "BC +Y expected B90: {bc:?}");
}

#[test]
fn tool_tilted_ab() {
    // [0.6,0,0.8]: AB → B36.869898, A0 (unchanged existing behaviour).
    let ab = oriented(0.6, 0.0, 0.8, ab());
    assert!(
        has(&ab, "B36.869898"),
        "AB tilt expected B36.869898: {ab:?}"
    );
    assert!(has(&ab, "A0"), "AB tilt expected A0: {ab:?}");
}

#[test]
fn tool_plus_z_is_all_zeros() {
    // identity (no orient) → +Z → every kinematics emits zero rotary words.
    for k in [ab(), ac(), bc()] {
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
            Kinematics::Ab { .. } => {
                assert!(
                    has(&g, "A0") && has(&g, "B0"),
                    "AB +Z expected A0 B0: {g:?}"
                );
            }
            Kinematics::Ac { .. } => {
                assert!(
                    has(&g, "A0") && has(&g, "C0"),
                    "AC +Z expected A0 C0: {g:?}"
                );
            }
            Kinematics::Bc { .. } => {
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
    let explicit = params(true, ab());
    assert_eq!(emit(&tp, &def), emit(&tp, &explicit));
}

#[test]
fn three_axis_emits_no_rotary() {
    // five_axis off ⇒ no rotary words regardless of kinematics.
    for k in [ab(), ac(), bc()] {
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

#[test]
fn test_serde_kinematics_config() {
    // 1. String forms
    let ab_str: Kinematics = serde_json::from_str("\"ab\"").unwrap();
    assert_eq!(ab_str, ab());

    // 2. Struct forms with tag
    let ab_obj: Kinematics = serde_json::from_str(
        r#"{"type": "ab", "pivot_offset": [1.0, 2.0, 3.0], "rotary_offset": [4.0, 5.0]}"#,
    )
    .unwrap();
    assert_eq!(
        ab_obj,
        Kinematics::Ab {
            pivot_offset: [1.0, 2.0, 3.0],
            rotary_offset: [4.0, 5.0],
        }
    );

    // 3. Optional fields defaulted
    let ac_obj_default: Kinematics = serde_json::from_str(r#"{"type": "ac"}"#).unwrap();
    assert_eq!(ac_obj_default, ac());
}

#[test]
fn test_ab_tilting_head_with_pivot_offset() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"orient","i":0.0,"j":1.0,"k":0.0},
                {"op":"move","x":10,"y":20,"z":30}]"#,
        ),
        &ResolveParams::default(),
    );
    let p = EmitParams {
        five_axis: true,
        kinematics: Kinematics::Ab {
            pivot_offset: [0.0, 0.0, -100.0],
            rotary_offset: [0.0, 0.0],
        },
        ..EmitParams::default()
    };
    let g = emit(&tp, &p);
    assert!(has(&g, "X10"), "expected X10: {g:?}");
    assert!(has(&g, "Y120"), "expected Y120: {g:?}");
    assert!(has(&g, "Z30"), "expected Z30: {g:?}");
    assert!(has(&g, "A90"), "expected A90: {g:?}");
    assert!(has(&g, "B0"), "expected B0: {g:?}");
}

#[test]
fn test_ac_table_with_pivot_offset() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"orient","i":1.0,"j":0.0,"k":0.0},
                {"op":"move","x":10,"y":0,"z":0}]"#,
        ),
        &ResolveParams::default(),
    );
    let p = EmitParams {
        five_axis: true,
        kinematics: Kinematics::Ac {
            pivot_offset: [0.0, 0.0, -50.0],
            rotary_offset: [0.0, 0.0],
        },
        ..EmitParams::default()
    };
    let g = emit(&tp, &p);
    assert!(has(&g, "X10"), "expected X10: {g:?}");
    assert!(has(&g, "Y50"), "expected Y50: {g:?}");
    assert!(has(&g, "Z50"), "expected Z50: {g:?}");
    assert!(has(&g, "A90"), "expected A90: {g:?}");
    assert!(has(&g, "C0"), "expected C0: {g:?}");
}

#[test]
fn test_bc_table_with_pivot_offset() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"orient","i":1.0,"j":0.0,"k":0.0},
                {"op":"move","x":10,"y":0,"z":0}]"#,
        ),
        &ResolveParams::default(),
    );
    let p = EmitParams {
        five_axis: true,
        kinematics: Kinematics::Bc {
            pivot_offset: [0.0, 0.0, -50.0],
            rotary_offset: [0.0, 0.0],
        },
        ..EmitParams::default()
    };
    let g = emit(&tp, &p);
    assert!(has(&g, "X-50"), "expected X-50: {g:?}");
    assert!(has(&g, "Y0"), "expected Y0: {g:?}");
    assert!(has(&g, "Z40"), "expected Z40: {g:?}");
    assert!(has(&g, "B90"), "expected B90: {g:?}");
    assert!(has(&g, "C0"), "expected C0: {g:?}");
}

#[test]
fn test_rotary_joint_offset() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"orient","i":0.0,"j":0.0,"k":1.0},
                {"op":"move","x":10,"y":20,"z":30}]"#,
        ),
        &ResolveParams::default(),
    );
    let p = EmitParams {
        five_axis: true,
        kinematics: Kinematics::Ab {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [10.0, -5.0],
        },
        ..EmitParams::default()
    };
    let g = emit(&tp, &p);
    assert!(has(&g, "A10"), "expected A10: {g:?}");
    assert!(has(&g, "B-5"), "expected B-5: {g:?}");
}

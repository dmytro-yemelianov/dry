//! P5.2 — the toolframe orientation survives `emit` → `import_gcode`.
//!
//! `emit` maps a tool-direction unit vector onto two rotary words under a kinematic model
//! (`emit/kinematics.rs::rotary_words`); the importer inverts that map. The words alone do not say
//! which model wrote them — `A`+`B` is `Ab`, `A`+`C` is `Ac`, `B`+`C` is `Bc`, and a machine writing
//! `A` could be any of the first two — so the model is an import *input*, supplied the same way the
//! emitter gets it (a profile's `machine.five_axis` → `GcodeImportParams::kinematics`). A program
//! that states rotary words with no model is refused, not imported with its orientation dropped:
//! ADR 0002 §4.
//!
//! **Off the singular cone, on purpose.** Where `|k| = 1` the tool points along the rotary axis it is
//! symmetric about and the second word is not recoverable *on the way out* (`atan2(0, 0)`), which is a
//! property of `rotary_words` and the subject of a separate slice. Every orientation below has
//! `hypot(i, j) > 0`, so what these tests measure is this importer's inverse, not that loss.

// The deprecated infallible `emit()` is what the in-tree call sites use; the round-trip under test is
// the one they get.
#![allow(deprecated)]

use dry_core::{
    emit, import_gcode, resolve, verify, Contracts, Design, EmitParams, GcodeImportParams,
    Kinematics, ResolveParams, Toolpath,
};

/// Round-trip tolerance for a recovered orientation **component**.
///
/// This is a test bound, not a production predicate: no tolerance is compared anywhere in the import
/// path (the refusals are structural — a word is present or it is not; a model was supplied or it was
/// not), so there is no epsilon here for `proofs/` to name under ADR 0001.
///
/// It is set by the emitter, not by the inverse. Rotary words are written `{:.6}` **degrees**, so a
/// word carries at most 5e-7° = 8.7e-9 rad of rounding, and a unit vector's components move by at
/// most that much. Measured worst case over the matrix below: 6.61e-9. The pure-trig inverse, with no
/// g-code in between, is exact to 1.67e-16 — pinned separately in
/// `emit::kinematics::tests::rotary_words_invert_back_to_the_orientation_they_came_from`.
const ROUND_TRIP_TOL: f64 = 1e-7;

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

fn models() -> [Kinematics; 3] {
    [
        Kinematics::Ab {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [0.0, 0.0],
        },
        Kinematics::Ac {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [0.0, 0.0],
        },
        Kinematics::Bc {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [0.0, 0.0],
        },
    ]
}

/// A single extruding move carrying one orientation.
fn oriented_toolpath(o: [f64; 3]) -> Toolpath {
    resolve(
        &design(&format!(
            r#"[{{"op":"geometry","width":0.6,"height":0.2}},{{"op":"extruder","on":true}},
                {{"op":"orient","i":{},"j":{},"k":{}}},
                {{"op":"move","x":0,"y":0,"z":0.2}},{{"op":"move","x":10,"y":0,"z":0.2}}]"#,
            o[0], o[1], o[2]
        )),
        &ResolveParams::default(),
    )
}

fn emit_five_axis(tp: &Toolpath, kinematics: Kinematics) -> Vec<String> {
    emit(
        tp,
        &EmitParams {
            five_axis: true,
            kinematics,
            ..EmitParams::default()
        },
    )
}

fn import_five_axis(gcode: &[String], kinematics: Option<Kinematics>) -> Vec<Option<[f64; 3]>> {
    import_gcode(
        &gcode.join("\n"),
        &GcodeImportParams {
            kinematics,
            ..GcodeImportParams::default()
        },
    )
    .unwrap_or_else(|error| panic!("import refused {gcode:?}: {error}"))
    .segments
    .iter()
    .map(|segment| segment.orientation)
    .collect()
}

fn assert_close(got: [f64; 3], want: [f64; 3], what: &str) {
    for axis in 0..3 {
        assert!(
            (got[axis] - want[axis]).abs() < ROUND_TRIP_TOL,
            "{what}: recovered {got:?}, wanted {want:?}"
        );
    }
}

/// The acceptance gate: a 5-axis program's toolframe orientation is the same vector after a trip
/// through g-code, under every model.
///
/// `[0.36, 0.48, 0.8]` is a unit vector with three *distinct non-zero* components, so dropping or
/// swapping any one of them fails this — which an axis-aligned orientation would not catch. The
/// negative-`k` and `k = 0` cases carry the tilt past 90°, where `Ac`/`Bc` invert `acos` on the far
/// side of the quadrant.
#[test]
fn emitted_orientation_round_trips_through_import_for_every_model() {
    let orientations = [
        [0.36, 0.48, 0.8],
        [-0.36, 0.48, 0.8],
        [0.48, 0.36, -0.8],
        [0.6, 0.0, 0.8],
        [0.0, 0.6, 0.8],
        [
            std::f64::consts::FRAC_1_SQRT_2,
            std::f64::consts::FRAC_1_SQRT_2,
            0.0,
        ],
    ];
    let mut checked = 0;
    for kinematics in models() {
        for orientation in orientations {
            let gcode = emit_five_axis(&oriented_toolpath(orientation), kinematics);
            // non-vacuity: the program must actually carry both of the model's rotary words, or the
            // assertions below would be checking a round-trip of nothing.
            for letter in kinematics_letters(kinematics) {
                assert!(
                    gcode
                        .iter()
                        .any(|line| line.split(' ').any(|word| word.starts_with(letter))),
                    "{kinematics:?} emitted no {letter} word for {orientation:?}: {gcode:?}"
                );
            }
            let recovered = import_five_axis(&gcode, Some(kinematics));
            assert!(!recovered.is_empty(), "no segments imported: {gcode:?}");
            for (index, got) in recovered.iter().enumerate() {
                let got = got
                    .unwrap_or_else(|| panic!("segment {index} lost its orientation: {gcode:?}"));
                assert_close(got, orientation, &format!("{kinematics:?} segment {index}"));
                checked += 1;
            }
        }
    }
    assert!(
        checked >= 18,
        "expected ≥18 recovered orientations, got {checked}"
    );
}

fn kinematics_letters(kinematics: Kinematics) -> [&'static str; 2] {
    match kinematics {
        Kinematics::Ab { .. } => ["A", "B"],
        Kinematics::Ac { .. } => ["A", "C"],
        Kinematics::Bc { .. } => ["B", "C"],
    }
}

/// Rotary words are modal: `emit` writes one only when it changes, so recovering the *second*
/// orientation of a program proves the importer carries the unchanged word forward rather than
/// reading each line in isolation.
#[test]
fn a_changed_orientation_is_recovered_per_segment() {
    let first = [0.36, 0.48, 0.8];
    let second = [-0.48, 0.36, 0.8];
    for kinematics in models() {
        let tp = resolve(
            &design(&format!(
                r#"[{{"op":"geometry","width":0.6,"height":0.2}},{{"op":"extruder","on":true}},
                    {{"op":"orient","i":{},"j":{},"k":{}}},
                    {{"op":"move","x":0,"y":0,"z":0.2}},{{"op":"move","x":10,"y":0,"z":0.2}},
                    {{"op":"orient","i":{},"j":{},"k":{}}},
                    {{"op":"move","x":20,"y":10,"z":0.2}}]"#,
                first[0], first[1], first[2], second[0], second[1], second[2]
            )),
            &ResolveParams::default(),
        );
        let gcode = emit_five_axis(&tp, kinematics);
        let recovered = import_five_axis(&gcode, Some(kinematics));
        assert!(
            recovered.len() >= 3,
            "{kinematics:?}: expected ≥3 segments, got {}: {gcode:?}",
            recovered.len()
        );
        let last = recovered.last().unwrap().expect("last segment orientation");
        assert_close(last, second, &format!("{kinematics:?} last segment"));
        let earlier = recovered[recovered.len() - 2].expect("earlier segment orientation");
        assert_close(earlier, first, &format!("{kinematics:?} earlier segment"));
    }
}

/// A non-zero rotary joint offset is *subtracted* on the way in, mirroring the addition on the way
/// out. Adding it twice, or ignoring it, moves the recovered vector far outside the tolerance.
#[test]
fn rotary_joint_offsets_round_trip() {
    let orientation = [0.36, 0.48, 0.8];
    let models = [
        Kinematics::Ab {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [10.0, -5.0],
        },
        Kinematics::Ac {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [-7.5, 21.0],
        },
        Kinematics::Bc {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [3.25, -90.0],
        },
    ];
    for kinematics in models {
        let gcode = emit_five_axis(&oriented_toolpath(orientation), kinematics);
        let recovered = import_five_axis(&gcode, Some(kinematics));
        let got = recovered.last().unwrap().expect("orientation");
        assert_close(got, orientation, &format!("{kinematics:?}"));
    }
}

/// What the importer recovers is a unit vector by construction, so the rule that would fire on a
/// sloppy inverse does not fire on this one.
#[test]
fn recovered_orientation_does_not_trip_orientation_not_unit() {
    for kinematics in models() {
        let gcode = emit_five_axis(&oriented_toolpath([0.36, 0.48, 0.8]), kinematics);
        let imported = import_gcode(
            &gcode.join("\n"),
            &GcodeImportParams {
                kinematics: Some(kinematics),
                ..GcodeImportParams::default()
            },
        )
        .unwrap();
        // non-vacuity: there is an orientation on the IR for the rule to look at.
        assert!(imported
            .segments
            .iter()
            .all(|segment| segment.orientation.is_some()));
        let report = verify(&imported, &Contracts::default());
        assert!(
            !report
                .findings
                .iter()
                .any(|finding| finding.rule == "orientation-not-unit"),
            "{kinematics:?}: {:?}",
            report.findings
        );
    }
}

/// Rotary words with no model: refuse. Silently dropping them is what this slice removes, and
/// guessing the model from the letters cannot work — `A`+`C` is `Ac` on a trunnion table and nothing
/// in the program says it is not a head Dry does not model.
#[test]
fn rotary_words_without_a_kinematic_model_are_refused() {
    for kinematics in models() {
        let gcode = emit_five_axis(&oriented_toolpath([0.36, 0.48, 0.8]), kinematics);
        let error = import_gcode(&gcode.join("\n"), &GcodeImportParams::default())
            .expect_err("import must refuse rotary words it has no model for");
        assert!(
            error.message.contains("no kinematic model was supplied"),
            "unexpected error: {error}"
        );
        assert_eq!(error.source_line, 1, "error should name the line: {error}");
    }
}

/// A word the model has no axis for is not an orientation under that model — it is evidence the
/// model is wrong for this program.
#[test]
fn a_rotary_word_outside_the_model_is_refused() {
    let error = import_gcode(
        "G1 F1000 X0 Y0 Z0.2 A20 C30\n",
        &GcodeImportParams {
            kinematics: Some(models()[0]), // Ab: has no C axis
            ..GcodeImportParams::default()
        },
    )
    .expect_err("a C word under Ab must be refused");
    assert!(
        error
            .message
            .contains("not an axis of the AB kinematic model"),
        "unexpected error: {error}"
    );
}

/// One of the two axes never commanded: the pose is half known. The machine has the other axis
/// somewhere, but the program does not say where, and `orientation` has no way to record "unknown"
/// for one component of a direction.
#[test]
fn a_half_known_pose_is_refused() {
    let error = import_gcode(
        "G1 F1000 X0 Y0 Z0.2 C30\n",
        &GcodeImportParams {
            kinematics: Some(models()[2]), // Bc: C stated, B never
            ..GcodeImportParams::default()
        },
    )
    .expect_err("a C word with no B must be refused under Bc");
    assert!(
        error.message.contains("never B"),
        "unexpected error: {error}"
    );
}

/// The 3-axis path is untouched: no rotary words means no orientation and no refusal, whether or not
/// a model was supplied.
#[test]
fn a_program_without_rotary_words_imports_unchanged() {
    let source = "M83\nG1 X0 Y0 Z0.2 F9000\nG1 X10 E1.5 F1200\n";
    for kinematics in [None, Some(models()[0]), Some(models()[2])] {
        let imported = import_gcode(
            source,
            &GcodeImportParams {
                kinematics,
                ..GcodeImportParams::default()
            },
        )
        .unwrap();
        assert_eq!(imported.segments.len(), 2);
        assert!(
            imported
                .segments
                .iter()
                .all(|segment| segment.orientation.is_none()),
            "a 3-axis program must not acquire an orientation: {:?}",
            imported.segments
        );
    }
}

/// A modal rotary-only line is motion: it re-points the tool without moving a linear axis. If the
/// parser dropped it, the *next* segment would carry the stale pose — a wrong orientation, not a
/// missing one.
#[test]
fn a_modal_rotary_only_line_re_points_the_tool() {
    let imported = import_gcode(
        "G1 F1000 X0 Y0 Z0.2 C0 B36.869898\nX10\nC90\nX20\n",
        &GcodeImportParams {
            kinematics: Some(models()[2]),
            ..GcodeImportParams::default()
        },
    )
    .unwrap();
    assert_eq!(imported.segments.len(), 4, "{:?}", imported.segments);
    // B36.869898 with C0 → [0.6, 0, 0.8]; the bare `C90` swings it to [0, 0.6, 0.8].
    assert_close(
        imported.segments[1].orientation.expect("orientation"),
        [0.6, 0.0, 0.8],
        "before the rotary-only line",
    );
    assert_close(
        imported.segments[2].orientation.expect("orientation"),
        [0.0, 0.6, 0.8],
        "the rotary-only line itself",
    );
    assert_close(
        imported.segments[3].orientation.expect("orientation"),
        [0.0, 0.6, 0.8],
        "after the rotary-only line",
    );
}

/// The offsets are validated once at ingress, which is what makes the inverse total: a NaN offset
/// would otherwise produce a NaN orientation in the IR.
#[test]
fn a_non_finite_rotary_offset_is_refused_at_import() {
    let error = import_gcode(
        "G1 F1000 X0 Y0 Z0.2 A20 B30\n",
        &GcodeImportParams {
            kinematics: Some(Kinematics::Ab {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [f64::NAN, 0.0],
            }),
            ..GcodeImportParams::default()
        },
    )
    .expect_err("a non-finite rotary offset must be refused");
    assert!(
        error.message.contains("must be finite"),
        "unexpected error: {error}"
    );
}

/// The model reaches the importer the same way it reaches the emitter: from the profile.
#[test]
fn a_profile_supplies_the_import_kinematics() {
    let profile: dry_core::Profile = serde_json::from_str(
        r#"{"version":1,"machine":{"five_axis":{"type":"bc","rotary_offset":[0,0]}}}"#,
    )
    .unwrap();
    let params = profile.gcode_import_params();
    assert_eq!(
        params.kinematics,
        Some(Kinematics::Bc {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [0.0, 0.0],
        })
    );
    let gcode = emit_five_axis(
        &oriented_toolpath([0.36, 0.48, 0.8]),
        params.kinematics.unwrap(),
    );
    let imported = import_gcode(&gcode.join("\n"), &params).unwrap();
    assert_close(
        imported
            .segments
            .last()
            .unwrap()
            .orientation
            .expect("orientation"),
        [0.36, 0.48, 0.8],
        "profile-sourced kinematics",
    );
    // and a profile that declares no 5-axis machine leaves the importer 3-axis.
    let three_axis: dry_core::Profile = serde_json::from_str(r#"{"version":1}"#).unwrap();
    assert_eq!(three_axis.gcode_import_params().kinematics, None);
}

//! P5.4: the KRL target emits a KUKA *module*, not substituted g-code words (#181).
//!
//! **Read the test names literally.** Nothing here executes a KUKA program. Every assertion below
//! is about the *structure* of the emitted text, judged against the BNF written down in
//! `crates/core/src/emit/krl.rs` and `docs/22-krl-emit.md`. The only judgement in this repository
//! that Dry did not author is `tools/krl_check.sh`, which parses the golden with an external ANTLR
//! grammar; even that is a parse, not an execution. Dry's KRL output has never run on a KUKA
//! controller or on a simulator, and no test here may be named as though it had.
//!
//! The golden under `conformance/reports/robot/` is drift-gated like the other generated corpora
//! (`CONTRIBUTING.md` → "Conformance, vectors and goldens"); regenerate with
//! `UPDATE_GOLDEN=1 cargo test -p dry-core --test krl_program_structure`.

use dry_core::{
    emit_stream, EmitParams, FirmwareFlavor, Kinematics, KrlFrame, KrlTransform, Length, Segment,
    SegmentKind,
};

/// The motion instructions of a program, in order — everything the frame and banner is not.
fn motion_lines(program: &str) -> Vec<&str> {
    program
        .lines()
        .filter(|l| l.starts_with("  PTP ") || l.starts_with("  LIN ") || l.starts_with("  CIRC "))
        .collect()
}

fn seg(kind: SegmentKind) -> Segment {
    Segment {
        start: [None, None, None],
        end: [None, None, None],
        travel: false,
        speed: dry_core::Feedrate(1200.0),
        length: Length::ZERO,
        volume: dry_core::Volume::ZERO,
        filament: Length::ZERO,
        width: None,
        height: None,
        kind,
        centre: None,
        clockwise: false,
        temperature: None,
        fan: None,
        flow: None,
        tool: None,
        dwell_s: None,
        manual_gcode: None,
        orientation: None,
        control_points: None,
    }
}

fn mm(v: f64) -> Option<Length> {
    Some(Length::mm(v))
}

fn krl(five_axis: bool) -> EmitParams {
    EmitParams {
        flavor: FirmwareFlavor::RobotKrl,
        five_axis,
        ..EmitParams::default()
    }
}

fn emit_krl(segments: Vec<Segment>, p: &EmitParams) -> Result<String, String> {
    emit_stream(segments.into_iter().map(Ok), p)
        .map(|lines| lines.join("\n") + "\n")
        .map_err(|e| e.to_string())
}

/// The program the golden is cut from: a rapid in, a straight cut, a counter-clockwise quarter arc,
/// a dwell, and a reorientation — one instance of every construct the renderer can write.
fn reference_program_segments() -> Vec<Segment> {
    vec![
        Segment {
            end: [mm(10.0), mm(0.0), mm(5.0)],
            travel: true,
            speed: dry_core::Feedrate(3000.0),
            orientation: Some([0.0, 0.0, 1.0]),
            ..seg(SegmentKind::Line)
        },
        Segment {
            start: [mm(10.0), mm(0.0), mm(5.0)],
            end: [mm(20.0), mm(0.0), mm(5.0)],
            orientation: Some([0.0, 0.0, 1.0]),
            ..seg(SegmentKind::Line)
        },
        Segment {
            start: [mm(20.0), mm(0.0), mm(5.0)],
            end: [mm(30.0), mm(10.0), mm(5.0)],
            centre: Some([Length::mm(20.0), Length::mm(10.0)]),
            orientation: Some([0.6, 0.0, 0.8]),
            ..seg(SegmentKind::Arc)
        },
        Segment {
            dwell_s: Some(1.5),
            ..seg(SegmentKind::Dwell)
        },
        Segment {
            start: [mm(30.0), mm(10.0), mm(5.0)],
            end: [mm(30.0), mm(20.0), mm(5.0)],
            speed: dry_core::Feedrate(600.0),
            orientation: Some([0.0, -1.0, 0.0]),
            ..seg(SegmentKind::Line)
        },
    ]
}

/// The emitted text is a `DEF`/`END` module with the frame pinned before any motion.
///
/// Structural only: this asserts what the bytes are, not that a controller accepts them.
#[test]
fn emitted_text_is_wrapped_in_a_def_end_module_with_a_pinned_frame() {
    let program = emit_krl(reference_program_segments(), &krl(true)).unwrap();
    let lines: Vec<&str> = program.lines().collect();

    assert_eq!(lines[0], "DEF dry ( )");
    assert_eq!(*lines.last().unwrap(), "END");
    assert!(
        lines[1..6].iter().all(|l| l.starts_with(';')),
        "the banner must state that this has never run on a controller: {:?}",
        &lines[1..6]
    );
    assert!(program.contains("never run on a KUKA controller or simulator"));
    // The banner may say the structure is *checkable*, never that it was checked: the emitter runs
    // no grammar, and `tools/krl_check.sh` has only ever been run over the golden.
    assert!(
        program.contains("has not been checked") && program.contains("tools/krl_check.sh"),
        "{program}"
    );
    assert!(!program.contains("Structure checked against"), "{program}");

    let tool = lines.iter().position(|l| l.contains("$TOOL")).unwrap();
    let base = lines.iter().position(|l| l.contains("$BASE")).unwrap();
    let first_motion = lines
        .iter()
        .position(|l| {
            l.starts_with("  PTP ") || l.starts_with("  LIN ") || l.starts_with("  CIRC ")
        })
        .unwrap();
    assert!(
        tool < first_motion && base < first_motion,
        "$TOOL/$BASE must be pinned before the first move, not inherited from the pendant"
    );
    assert_eq!(
        lines[tool],
        "  $TOOL = {X 0.0, Y 0.0, Z 0.0, A 0.0, B 0.0, C 0.0}"
    );
}

/// The whole reference program, frozen. Also the file `tools/krl_check.sh` parses by default.
#[test]
fn golden_krl_module_does_not_drift() {
    let program = emit_krl(reference_program_segments(), &krl(true)).unwrap();
    let golden_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/reports/robot/reference-five-axis.src"
    );
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(std::path::Path::new(golden_path).parent().unwrap()).unwrap();
        std::fs::write(golden_path, &program).unwrap();
    }
    let golden = std::fs::read_to_string(golden_path).expect(
        "golden exists — regenerate with `UPDATE_GOLDEN=1 cargo test -p dry-core --test \
         krl_program_structure`",
    );
    assert_eq!(program, golden, "KRL output drifted from the frozen golden");
}

/// `$VEL.CP` is metres per second and governs CP motion only.
#[test]
fn vel_cp_is_metres_per_second_and_precedes_only_cp_moves() {
    let program = emit_krl(reference_program_segments(), &krl(true)).unwrap();
    // 1200 mm/min = 0.02 m/s; 600 mm/min = 0.01 m/s.
    assert!(program.contains("  $VEL.CP = 0.02\n"), "{program}");
    assert!(program.contains("  $VEL.CP = 0.01\n"), "{program}");
    // The 3000 mm/min rapid is a PTP, whose speed $VEL.CP does not govern, so 0.05 is never stated.
    assert!(!program.contains("0.05"), "{program}");

    let lines: Vec<&str> = program.lines().collect();
    let ptp = lines.iter().position(|l| l.starts_with("  PTP ")).unwrap();
    let first_vel = lines
        .iter()
        .position(|l| l.starts_with("  $VEL.CP"))
        .unwrap();
    assert!(
        ptp < first_vel,
        "a $VEL.CP ahead of the PTP would imply it governed a joint move"
    );
}

/// A KRL `CIRC` takes an auxiliary point, and that point is what carries the direction of travel:
/// the same arc reversed must emit a different auxiliary point.
///
/// This is the audit finding that `CIRC` lost arc direction — the old form emitted identical
/// `C`/`D` centre offsets for both directions.
#[test]
fn circ_auxiliary_point_distinguishes_clockwise_from_counter_clockwise() {
    let arc = |clockwise: bool| Segment {
        start: [mm(20.0), mm(0.0), mm(0.0)],
        end: [mm(30.0), mm(10.0), mm(0.0)],
        centre: Some([Length::mm(20.0), Length::mm(10.0)]),
        clockwise,
        ..seg(SegmentKind::Arc)
    };
    let ccw = emit_krl(vec![arc(false)], &krl(false)).unwrap();
    let cw = emit_krl(vec![arc(true)], &krl(false)).unwrap();

    // Radius 10 about (20, 10); start at -90 deg, end at 0 deg. The short way round sweeps +90 deg
    // through -45 deg; the long way sweeps -270 deg through +135 deg.
    assert_eq!(
        motion_lines(&ccw),
        ["  CIRC {E6POS: X 27.071068, Y 2.928932, Z 0.0}, {E6POS: X 30.0, Y 10.0, Z 0.0}"]
    );
    assert_eq!(
        motion_lines(&cw),
        ["  CIRC {E6POS: X 12.928932, Y 17.071068, Z 0.0}, {E6POS: X 30.0, Y 10.0, Z 0.0}"]
    );
    assert_ne!(ccw, cw, "the two directions must not emit the same program");
    // No `C`/`D` words survive: those were Dry-invented, not KRL.
    assert!(!ccw.contains(" C-") && !ccw.contains(" D"), "{ccw}");
}

/// Arcs a three-point `CIRC` cannot express are refused, not approximated (ADR 0002 §4).
#[test]
fn circ_refuses_the_arcs_three_points_cannot_describe() {
    let base = Segment {
        start: [mm(20.0), mm(0.0), mm(0.0)],
        end: [mm(30.0), mm(10.0), mm(0.0)],
        centre: Some([Length::mm(20.0), Length::mm(10.0)]),
        ..seg(SegmentKind::Arc)
    };

    let helix = Segment {
        end: [mm(30.0), mm(10.0), mm(4.0)],
        ..base.clone()
    };
    let err = emit_krl(vec![helix], &krl(false)).unwrap_err();
    assert!(err.contains("cannot climb"), "{err}");

    let full_turn = Segment {
        end: [mm(20.0), mm(0.0), mm(0.0)],
        ..base.clone()
    };
    let err = emit_krl(vec![full_turn], &krl(false)).unwrap_err();
    assert!(err.contains("full turn"), "{err}");

    let zero_radius = Segment {
        centre: Some([Length::mm(20.0), Length::mm(0.0)]),
        ..base.clone()
    };
    let err = emit_krl(vec![zero_radius], &krl(false)).unwrap_err();
    assert!(err.contains("non-zero radius"), "{err}");
}

/// The untilted tool is the canonical KUKA tool-pointing-down pose, and a tilt lands where the
/// ZYX-Euler decomposition says it should.
#[test]
fn orientation_becomes_zyx_euler_angles_with_the_untilted_tool_at_a0_b0_c180() {
    let line = |orientation: [f64; 3]| Segment {
        end: [mm(1.0), mm(0.0), mm(0.0)],
        orientation: Some(orientation),
        ..seg(SegmentKind::Line)
    };

    let upright = emit_krl(vec![line([0.0, 0.0, 1.0])], &krl(true)).unwrap();
    assert_eq!(
        motion_lines(&upright),
        ["  LIN {E6POS: X 1.0, Y 0.0, Z 0.0, A 0.0, B 0.0, C 180.0}"]
    );

    // Tool axis 30 deg from +Z, swung 90 deg about Z: A = 90, B = 30.
    let tilted = emit_krl(vec![line([0.0, 0.5, 3.0_f64.sqrt() / 2.0])], &krl(true)).unwrap();
    assert_eq!(
        motion_lines(&tilted),
        ["  LIN {E6POS: X 1.0, Y 0.0, Z 0.0, A 90.0, B 30.0, C 180.0}"]
    );

    // 3-axis: no orientation is stated at all, so the robot keeps the one it has.
    let flat = emit_krl(vec![line([0.0, 0.5, 3.0_f64.sqrt() / 2.0])], &krl(false)).unwrap();
    assert_eq!(motion_lines(&flat), ["  LIN {E6POS: X 1.0, Y 0.0, Z 0.0}"]);
}

/// Machine-tool offsets have no meaning for a TCP pose in `$BASE`, so a model carrying them is
/// refused rather than silently dropped.
#[test]
fn kinematic_offsets_that_a_robot_pose_cannot_carry_are_refused() {
    let params = EmitParams {
        kinematics: Kinematics::Bc {
            pivot_offset: [0.0, 0.0, 12.5],
            rotary_offset: [0.0, 0.0],
        },
        ..krl(true)
    };
    let err = emit_krl(
        vec![Segment {
            end: [mm(1.0), None, None],
            orientation: Some([0.0, 0.0, 1.0]),
            ..seg(SegmentKind::Line)
        }],
        &params,
    )
    .unwrap_err();
    assert!(err.contains("E6POS is a TCP pose in $BASE"), "{err}");
}

/// `$APO` is emitted only when there is an approximation distance to state, and then every CP
/// instruction references it. Setting one with nothing referencing it would be vacuous.
///
/// The three negative cases are the whole point: `approx_mm` alone is not enough, because a program
/// with no `LIN`/`CIRC` in it has nothing to blend. Asserting only the single-`LIN` case is what let
/// `$APO.CDIS` ship into PTP-only and dwell-only programs.
#[test]
fn apo_and_c_dis_appear_together_or_not_at_all() {
    let blended = |segments: Vec<Segment>| {
        emit_krl(
            segments,
            &EmitParams {
                krl_frame: KrlFrame {
                    approx_mm: Some(1.5),
                    ..KrlFrame::default()
                },
                ..krl(false)
            },
        )
        .unwrap()
    };
    let cp_move = vec![Segment {
        end: [mm(10.0), mm(0.0), mm(0.0)],
        ..seg(SegmentKind::Line)
    }];

    let exact = emit_krl(cp_move.clone(), &krl(false)).unwrap();
    assert!(
        !exact.contains("$APO") && !exact.contains("C_DIS"),
        "{exact}"
    );

    let with_cp = blended(cp_move);
    assert!(with_cp.contains("  $APO.CDIS = 1.5\n"), "{with_cp}");
    assert!(with_cp.contains("} C_DIS\n"), "{with_cp}");

    // PTP-only: `C_PTP`/`$APO.CPTP` is the pair that would blend a joint move, and dry emits
    // neither, so a `$APO.CDIS` here would be referenced by nothing.
    let ptp_only = blended(vec![
        Segment {
            end: [mm(10.0), mm(0.0), mm(0.0)],
            travel: true,
            ..seg(SegmentKind::Line)
        },
        Segment {
            start: [mm(10.0), mm(0.0), mm(0.0)],
            end: [mm(20.0), mm(0.0), mm(0.0)],
            travel: true,
            ..seg(SegmentKind::Line)
        },
    ]);
    assert!(
        !ptp_only.contains("$APO") && !ptp_only.contains("C_DIS"),
        "{ptp_only}"
    );

    // Dwell-only: no motion instruction at all.
    let dwell_only = blended(vec![Segment {
        dwell_s: Some(1.0),
        ..seg(SegmentKind::Dwell)
    }]);
    assert!(!dwell_only.contains("$APO"), "{dwell_only}");

    // And when both kinds are present the `$APO` line lands with the CP move, after the PTP.
    let mixed = blended(vec![
        Segment {
            end: [mm(10.0), mm(0.0), mm(0.0)],
            travel: true,
            ..seg(SegmentKind::Line)
        },
        Segment {
            start: [mm(10.0), mm(0.0), mm(0.0)],
            end: [mm(20.0), mm(0.0), mm(0.0)],
            ..seg(SegmentKind::Line)
        },
    ]);
    let lines: Vec<&str> = mixed.lines().collect();
    let ptp = lines.iter().position(|l| l.starts_with("  PTP ")).unwrap();
    let apo = lines.iter().position(|l| l.contains("$APO")).unwrap();
    let lin = lines.iter().position(|l| l.starts_with("  LIN ")).unwrap();
    assert!(ptp < apo && apo < lin, "{mixed}");
    assert_eq!(mixed.matches("$APO").count(), 1, "{mixed}");
}

/// A `manualgcode` segment is refused, not copied through.
///
/// The IR defines the field as verbatim *g-code* (`spec/dry-ir-v0.schema.json`,
/// `docs/10-dry-ir-v0-spec.md`), so its content is provably not a KRL statement. Copying it produced
/// a `DEF`/`END` module with `M117 hello` in the middle of it, which the external grammar rejects
/// (`line 7:5 no viable alternative at input 'M117hello'`) — the emitter must not write a file whose
/// only defence is a check nobody ran.
#[test]
fn a_manual_gcode_passthrough_is_refused_rather_than_copied_into_the_module() {
    let err = emit_krl(
        vec![
            Segment {
                end: [mm(10.0), mm(0.0), mm(0.0)],
                ..seg(SegmentKind::Line)
            },
            Segment {
                manual_gcode: Some("M117 hello".to_string()),
                ..seg(SegmentKind::ManualGcode)
            },
        ],
        &krl(false),
    )
    .unwrap_err();
    assert!(
        err.contains("manualgcode segment cannot be emitted as KRL"),
        "{err}"
    );
}

/// A segment that commands no pose change is refused, not restated as a duplicate instruction.
///
/// Restating the pose the robot is already at fabricated a zero-distance move the IR never asked
/// for — and, with blending on, one carrying `C_DIS`, i.e. an approximation request on zero
/// distance. Both segments of `conformance/vectors/retract_unretract` produced one.
#[test]
fn a_segment_that_moves_nothing_is_refused_rather_than_restated() {
    let at = |x: f64| Segment {
        start: [mm(x), mm(0.0), mm(0.2)],
        end: [mm(x), mm(0.0), mm(0.2)],
        filament: Length::mm(-2.0),
        ..seg(SegmentKind::Retract)
    };
    // The first segment still states its endpoint: nothing has been written yet, so every component
    // differs from what the controller is known to hold. The second has nothing left to say.
    let err = emit_krl(vec![at(0.0), at(0.0)], &krl(false)).unwrap_err();
    assert!(err.contains("commands no pose"), "{err}");
    assert!(err.contains("not KRL quantities"), "{err}");

    // Same refusal for a segment naming no axis at all — one rule, not two.
    let err = emit_krl(
        vec![
            Segment {
                end: [mm(10.0), mm(0.0), mm(0.0)],
                ..seg(SegmentKind::Line)
            },
            Segment {
                filament: Length::mm(2.0),
                ..seg(SegmentKind::Unretract)
            },
        ],
        &krl(false),
    )
    .unwrap_err();
    assert!(err.contains("commands no pose"), "{err}");
}

/// Five-axis linear words are chosen by exactly the g-code renderer's rule, so an axis that
/// *changed* is stated whether or not this segment named it.
///
/// The dropped case: `start: [50, 0, 0]`, `end: [null, 10, null]`. X inherits 50 from the segment's
/// own start, which the emitter has not written yet — `rs274` emits `X50`, and a KRL module that
/// omitted it walked the arm 40 mm to the wrong place.
#[test]
fn five_axis_states_an_axis_that_changed_even_when_the_segment_did_not_name_it() {
    let program = emit_krl(
        vec![Segment {
            start: [mm(50.0), mm(0.0), mm(0.0)],
            end: [None, mm(10.0), None],
            orientation: Some([0.0, 0.0, 1.0]),
            ..seg(SegmentKind::Line)
        }],
        &krl(true),
    )
    .unwrap();
    assert_eq!(
        motion_lines(&program),
        ["  LIN {E6POS: X 50.0, Y 10.0, Z 0.0, A 0.0, B 0.0, C 180.0}"]
    );
}

/// A frame that reached the emitter without passing through a profile is still validated.
#[test]
fn an_invalid_frame_refuses_the_program_instead_of_writing_it() {
    let segments = vec![Segment {
        end: [mm(10.0), None, None],
        ..seg(SegmentKind::Line)
    }];
    let with = |frame: KrlFrame| EmitParams {
        krl_frame: frame,
        ..krl(false)
    };

    let err = emit_krl(
        segments.clone(),
        &with(KrlFrame {
            program_name: Some("1st part".into()),
            ..KrlFrame::default()
        }),
    )
    .unwrap_err();
    assert!(err.contains("program_name"), "{err}");

    let err = emit_krl(
        segments.clone(),
        &with(KrlFrame {
            approx_mm: Some(0.0),
            ..KrlFrame::default()
        }),
    )
    .unwrap_err();
    assert!(err.contains("approx_mm"), "{err}");

    let err = emit_krl(
        segments,
        &with(KrlFrame {
            tool: KrlTransform {
                z: f64::NAN,
                ..KrlTransform::default()
            },
            ..KrlFrame::default()
        }),
    )
    .unwrap_err();
    assert!(err.contains("$TOOL.Z"), "{err}");
}

/// A named program keeps its name, because KUKA requires the `DEF` to match the file name.
#[test]
fn a_supplied_program_name_reaches_the_def_line() {
    let program = emit_krl(
        vec![Segment {
            end: [mm(1.0), None, None],
            ..seg(SegmentKind::Line)
        }],
        &EmitParams {
            krl_frame: KrlFrame {
                program_name: Some("bracket_v2".into()),
                ..KrlFrame::default()
            },
            ..krl(false)
        },
    )
    .unwrap();
    assert!(program.starts_with("DEF bracket_v2 ( )\n"), "{program}");
}

/// A dwell is `WAIT SEC`, not the bare `WAIT` the old renderer wrote — which is not KRL and which
/// the external grammar rejects.
#[test]
fn dwell_is_wait_sec() {
    let program = emit_krl(
        vec![
            Segment {
                end: [mm(1.0), None, None],
                ..seg(SegmentKind::Line)
            },
            Segment {
                dwell_s: Some(2.0),
                ..seg(SegmentKind::Dwell)
            },
        ],
        &krl(false),
    )
    .unwrap();
    assert!(program.contains("  WAIT SEC 2.0\n"), "{program}");
}

/// A path Dry cannot state a CP velocity for is refused, not run at an unstated speed.
#[test]
fn a_cp_move_with_a_non_positive_feedrate_is_refused() {
    let err = emit_krl(
        vec![Segment {
            end: [mm(10.0), None, None],
            speed: dry_core::Feedrate(0.0),
            ..seg(SegmentKind::Line)
        }],
        &krl(false),
    )
    .unwrap_err();
    assert!(err.contains("$VEL.CP"), "{err}");
}

/// A feedrate that is positive and finite but too small to print is refused too — because
/// `$VEL.CP = 0.0` is a controller fault whether it arrived as a zero or as a rounding.
#[test]
fn a_cp_move_whose_feedrate_rounds_to_zero_is_refused() {
    let program = emit_krl(
        vec![Segment {
            end: [mm(10.0), None, None],
            speed: dry_core::Feedrate(1e-8),
            ..seg(SegmentKind::Line)
        }],
        &krl(false),
    );
    let err = program.unwrap_err();
    assert!(err.contains("rounds to 0"), "{err}");
}

/// A `PTP` carries no speed word, but IR whose feedrate is not finite is still refused — the gate is
/// the same width here as on every g-code flavor.
#[test]
fn a_rapid_with_a_non_finite_feedrate_is_refused_even_though_ptp_states_no_speed() {
    let err = emit_krl(
        vec![Segment {
            end: [mm(1.0), None, None],
            travel: true,
            speed: dry_core::Feedrate(f64::NAN),
            ..seg(SegmentKind::Line)
        }],
        &krl(false),
    )
    .unwrap_err();
    assert!(err.contains("not finite"), "{err}");
}

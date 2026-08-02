//! Golden report generator + drift gate (`docs/11-profiles-and-reports.md`).
//!
//! Seeds are authored so that **every** [`dry_core::RuleId`] is triggered by at least one case — the
//! `rule_catalog_is_covered` assertion turns the goldens into a completeness check on the catalog. Run
//! with `UPDATE_REPORTS=1` to (re)write the goldens under `conformance/reports/`; the normal run asserts
//! the committed goldens still match the engine. The independent Python validator
//! (`tools/validate_reports.py`) re-checks every golden against `spec/dry-reports-v1.schema.json`.

use dry_core::{
    apply_gated, apply_safe_gated, build_explain_bundle, simulate, trace_summary, verify,
    CompareDelta, Contracts, ExplainBundle, ExplainReports, Feedrate, KinematicContracts, Length,
    LicenseStamp, OptimizeMode, Profile, Report, ReviewReport, RewriteReport, RewriteSpanResult,
    Segment, SegmentKind, Toolpath, TraceReport, Volume, REFERENCE_FIVE_AXIS_LIMITS,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn reports_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/reports")
}

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples/profiles")
}

fn update_mode() -> bool {
    std::env::var_os("UPDATE_REPORTS").is_some()
}

/// A valid extruding line; override per case.
fn base() -> Segment {
    Segment {
        start: [
            Some(Length::mm(0.0)),
            Some(Length::mm(0.0)),
            Some(Length::mm(0.2)),
        ],
        end: [
            Some(Length::mm(10.0)),
            Some(Length::mm(0.0)),
            Some(Length::mm(0.2)),
        ],
        travel: false,
        speed: Feedrate(1500.0),
        length: Length::mm(10.0),
        volume: Volume(0.8),
        filament: Length::mm(0.33),
        width: Some(Length::mm(0.4)),
        height: Some(Length::mm(0.2)),
        kind: SegmentKind::Line,
        centre: None,
        clockwise: false,
        temperature: Some(210.0),
        fan: None,
        flow: None,
        tool: None,
        power: None,
        dwell_s: None,
        manual_gcode: None,
        orientation: None,
        control_points: None,
    }
}

/// A non-printing move from `from` to `to`, used to stitch the rule probes below into a real path.
fn connecting_travel(from: [Option<Length>; 3], to: [Option<Length>; 3]) -> Segment {
    let d =
        |k: usize| to[k].unwrap_or(Length::ZERO).value() - from[k].unwrap_or(Length::ZERO).value();
    let (dx, dy, dz) = (d(0), d(1), d(2));
    Segment {
        start: from,
        end: to,
        travel: true,
        speed: Feedrate(6000.0),
        length: Length::mm((dx * dx + dy * dy + dz * dz).sqrt()),
        volume: Volume(0.0),
        filament: Length::ZERO,
        width: None,
        height: None,
        ..base()
    }
}

/// A toolpath taken exactly as written, with no stitching — for cases whose subject *is* a gap.
fn tp_raw(segments: Vec<Segment>) -> Toolpath {
    Toolpath {
        version: 0,
        meta: None,
        segments,
    }
}

/// Build a toolpath from a list of rule probes, inserting a travel wherever consecutive probes do not
/// meet.
///
/// Each case below is a bag of independent probes spread from `base()`, so every one of them ran
/// `(0,0,0.2) → (10,0,0.2)`: as written they described a path that teleported 10 mm backwards between
/// every pair. H1.3's `continuity` rule says so, correctly — the emitter writes endpoints only, so
/// each of those gaps is a move the machine makes and no rule inspected.
///
/// Stitching with travels rather than relocating the probes keeps every probe's own geometry, bead and
/// channels exactly as the fixture author wrote them, so each case still exercises the rules it was
/// built for. It also preserves the `kinematics` case's deliberately non-contiguous arc: a travel
/// resets junction tracking, which is precisely the property that fixture exists to pin.
fn tp(segments: Vec<Segment>) -> Toolpath {
    let mut out: Vec<Segment> = Vec::with_capacity(segments.len() * 2);
    let mut pos: Option<[Option<Length>; 3]> = None;
    for s in segments {
        if let Some(p) = pos {
            let meets = (0..3).all(|k| match (p[k], s.start[k]) {
                (Some(a), Some(b)) => (a.value() - b.value()).abs() <= 1e-9,
                _ => true,
            });
            if !meets {
                out.push(connecting_travel(p, s.start));
            }
        }
        pos = Some(s.end);
        out.push(s);
    }
    Toolpath {
        version: 0,
        meta: None,
        segments: out,
    }
}

struct Case {
    name: &'static str,
    toolpath: Toolpath,
    contracts: Contracts,
    /// Also emit trace + review goldens (skipped for cases that carry non-finite values).
    full: bool,
}

fn cases() -> Vec<Case> {
    // --- non_finite: a NaN quantity (verify-only: NaN would serialize as null in metrics). ---
    let non_finite = Case {
        name: "non_finite",
        toolpath: tp(vec![Segment {
            speed: Feedrate(f64::NAN),
            ..base()
        }]),
        contracts: Contracts::default(),
        full: false,
    };

    // --- structural: travel-extrudes, bead, orientation-not-unit, arc-radius (no contracts). ---
    let structural = Case {
        name: "structural",
        toolpath: tp(vec![
            // travel that deposits material
            Segment {
                travel: true,
                volume: Volume(0.5),
                ..base()
            },
            // extruding move with no bead width/height
            Segment {
                width: None,
                height: None,
                ..base()
            },
            // non-unit toolframe orientation
            Segment {
                orientation: Some([1.0, 1.0, 1.0]),
                ..base()
            },
            // arc whose endpoint radius disagrees with its start radius
            Segment {
                start: [
                    Some(Length::mm(10.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(0.0)),
                    Some(Length::mm(10.0)),
                    Some(Length::mm(0.2)),
                ],
                kind: SegmentKind::Arc,
                centre: Some([Length::mm(1.0), Length::mm(0.0)]),
                ..base()
            },
        ]),
        contracts: Contracts::default(),
        full: true,
    };

    // --- contracts: bounds, max-flow, speed, monotonic-z, cold-extrusion. ---
    let contracts_case = Case {
        name: "contracts",
        toolpath: tp(vec![
            // out of build volume (length matches the endpoints it actually spans)
            Segment {
                end: [
                    Some(Length::mm(500.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                length: Length::mm(500.0),
                ..base()
            },
            // flow over the ceiling (vol 8 over ~0.1s). `filament` is scaled with `volume` so the
            // feedstock cross-section stays base()'s 0.8/0.33; overriding one without the other made
            // the probe claim two different filament diameters in one program.
            Segment {
                speed: Feedrate(6000.0),
                volume: Volume(8.0),
                filament: Length::mm(3.3),
                ..base()
            },
            // feedrate out of range (flow kept under the ceiling)
            Segment {
                speed: Feedrate(12000.0),
                volume: Volume(0.4),
                filament: Length::mm(0.165),
                ..base()
            },
            // Z decreases. The 0.2 mm drop makes the move 10.002 mm long, not 10 — a ramp is longer
            // than its own XY run.
            Segment {
                start: [
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.4)),
                ],
                end: [
                    Some(Length::mm(10.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                length: Length::mm(100.04_f64.sqrt()),
                ..base()
            },
            // cold extrusion
            Segment {
                temperature: Some(150.0),
                ..base()
            },
        ]),
        contracts: Contracts {
            bounds: Some([[0.0, 200.0], [0.0, 200.0], [0.0, 200.0]]),
            max_flow: Some(10.0),
            speed_range: Some([300.0, 9000.0]),
            monotonic_z: true,
            min_temp: Some(200.0),
            ..Contracts::default()
        },
        full: true,
    };

    // --- retraction: travel-without-retraction, retraction-distance, retraction-speed. ---
    let retraction = Case {
        name: "retraction",
        toolpath: tp(vec![
            // extrude (clears the retracted flag)
            base(),
            // long travel without a retraction (50 mm spanned, so 50 mm declared)
            Segment {
                travel: true,
                volume: Volume(0.0),
                filament: Length::mm(0.0),
                length: Length::mm(50.0),
                end: [
                    Some(Length::mm(50.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                ..base()
            },
            // retraction that is too long. Extruder-only, so the machine does not move: start == end.
            // Inheriting base()'s 0 -> 10 endpoints while declaring length 0 made these probes emit a
            // 10 mm move at retraction feedrate, which is the same defect H1.3's probe found frozen
            // into conformance/vectors/retract_unretract.
            Segment {
                travel: false,
                volume: Volume(0.0),
                filament: Length::mm(-8.0),
                length: Length::mm(0.0),
                speed: Feedrate(1000.0),
                start: [
                    Some(Length::mm(50.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(50.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                ..base()
            },
            // retraction that is too fast
            Segment {
                travel: false,
                volume: Volume(0.0),
                filament: Length::mm(-2.0),
                length: Length::mm(0.0),
                speed: Feedrate(2500.0),
                start: [
                    Some(Length::mm(50.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(50.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                ..base()
            },
        ]),
        contracts: Contracts {
            max_retraction_distance: Some(5.0),
            max_retraction_speed: Some(2000.0),
            max_travel_without_retract: Some(30.0),
            ..Contracts::default()
        },
        full: true,
    };

    // --- first_layer: first-layer-height, first-layer-speed. ---
    let first_layer = Case {
        name: "first_layer",
        toolpath: tp(vec![
            // first-layer height out of range
            Segment {
                height: Some(Length::mm(0.5)),
                ..base()
            },
            // first-layer speed out of range
            Segment {
                speed: Feedrate(3000.0),
                ..base()
            },
        ]),
        contracts: Contracts {
            first_layer_height_range: Some([0.1, 0.3]),
            first_layer_speed_range: Some([500.0, 2000.0]),
            ..Contracts::default()
        },
        full: true,
    };

    // --- kinematics: peak-acceleration (arc), junction-velocity (contiguous Δv). ---
    // Arc radius = 1 mm at 6000 mm/min → centripetal_a = (100 mm/s)²/1 mm = 10 000 mm/s² >> 500.
    // Seg 0 at 1500 mm/min (25 mm/s) → seg 1 at 3000 mm/min (50 mm/s): Δv = 25 mm/s > limit 20.
    // The arc (seg 2) starts at (5,1) while seg 1 ends at (10,10): NOT contiguous → no jv false positive.
    let kinematics_case = Case {
        name: "kinematics",
        toolpath: tp(vec![
            // seg 0: extruding line at 1500 mm/min (25 mm/s)
            Segment {
                start: [
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(10.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                length: Length::mm(10.0),
                ..base()
            },
            // seg 1: contiguous extruding line at 3000 mm/min (50 mm/s) — Δv = 25 mm/s > 20 → jv fires
            Segment {
                start: [
                    Some(Length::mm(10.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(10.0)),
                    Some(Length::mm(10.0)),
                    Some(Length::mm(0.2)),
                ],
                speed: Feedrate(3000.0),
                length: Length::mm(10.0),
                ..base()
            },
            // arc with radius 1 mm at high speed — triggers peak-acceleration
            // centre (5,0), start (5,1), end (6,0): both radii = 1 mm ✓
            Segment {
                start: [
                    Some(Length::mm(5.0)),
                    Some(Length::mm(1.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(6.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                speed: Feedrate(6000.0),
                length: Length::mm(std::f64::consts::FRAC_PI_2), // ≈ quarter-arc arc-length
                kind: SegmentKind::Arc,
                centre: Some([Length::mm(5.0), Length::mm(0.0)]),
                clockwise: true,
                ..base()
            },
        ]),
        contracts: Contracts {
            kinematics: Some(KinematicContracts {
                max_acceleration_mm_s2: Some(500.0),
                max_junction_velocity_mm_s: Some(20.0),
            }),
            ..Contracts::default()
        },
        full: true,
    };

    // --- unmodeled: verbatim manual G-code is preserved but cannot be semantically verified. ---
    let unmodeled = Case {
        name: "unmodeled",
        toolpath: tp(vec![Segment {
            start: [None, None, None],
            end: [None, None, None],
            travel: true,
            speed: Feedrate(0.0),
            length: Length::ZERO,
            volume: Volume::ZERO,
            filament: Length::ZERO,
            width: None,
            height: None,
            temperature: None,
            kind: SegmentKind::ManualGcode,
            manual_gcode: Some("M84".into()),
            ..base()
        }]),
        contracts: Contracts::default(),
        full: true,
    };

    // --- wellformedness: continuity, negative-quantity, bead-volume (H1.3). ---
    //
    // Built with `tp_raw` rather than `tp`, because the whole point of the first probe is a gap that
    // `tp`'s stitching would helpfully repair away.
    let wellformedness = Case {
        name: "wellformedness",
        toolpath: tp_raw(vec![
            // a normal move, so there is a position to be discontinuous from
            base(),
            // starts 10 mm from where the previous move ended: the emitter writes endpoints only, so
            // the machine cuts straight across the gap and no other rule inspects that path
            Segment {
                start: [
                    Some(Length::mm(20.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(30.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                ..base()
            },
            // a negative length: outside the IR's own type contract, so no contract can excuse it
            Segment {
                start: [
                    Some(Length::mm(30.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(40.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                length: Length::mm(-10.0),
                ..base()
            },
            // deposits half the material its own bead geometry implies (10 x 0.4 x 0.2 = 0.8 mm³).
            // `filament` is scaled with it so this probe trips `bead-volume` alone.
            Segment {
                start: [
                    Some(Length::mm(40.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(50.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                volume: Volume(0.4),
                filament: Length::mm(0.165),
                ..base()
            },
            // consumes feedstock at a different cross-section from every earlier move on this tool:
            // one of the two segments misstates how much material it lays down, and the IR alone
            // cannot say which. Bead geometry is left consistent so this trips only that rule.
            Segment {
                start: [
                    Some(Length::mm(50.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(60.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                filament: Length::mm(0.5),
                ..base()
            },
        ]),
        contracts: Contracts {
            bead_volume_tolerance: Some(0.05),
            ..Contracts::default()
        },
        full: false,
    };

    // --- five_axis: rotary-travel, rotary-feed, orientation-reachability (#180 gaps 1-2). ---
    //
    // Judged against `REFERENCE_FIVE_AXIS_LIMITS` — B/C table, tilt limited to 120°, 3 600 °/min of
    // rotary rate, and a machine envelope whose Z floor is only 50 mm below the table.
    //
    // `tp_raw` rather than `tp`: the stitching travels `tp` inserts carry no orientation, which under a
    // 5-axis model is the identity (+Z) and therefore a rotary transition of its own — the probes below
    // are hand-stitched instead so each one trips exactly the rule it was written for. Each probe's
    // *other* two rotary quantities are deliberately kept inside their limits, which is what makes the
    // three rules separable here rather than one three-headed finding.
    let five_axis = Case {
        name: "five_axis",
        toolpath: tp_raw(vec![
            // 0: vertical tool, nothing to say. Establishes the rotary position the next probe moves
            //    from — without it, probe 1 would be the first oriented segment and have no Δ.
            Segment {
                orientation: Some([0.0, 0.0, 1.0]),
                ..base()
            },
            // 1: swings the tool to +X (B: 0° → 90°) inside a 10 mm move at 6000 mm/min — 0.1 s for
            //    90°, i.e. 54 000 °/min against a 3 600 °/min axis. Tilt itself is legal, and at
            //    x ≤ 20 the tilted point stays in the envelope, so only `rotary-feed` fires.
            Segment {
                start: [
                    Some(Length::mm(10.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(20.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                speed: Feedrate(6000.0),
                orientation: Some([1.0, 0.0, 0.0]),
                ..base()
            },
            // 2: asks for the tool to point straight down — B = acos(-1) = 180°, past the 120° the
            //    trunnion reaches. Slowed to 60 mm/min so the same 90° of sweep takes 10 s and does
            //    not also trip the rate limit: only `rotary-travel` fires.
            Segment {
                start: [
                    Some(Length::mm(20.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(30.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                speed: Feedrate(60.0),
                orientation: Some([0.0, 0.0, -1.0]),
                ..base()
            },
            // 3: a travel back to vertical and out to x = 100. 70 mm at 600 mm/min is 7 s for the
            //    180° unwind (1 543 °/min), and an untilted point is its own machine position, so this
            //    move is clean under all three rules.
            Segment {
                start: [
                    Some(Length::mm(30.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(100.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                travel: true,
                speed: Feedrate(600.0),
                length: Length::mm(70.0),
                volume: Volume(0.0),
                filament: Length::ZERO,
                width: None,
                height: None,
                orientation: Some([0.0, 0.0, 1.0]),
                ..base()
            },
            // 4: the same 90° tilt as probe 1, at the same legal rate as probe 2 — but 100 mm out in
            //    X, where tilting the table swings the point to Z = -100 in machine coordinates, below
            //    the -50 floor. Reachability is a property of the point and the orientation together,
            //    and this is the probe that shows it: nothing about the tilt alone is out of range.
            Segment {
                start: [
                    Some(Length::mm(100.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(110.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                speed: Feedrate(60.0),
                orientation: Some([1.0, 0.0, 0.0]),
                ..base()
            },
        ]),
        contracts: Contracts {
            rotary: Some(REFERENCE_FIVE_AXIS_LIMITS),
            ..Contracts::default()
        },
        full: true,
    };

    vec![
        non_finite,
        structural,
        contracts_case,
        retraction,
        first_layer,
        kinematics_case,
        unmodeled,
        wellformedness,
        five_axis,
    ]
}

fn write_or_check(path: PathBuf, bytes: &[u8], update: bool) {
    if update {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
    } else {
        let committed = fs::read(&path).unwrap_or_else(|_| {
            panic!(
                "missing {path:?} — run `UPDATE_REPORTS=1 cargo test -p dry-core --test report_goldens`"
            )
        });
        assert_eq!(committed, bytes, "{path:?} drifted from the engine output");
    }
}

#[test]
fn report_goldens_match_or_update() {
    let update = update_mode();
    let dir = reports_dir();
    let mut covered: BTreeSet<String> = BTreeSet::new();

    for case in cases() {
        let cdir = dir.join(case.name);
        let report = verify(&case.toolpath, &case.contracts);
        for f in &report.findings {
            covered.insert(f.rule.clone());
        }

        let verify_json = serde_json::to_string_pretty(&report).unwrap() + "\n";
        write_or_check(cdir.join("verify.json"), verify_json.as_bytes(), update);

        if case.full {
            let metrics = simulate(&case.toolpath);
            let review = ReviewReport::build(
                None,
                None,
                case.toolpath.segments.len(),
                metrics,
                &report,
                |_| None,
            );
            let review_json = serde_json::to_string_pretty(&review).unwrap() + "\n";
            write_or_check(cdir.join("review.json"), review_json.as_bytes(), update);

            let trace = trace_summary(&case.toolpath, 5.0).unwrap();
            let trace_report = TraceReport {
                file: None,
                profile: None,
                trace,
            };
            let trace_json = serde_json::to_string_pretty(&trace_report).unwrap() + "\n";
            write_or_check(cdir.join("trace.json"), trace_json.as_bytes(), update);
        }
    }

    // Forensics goldens: a Cura sample (no config block) and a PrusaSlicer sample (config block +
    // 45° infill → declared settings, inferred infill angle, recoverable extrusion multiplier).
    for (sample_file, case) in [
        ("examples/sliced-sample.gcode", "forensics"),
        ("examples/sliced-prusa-sample.gcode", "forensics-prusa"),
    ] {
        let sample = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(sample_file),
        )
        .unwrap_or_else(|_| panic!("{sample_file} exists"));
        let imported =
            dry_core::import_gcode_with_map(&sample, &dry_core::GcodeImportParams::default())
                .expect("import sample");
        let forensics = dry_core::forensics_analyze(&imported);
        let forensics_json = serde_json::to_string_pretty(&forensics).unwrap() + "\n";
        write_or_check(
            dir.join(case).join("forensics.json"),
            forensics_json.as_bytes(),
            update,
        );
    }

    // Completeness: every rule in the catalog must be exercised by at least one golden.
    let all: BTreeSet<String> = dry_core::RuleId::ALL
        .iter()
        .map(|r| r.as_str().to_string())
        .collect();
    assert_eq!(
        covered,
        all,
        "report goldens do not cover every RuleId (missing: {:?})",
        all.difference(&covered).collect::<Vec<_>>()
    );
}

/// A plain extruding line move between two 3-D points (off [`base`]).
fn line_seg(start: [f64; 3], end: [f64; 3]) -> Segment {
    let length =
        ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2) + (end[2] - start[2]).powi(2))
            .sqrt();
    Segment {
        start: [
            Some(Length::mm(start[0])),
            Some(Length::mm(start[1])),
            Some(Length::mm(start[2])),
        ],
        end: [
            Some(Length::mm(end[0])),
            Some(Length::mm(end[1])),
            Some(Length::mm(end[2])),
        ],
        length: Length::mm(length),
        volume: Volume(0.4),
        filament: Length::mm(0.16),
        ..base()
    }
}

/// Golden for `rewrite-gcode --json --mode safe`: a two-span case where span 0 (a collinear run) is
/// accepted and canonicalised, and span 1 (a circular run whose fitted arc bulges out of the build
/// volume) is rejected and passes through verbatim. Drift-gated like the other report goldens.
#[test]
fn rewrite_report_golden_matches_or_update() {
    let update = update_mode();
    let dir = reports_dir();

    // build volume admits both spans' chord endpoints but not the fitted arc's top point (y = 5).
    let contracts = Contracts {
        bounds: Some([[0.0, 200.0], [0.0, 4.5], [0.0, 200.0]]),
        ..Contracts::default()
    };

    // span 0: a collinear extruding run (merges to one move), in-bounds → accepted.
    let span0 = vec![
        line_seg([0.0, 1.0, 0.2], [10.0, 1.0, 0.2]),
        line_seg([10.0, 1.0, 0.2], [20.0, 1.0, 0.2]),
        line_seg([20.0, 1.0, 0.2], [30.0, 1.0, 0.2]),
    ];
    // span 1: four points on a circle of radius 5 centred at (10, 0), swept across the top. Every chord
    // endpoint has y ≤ 4.33, but the arc passes through (10, 5) → a NEW bounds error → rejected.
    let (cx, cy, r) = (10.0_f64, 0.0_f64, 5.0_f64);
    let pt = |deg: f64| {
        let a = deg.to_radians();
        [cx + r * a.cos(), cy + r * a.sin(), 0.2]
    };
    let span1 = vec![
        line_seg(pt(30.0), pt(60.0)),
        line_seg(pt(60.0), pt(120.0)),
        line_seg(pt(120.0), pt(150.0)),
    ];

    let mut before_segs: Vec<Segment> = Vec::new();
    let mut after_segs: Vec<Segment> = Vec::new();
    let mut span_results: Vec<RewriteSpanResult> = Vec::new();
    for (index, span) in [span0, span1].into_iter().enumerate() {
        let span_tp = tp(span);
        let before_count = span_tp.segments.len();
        let result = apply_safe_gated(&span_tp, &contracts);
        before_segs.extend(span_tp.segments.iter().cloned());
        after_segs.extend(result.toolpath.segments.iter().cloned());
        span_results.push(RewriteSpanResult {
            span_index: index,
            accepted: result.accepted,
            segment_count_before: before_count,
            segment_count_after: result.toolpath.segments.len(),
            new_error_rules: result.new_error_rules,
        });
    }

    let report = RewriteReport::build(
        Some("two-span.gcode".to_string()),
        Some("safe-demo".to_string()),
        "safe".to_string(),
        &tp(before_segs),
        &tp(after_segs),
        span_results,
    );
    let report_json = serde_json::to_string_pretty(&report).unwrap() + "\n";
    write_or_check(
        dir.join("rewrite_safe").join("report.json"),
        report_json.as_bytes(),
        update,
    );
}

/// Golden for `rewrite-gcode --json --mode balanced`: a two-span case under a `speed_range` contract
/// where span 0 (a collinear run) is canonicalised, shaped and accepted, while span 1 (a sharp 90°
/// corner) has its junction feedrate scaled by `adaptive_speed` below the range minimum, introducing a
/// new `speed` error → rejected and passed through verbatim. Drift-gated like the other report goldens.
#[test]
fn rewrite_balanced_report_golden_matches_or_update() {
    let update = update_mode();
    let dir = reports_dir();

    // a feedrate floor that admits the authored 1500 mm/min but not the corner's shaped speed (~1060).
    let contracts = Contracts {
        speed_range: Some([1200.0, 6000.0]),
        ..Contracts::default()
    };

    // span 0: a collinear extruding run (merges to one move; isolated → adaptive-speed no-op) → accepted.
    let span0 = vec![
        line_seg([0.0, 1.0, 0.2], [10.0, 1.0, 0.2]),
        line_seg([10.0, 1.0, 0.2], [20.0, 1.0, 0.2]),
        line_seg([20.0, 1.0, 0.2], [30.0, 1.0, 0.2]),
    ];
    // span 1: a 90° corner. `adaptive_speed` scales both legs to ~0.707×1500 < 1200 → new `speed` error.
    let span1 = vec![
        line_seg([0.0, 0.0, 0.2], [10.0, 0.0, 0.2]),
        line_seg([10.0, 0.0, 0.2], [10.0, 10.0, 0.2]),
    ];

    let mut before_segs: Vec<Segment> = Vec::new();
    let mut after_segs: Vec<Segment> = Vec::new();
    let mut span_results: Vec<RewriteSpanResult> = Vec::new();
    for (index, span) in [span0, span1].into_iter().enumerate() {
        let span_tp = tp(span);
        let before_count = span_tp.segments.len();
        let result = apply_gated(&span_tp, &contracts, OptimizeMode::Balanced, None);
        before_segs.extend(span_tp.segments.iter().cloned());
        after_segs.extend(result.toolpath.segments.iter().cloned());
        span_results.push(RewriteSpanResult {
            span_index: index,
            accepted: result.accepted,
            segment_count_before: before_count,
            segment_count_after: result.toolpath.segments.len(),
            new_error_rules: result.new_error_rules,
        });
    }

    let report = RewriteReport::build(
        Some("two-span.gcode".to_string()),
        Some("balanced-demo".to_string()),
        "balanced".to_string(),
        &tp(before_segs),
        &tp(after_segs),
        span_results,
    );
    let report_json = serde_json::to_string_pretty(&report).unwrap() + "\n";
    write_or_check(
        dir.join("rewrite_balanced").join("report.json"),
        report_json.as_bytes(),
        update,
    );
}

/// Golden for `dry explain --json`: the full explanation bundle (trace + forensics + verify + the
/// curated prompt) for the PrusaSlicer sample. Drift-gated like the other report goldens, and validated
/// against the `ExplainBundle` schema by `tools/validate_reports.py`.
#[test]
fn explain_bundle_golden_matches_or_update() {
    let update = update_mode();
    let dir = reports_dir();

    let sample = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("examples/sliced-prusa-sample.gcode"),
    )
    .expect("prusa sample exists");
    let imported =
        dry_core::import_gcode_with_map(&sample, &dry_core::GcodeImportParams::default())
            .expect("import sample");

    let metrics = simulate(&imported.toolpath);
    let report = verify(&imported.toolpath, &Contracts::default());
    let review = ReviewReport::build(
        Some("sliced-prusa-sample.gcode".to_string()),
        None,
        imported.toolpath.segments.len(),
        metrics,
        &report,
        |seg| imported.source_line_for_segment(seg),
    );

    let source_lines: Vec<Option<usize>> = imported
        .segment_source_lines
        .iter()
        .copied()
        .map(Some)
        .collect();
    let trace = dry_core::trace_summary_with_sources(&imported.toolpath, 5.0, &source_lines)
        .expect("trace sample");
    let trace_report = TraceReport {
        file: Some("sliced-prusa-sample.gcode".to_string()),
        profile: None,
        trace,
    };

    let forensics = dry_core::forensics_analyze(&imported);

    let bundle = build_explain_bundle(
        Some("sliced-prusa-sample.gcode".to_string()),
        None,
        false,
        ExplainReports {
            trace: trace_report,
            forensics,
            verify: review,
        },
    );

    // Invariants: the prompt carries the safety guardrail; the markdown render has all three sections.
    assert!(
        bundle.prompt.contains(dry_core::explain::GUARDRAIL),
        "explain prompt must carry the re-verify guardrail"
    );
    let md = dry_core::render_markdown(&bundle);
    for marker in [
        "## Headlines",
        "## Facts",
        "## Prompt",
        dry_core::explain::GUARDRAIL,
    ] {
        assert!(md.contains(marker), "markdown missing: {marker}");
    }

    let bundle_json = serde_json::to_string_pretty(&bundle).unwrap() + "\n";
    write_or_check(
        dir.join("explain").join("explain.json"),
        bundle_json.as_bytes(),
        update,
    );
}

/// The license stamp is passive and absent by default, so the committed goldens must be unaffected by
/// it: every stamped envelope has to parse from bytes that carry no `license` key, report no stamp, and
/// re-serialize to exactly those bytes again. That round-trip is the no-drift guarantee — if the serde
/// attributes on the new field are ever wrong (a missing `skip_serializing_if` or `default`), one of
/// these five assertions fails before any golden is rewritten.
#[test]
fn license_stamp_is_absent_from_unstamped_goldens() {
    let dir = reports_dir();
    let read =
        |rel: &str| fs::read_to_string(dir.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));

    // One committed golden per envelope that carries the optional stamp.
    let review_text = read("structural/review.json");
    let verify_text = read("structural/verify.json");
    let rewrite_text = read("rewrite_safe/report.json");
    let explain_text = read("explain/explain.json");
    let compare_text = read("compare/expected.json");

    for (name, text) in [
        ("structural/review.json", &review_text),
        ("structural/verify.json", &verify_text),
        ("rewrite_safe/report.json", &rewrite_text),
        ("explain/explain.json", &explain_text),
        ("compare/expected.json", &compare_text),
    ] {
        assert!(
            !text.contains("\"license\""),
            "{name} must carry no license stamp"
        );
    }

    // Typed round-trip: parse (proving the field is optional on the wire), assert no stamp, re-emit.
    macro_rules! round_trip {
        ($ty:ty, $name:literal, $text:expr) => {{
            let parsed: $ty = serde_json::from_str(&$text).unwrap();
            assert!(parsed.license.is_none(), "{} parsed a stamp", $name);
            assert_eq!(
                serde_json::to_string_pretty(&parsed).unwrap(),
                $text.trim_end(),
                "{} drifted when re-serialized with the license field present",
                $name
            );
        }};
    }
    round_trip!(ReviewReport, "structural/review.json", review_text);
    round_trip!(Report, "structural/verify.json", verify_text);
    round_trip!(RewriteReport, "rewrite_safe/report.json", rewrite_text);
    round_trip!(ExplainBundle, "explain/explain.json", explain_text);
    round_trip!(CompareDelta, "compare/expected.json", compare_text);
}

/// A stamped report still serializes the stamp — the field is skipped only when it is `None`.
#[test]
fn license_stamp_serializes_when_present() {
    let mut review: ReviewReport = serde_json::from_str(
        &fs::read_to_string(reports_dir().join("structural/review.json")).unwrap(),
    )
    .unwrap();
    review.license = Some(LicenseStamp {
        mode: "licensed".to_string(),
        licensee: Some("Test Co".to_string()),
        tier: Some("team".to_string()),
    });
    let json: serde_json::Value = serde_json::to_value(&review).unwrap();
    assert_eq!(json["license"]["mode"], "licensed");
    assert_eq!(json["license"]["licensee"], "Test Co");
    assert_eq!(json["license"]["tier"], "team");

    // Evaluation stamps carry no licensee/tier, and those two skip rather than emit null.
    review.license = Some(LicenseStamp {
        mode: "evaluation".to_string(),
        licensee: None,
        tier: None,
    });
    let json: serde_json::Value = serde_json::to_value(&review).unwrap();
    assert_eq!(json["license"], serde_json::json!({ "mode": "evaluation" }));
}

#[test]
fn example_profiles_are_valid() {
    let dir = examples_dir();
    let mut count = 0;
    for entry in fs::read_dir(&dir).expect("spec/examples/profiles exists") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        Profile::from_json(&text)
            .unwrap_or_else(|e| panic!("example profile {path:?} is invalid: {e}"));
        count += 1;
    }
    assert!(
        count >= 4,
        "expected at least 4 example profiles, found {count}"
    );
}

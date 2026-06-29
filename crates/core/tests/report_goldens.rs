//! Golden report generator + drift gate (`docs/11-profiles-and-reports.md`).
//!
//! Seeds are authored so that **every** [`dry_core::RuleId`] is triggered by at least one case — the
//! `rule_catalog_is_covered` assertion turns the goldens into a completeness check on the catalog. Run
//! with `UPDATE_REPORTS=1` to (re)write the goldens under `conformance/reports/`; the normal run asserts
//! the committed goldens still match the engine. The independent Python validator
//! (`tools/validate_reports.py`) re-checks every golden against `spec/dry-reports-v1.schema.json`.

use dry_core::{
    simulate, trace_summary, verify, Contracts, Feedrate, Length, Profile, ReviewReport, Segment,
    SegmentKind, Toolpath, TraceReport, Volume,
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
        dwell_s: None,
        manual_gcode: None,
        orientation: None,
        control_points: None,
    }
}

fn tp(segments: Vec<Segment>) -> Toolpath {
    Toolpath {
        version: 0,
        meta: None,
        segments,
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
            // out of build volume
            Segment {
                end: [
                    Some(Length::mm(500.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                ..base()
            },
            // flow over the ceiling (vol 8 over ~0.1s)
            Segment {
                speed: Feedrate(6000.0),
                volume: Volume(8.0),
                ..base()
            },
            // feedrate out of range (flow kept under the ceiling)
            Segment {
                speed: Feedrate(12000.0),
                volume: Volume(0.4),
                ..base()
            },
            // Z decreases
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
            // long travel without a retraction
            Segment {
                travel: true,
                volume: Volume(0.0),
                filament: Length::mm(0.0),
                length: Length::mm(40.0),
                end: [
                    Some(Length::mm(50.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                ..base()
            },
            // retraction that is too long
            Segment {
                travel: false,
                volume: Volume(0.0),
                filament: Length::mm(-8.0),
                length: Length::mm(0.0),
                speed: Feedrate(1000.0),
                ..base()
            },
            // retraction that is too fast
            Segment {
                travel: false,
                volume: Volume(0.0),
                filament: Length::mm(-2.0),
                length: Length::mm(0.0),
                speed: Feedrate(2500.0),
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

    vec![
        non_finite,
        structural,
        contracts_case,
        retraction,
        first_layer,
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

    // Forensics golden from the sliced sample (slicer detection + feature attribution).
    let sample = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/sliced-sample.gcode"),
    )
    .expect("examples/sliced-sample.gcode exists");
    let imported =
        dry_core::import_gcode_with_map(&sample, &dry_core::GcodeImportParams::default())
            .expect("import sliced sample");
    let forensics = dry_core::forensics_analyze(&imported);
    let forensics_json = serde_json::to_string_pretty(&forensics).unwrap() + "\n";
    write_or_check(
        dir.join("forensics").join("forensics.json"),
        forensics_json.as_bytes(),
        update,
    );

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

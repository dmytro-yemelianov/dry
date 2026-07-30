//! P5.3 acceptance: a pocket/profile emits a valid CNC program (#179).
//!
//! The golden under `conformance/reports/cnc/` is drift-gated like the other generated corpora
//! (`CONTRIBUTING.md` → "Conformance, vectors and goldens"); regenerate with
//! `UPDATE_GOLDEN=1 cargo test -p dry-core --test cnc_pocket_e2e`.
use dry_core::{
    emit, resolve_checked, verify, CncFrame, Contracts, CutMode, EmitParams, FirmwareFlavor,
    PocketOptions, PocketShape, ResolveParams,
};

fn opts() -> PocketOptions {
    PocketOptions {
        shape: PocketShape::Rect {
            x: 0.0,
            y: 0.0,
            width: 60.0,
            height: 40.0,
        },
        mode: CutMode::Pocket,
        tool_diameter: 6.0,
        stepover: None,
        depth: 5.0,
        depth_per_pass: Some(2.5),
        z_top: Some(0.0),
        safe_z: Some(5.0),
        cut_feed: Some(300.0),
        plunge_feed: Some(100.0),
    }
}

fn frame() -> CncFrame {
    CncFrame {
        wcs: Some(54),
        tool: Some(1),
        spindle_rpm: Some(10000.0),
        coolant: Some(false),
    }
}

/// Every word LinuxCNC's RS-274/NGC dialect documents for this program class.
const ALLOWED_WORDS: &[&str] = &[
    "G0", "G1", "G2", "G3", "G4", "G17", "G21", "G54", "G55", "G56", "G57", "G58", "G59", "G90",
    "M3", "M5", "M6", "M8", "M9", "M30",
];

fn word_is_allowed(tok: &str) -> bool {
    ALLOWED_WORDS.contains(&tok)
        || tok.starts_with('X')
        || tok.starts_with('Y')
        || tok.starts_with('Z')
        || tok.starts_with('I')
        || tok.starts_with('J')
        || tok.starts_with('F')
        || tok.starts_with('S')
        || tok.starts_with('T')
}

#[test]
fn pocket_emits_a_framed_parseable_rs274_program() {
    let design = dry_core::pocket_design(&opts());
    let tp = resolve_checked(&design, &ResolveParams::default()).unwrap();
    let report = verify(&tp, &Contracts::default());
    assert!(report.ok(), "clean verify: {report:?}");

    let params = EmitParams {
        flavor: FirmwareFlavor::Rs274,
        cnc_frame: Some(frame()),
        ..EmitParams::default()
    };
    let lines = emit(&tp, &params);
    assert_eq!(lines[0], "G21 G17 G90");
    assert_eq!(lines.last().unwrap(), "M30");
    assert_eq!(lines.iter().filter(|l| *l == "M30").count(), 1);
    for line in &lines {
        for tok in line.split_whitespace() {
            assert!(
                word_is_allowed(tok),
                "word outside the RS-274 vocabulary: {tok} in {line}"
            );
        }
    }
    assert!(
        !lines.iter().any(|l| l.contains(" E")),
        "no extruder words on rs274"
    );
    assert!(lines.iter().any(|l| l.starts_with("G0")), "rapids present");
}

#[test]
fn circle_pocket_emits_arc_words() {
    let design = dry_core::pocket_design(&PocketOptions {
        shape: PocketShape::Circle {
            cx: 30.0,
            cy: 20.0,
            radius: 15.0,
        },
        ..opts()
    });
    let tp = resolve_checked(&design, &ResolveParams::default()).unwrap();
    let lines = emit(
        &tp,
        &EmitParams {
            flavor: FirmwareFlavor::Rs274,
            ..EmitParams::default()
        },
    );
    assert!(
        lines
            .iter()
            .any(|l| (l.starts_with("G2 ") || l.starts_with("G3 "))
                && l.contains("I")
                && l.contains("J")),
        "arc words with I/J offsets expected"
    );
}

#[test]
fn golden_rect_pocket_program_does_not_drift() {
    let design = dry_core::pocket_design(&opts());
    let tp = resolve_checked(&design, &ResolveParams::default()).unwrap();
    let params = EmitParams {
        flavor: FirmwareFlavor::Rs274,
        cnc_frame: Some(frame()),
        ..EmitParams::default()
    };
    let program = emit(&tp, &params).join("\n") + "\n";
    let golden_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/reports/cnc/pocket-rect-rs274.ngc"
    );
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(std::path::Path::new(golden_path).parent().unwrap()).unwrap();
        std::fs::write(golden_path, &program).unwrap();
    }
    let golden = std::fs::read_to_string(golden_path).expect(
        "golden exists — regenerate with `UPDATE_GOLDEN=1 cargo test -p dry-core --test cnc_pocket_e2e`",
    );
    assert_eq!(
        program, golden,
        "rs274 pocket output drifted from the frozen golden"
    );
}

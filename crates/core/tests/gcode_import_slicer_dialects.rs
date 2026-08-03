//! Regression: real slicer output imports, and the lines Dry does not model are *reported* rather
//! than fatal.
//!
//! Measured before this gate existed: 4/4 stock OrcaSlicer 2.4.0 files — Bambu Lab X1 Carbon and
//! Prusa MK4, both from Orca's own bundled profiles, no hand-editing — failed `dry review-gcode`
//! and `dry trace-gcode` outright. All four failures were upstream of the documented
//! "preserved byte-for-byte and reported as `unmodeled-gcode`" contract (`docs/14`), in classifying
//! the line rather than in reviewing it:
//!
//! 1. a vendor macro's parameters are not G-code words (`M1002 set_gcode_claim_speed_level : 5`),
//! 2. a firmware capability check takes a quoted string (`M862.3 P "MK4"`),
//! 3. a macro argument reuses an RS-274 word letter Dry lifts as a KRL command marker
//!    (`M1006 A0 B10 L100 …` scanned as a `LIN` move with a rotary pose and 37 mm of extrusion).
//!
//! The fixtures below are hand-written from those files' start-g-code blocks — small enough to read,
//! and carrying every construct that broke. The 13k-line originals are deliberately not committed.

use dry_core::{
    import_gcode_with_map, simulate, verify, Contracts, GcodeImportParams, ReviewReport,
};

/// A Bambu Lab X1C-style header: bareword macros, a flag-word `M221`, a SKIPPABLE calibration
/// block, a base64 AMS payload, and the note-playing `M1006` whose `L`/`A`/`B`/`C`/`E` arguments
/// are not motion. Ends in real moves.
const BAMBU_HEADER: &str = "\
; HEADER_BLOCK_START
M73.2   R1.0 ;Reset left time magnitude
M1002 set_gcode_claim_speed_level : 5
M221 X0 Y0 Z0 ; turn off soft endstop to prevent protential logic problem
G29.1 Z0 ; clear z-trim value first
M1002 gcode_claim_action : 2
M221 S; push soft endstop status
M620 M
M1006 A0 B10 L100 C37 D10 M60 E37 F10 N60
; SKIPPABLE_START
M622.1 S1
M1002 judge_flag extrude_cali_flag
M624 AQAAAAAAAAA=
M623
; SKIPPABLE_END
G21
G90
M83
G1 Z0.2 F600
G1 X0 Y0 F6000
G1 X10 Y0 E0.5 F1800
G1 X10 Y10 E0.5
";

/// A Prusa MK4-style header: the `M862.x` firmware checks (quoted with and without a leading space),
/// a firmware version string, and `M555` whose `W`/`H` arguments are a print area, not a `WAIT`.
const PRUSA_HEADER: &str = "\
;TYPE:Custom
M17 ; enable steppers
M862.1 P0.4 ; nozzle diameter check
M862.3 P \"MK4\" ; printer model check
M862.5 P2 ; g-code level check
M862.6 P\"Input shaper\" ; FW feature check
M115 U5.0.0-RC+11963
M555 X112.593 Y88.5929 W32 H28.8142
M84 E ; turn off E motor
G21
G90 ; use absolute coordinates
M83 ; extruder relative mode
G1 X0 Y0 Z0.2 F6000
G1 X20 Y0 E0.8 F1800
G1 X20 Y20 E0.8
";

fn params() -> GcodeImportParams {
    GcodeImportParams {
        line_width: Some(0.42),
        layer_height: Some(0.2),
        ..GcodeImportParams::default()
    }
}

/// Every unmodeled line is reported, at its own source line, under its own leading command — and no
/// line is reported twice or missed.
fn assert_unmodeled(source: &str, expected: &[(usize, &str)]) {
    let imported = import_gcode_with_map(source, &params())
        .unwrap_or_else(|e| panic!("stock slicer output must import: {e}"));
    let got: Vec<(usize, &str)> = imported
        .unmodeled_commands
        .iter()
        .map(|command| (command.source_line, command.command.as_str()))
        .collect();
    assert_eq!(got, expected);

    // The warning has to quote the line, and the line has to be the source byte-for-byte.
    for command in &imported.unmodeled_commands {
        assert_eq!(
            command.raw,
            source.lines().nth(command.source_line - 1).unwrap(),
            "line {} is not preserved verbatim",
            command.source_line
        );
    }
    assert_eq!(imported.source_text().trim_end(), source.trim_end());
}

#[test]
fn a_bambu_x1c_header_imports_with_its_macros_reported_not_executed() {
    assert_unmodeled(
        BAMBU_HEADER,
        &[
            (2, "M73.2"),
            (3, "M1002"),
            (4, "M221"),
            (5, "G29.1"),
            (6, "M1002"),
            // `M221 S` pushes the soft-endstop status; a flag `S` is not the `M221 S100` multiplier.
            (7, "M221"),
            (8, "M620"),
            // The note macro: `L100` is not a KRL `LIN`, and `E37` is not 37 mm of filament.
            (9, "M1006"),
            (11, "M622.1"),
            (12, "M1002"),
            (13, "M624"),
            (14, "M623"),
        ],
    );

    let imported = import_gcode_with_map(BAMBU_HEADER, &params()).unwrap();
    // Only the four `G1` lines are motion, and each maps back to its own source line.
    assert_eq!(
        imported.segment_source_lines,
        vec![19, 20, 21, 22],
        "motion must be recovered from exactly the G1 lines"
    );
    let segments = &imported.toolpath.segments;
    assert_eq!(segments.len(), 4);
    // `M83` was honoured, so the two extruding moves deposit 0.5 mm of filament each, not a
    // cumulative absolute total — and `M1006 ... E37 ...` deposited nothing.
    assert!(segments[0].travel && segments[1].travel);
    for segment in &segments[2..] {
        assert!(!segment.travel);
        assert_eq!(segment.length.value(), 10.0);
        assert_eq!(segment.filament.value(), 0.5);
    }
}

#[test]
fn a_prusa_mk4_header_imports_with_its_quoted_firmware_checks_reported() {
    assert_unmodeled(
        PRUSA_HEADER,
        &[
            (2, "M17"),
            (3, "M862.1"),
            (4, "M862.3"),
            (5, "M862.5"),
            (6, "M862.6"),
            (7, "M115"),
            // `W32 H28.8142` is a print area, not a KRL `WAIT`.
            (8, "M555"),
            (9, "M84"),
        ],
    );

    let imported = import_gcode_with_map(PRUSA_HEADER, &params()).unwrap();
    assert_eq!(imported.segment_source_lines, vec![13, 14, 15]);
    let segments = &imported.toolpath.segments;
    assert_eq!(segments.len(), 3);
    assert!(segments[0].travel);
    for segment in &segments[1..] {
        assert!(!segment.travel);
        assert_eq!(segment.length.value(), 20.0);
        assert_eq!(segment.filament.value(), 0.8);
    }
}

/// The whole point of importing these files is being able to review them: `review-gcode` runs the
/// verifier over the lifted motion and folds the preserved lines in as warnings, and neither half
/// may be empty.
#[test]
fn review_gcode_runs_over_both_headers_and_reports_the_preserved_lines() {
    for (name, source, unmodeled) in [("bambu", BAMBU_HEADER, 12), ("prusa", PRUSA_HEADER, 8)] {
        let imported = import_gcode_with_map(source, &params()).unwrap();
        let metrics = simulate(&imported.toolpath);
        let report = verify(&imported.toolpath, &Contracts::default());
        let mut review = ReviewReport::build(
            Some(format!("{name}.gcode")),
            None,
            imported.toolpath.segments.len(),
            metrics,
            &report,
            |segment| imported.source_line_for_segment(segment),
        );
        review.add_unmodeled_gcode(&imported);

        // The verifier inspected the motion rather than refusing the program...
        assert_eq!(
            report.segments_inspected,
            imported.toolpath.segments.len(),
            "[{name}] verify must inspect every lifted segment"
        );
        assert_eq!(report.error_count(), 0, "[{name}] {:?}", report.findings);
        // ...and every preserved line came through as a warning naming its own source line.
        let warnings: Vec<&dry_core::LocatedFinding> = review
            .findings
            .iter()
            .filter(|finding| finding.rule == "unmodeled-gcode")
            .collect();
        assert_eq!(warnings.len(), unmodeled, "[{name}]");
        assert!(
            warnings.iter().all(|finding| finding.source_line.is_some()),
            "[{name}] an unmodeled warning with no source line cannot be reviewed"
        );
        assert!(
            serde_json::to_string(&review).is_ok(),
            "[{name}] review report must serialise"
        );
    }
}

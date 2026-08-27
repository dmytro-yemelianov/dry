//! DRY2 rows are self-describing, and the decoder never panics on a payload it did not write.
//!
//! The encoder has always written a one-byte placeholder for a segment whose end coordinates are
//! undefined, which is every segment of ordinary 2D work — a laser or router that emits no Z word.
//! That row was not in the documented layout and the decoder did not know about it: it read a flag
//! byte then unconditionally consumed sixteen more, so the first short row desynchronised the
//! cursor and the read ran off the end of the buffer.

use dry_core::{decode_dry2, encode_dry2, import_gcode, GcodeImportParams};

/// 2D g-code with no Z word: every segment has `end[2] == None`, so every row is short.
#[test]
fn two_dimensional_gcode_round_trips() {
    let src = "G21\nG90\nG1 X10 Y10 F1000\nG1 X50 Y10\nG1 X50 Y40\n";
    let toolpath = import_gcode(src, &GcodeImportParams::default()).expect("import");
    assert!(
        toolpath.segments.iter().all(|s| s.end[2].is_none()),
        "the fixture must actually exercise undefined coordinates"
    );

    let encoded = encode_dry2(&toolpath);
    let decoded = decode_dry2(&encoded).expect("a payload the encoder wrote must decode");

    assert_eq!(
        decoded.segments.len(),
        toolpath.segments.len(),
        "every short row must survive as a segment rather than desynchronising the cursor"
    );
    assert!(
        decoded.segments.iter().all(|s| s.end[2].is_none()),
        "an undefined coordinate must stay undefined, not become an invented zero"
    );
}

/// A short row must not swallow the row after it: coordinates on later segments still decode.
#[test]
fn a_short_row_does_not_consume_the_next_row() {
    let src = "G21\nG90\nG1 X10 Y10 F1000\nG1 X50 Y10 Z5\nG1 X50 Y40 Z5\n";
    let toolpath = import_gcode(src, &GcodeImportParams::default()).expect("import");
    let short = toolpath
        .segments
        .iter()
        .filter(|s| s.end[2].is_none())
        .count();
    let full = toolpath.segments.len() - short;
    assert!(
        short > 0 && full > 0,
        "fixture must mix short and full rows"
    );

    let decoded = decode_dry2(&encode_dry2(&toolpath)).expect("mixed rows must decode");
    assert_eq!(decoded.segments.len(), toolpath.segments.len());

    // The full rows must still carry their coordinates, which is what proves alignment held.
    let decoded_full = decoded
        .segments
        .iter()
        .filter(|s| s.end[2].is_some())
        .count();
    assert_eq!(
        decoded_full, full,
        "a short row misaligned the rows that follow it"
    );
}

/// The decoder reads bytes from outside the process — the wasm binding hands it straight to
/// JavaScript — so a corrupt payload is an error, never a panic. In wasm a panic is an abort the
/// caller cannot catch.
#[test]
fn truncated_payloads_are_errors_not_panics() {
    let src = "G21\nG90\nG1 X10 Y10 Z1 F1000\nG1 X50 Y10 Z1\n";
    let toolpath = import_gcode(src, &GcodeImportParams::default()).expect("import");
    let encoded = encode_dry2(&toolpath);
    assert!(
        encoded.len() > 12,
        "fixture must have at least one row body"
    );

    // Every truncation point past the header, including mid-row.
    for cut in 12..encoded.len() {
        let result = decode_dry2(&encoded[..cut]);
        // Either it errors, or it decodes the whole rows it did receive — never a panic.
        if let Ok(toolpath) = result {
            assert!(
                toolpath.segments.len() <= encoded.len(),
                "a truncated payload must not fabricate segments"
            );
        }
    }
}

/// The ordinary 3D path is unchanged.
#[test]
fn three_dimensional_gcode_still_round_trips() {
    let src = "G21\nG90\nG1 X10 Y10 Z0.2 F1000\nG1 X50 Y10 Z0.2\nG1 X50 Y40 Z0.4\n";
    let toolpath = import_gcode(src, &GcodeImportParams::default()).expect("import");
    assert!(toolpath.segments.iter().all(|s| s.end[2].is_some()));

    let decoded = decode_dry2(&encode_dry2(&toolpath)).expect("decode");
    assert_eq!(decoded.segments.len(), toolpath.segments.len());
    for (before, after) in toolpath.segments.iter().zip(&decoded.segments) {
        let (b, a) = (
            before.end[2].unwrap().value(),
            after.end[2].unwrap().value(),
        );
        assert!(
            (b - a).abs() < 1e-3,
            "Z drifted through the round trip: {b} -> {a}"
        );
    }
}

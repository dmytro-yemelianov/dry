//! Conformance: Dry G-code round-trip parser gate.
//! Parses Marlin, Klipper, and Duet flavor G-code to Toolpath IR, and re-emits to match original G-code byte-for-byte.

use dry_core::{emit, EmitParams, GcodeImportParams, import_gcode};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize)]
struct InitParams {
    relative_e: bool,
    travel_format: String,
}

#[derive(Deserialize)]
struct Fixture {
    design: String,
    variant: String,
    init_params: InitParams,
    gcode: String,
}

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/roundtrip")
}

#[test]
fn roundtrip_conformance_byte_for_byte() {
    let dir = corpus_dir();
    let mut checked = 0;
    for entry in fs::read_dir(&dir).expect("conformance/roundtrip exists") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let fx: Fixture = serde_json::from_str(&fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|e| panic!("parse {path:?}: {e}"));

        let import_params = GcodeImportParams {
            version: 0,
            filament_diameter: 1.75,
            line_width: None,
            layer_height: None,
            relative_e: fx.init_params.relative_e,
        };

        let tp = import_gcode(&fx.gcode, &import_params)
            .unwrap_or_else(|e| panic!("import G-code for {}/{}: {}", fx.design, fx.variant, e));

        let emit_params = EmitParams {
            relative_e: fx.init_params.relative_e,
            travel_g1_e0: fx.init_params.travel_format == "G1_E0",
            five_axis: false,
            ..EmitParams::default()
        };

        let got_lines = emit(&tp, &emit_params);
        let got_gcode = got_lines.join("\n");

        assert_eq!(
            got_gcode.trim(),
            fx.gcode.trim(),
            "G-code round-trip mismatch for {}/{}",
            fx.design,
            fx.variant
        );
        checked += 1;
    }
    assert!(checked >= 12, "expected at least 12 roundtrip fixtures, found {}", checked);
    println!("G-code round-trip conformance: {checked} fixtures passed");
}

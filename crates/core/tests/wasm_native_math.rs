// These exercise the deprecated infallible `emit()` on purpose: it is still the entry point the
// in-tree call sites use, and refusing the whole program is part of what is under test here.
#![allow(deprecated)]

use dry_core::{emit, resolve, Design, EmitParams, ResolveParams, Toolpath};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn corpus_dir() -> PathBuf {
    workspace_root().join("conformance/gcode")
}

fn decode_hex(s: &str) -> Vec<u8> {
    let s = s.trim();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// Resolve and emit one L1 design both natively and under Node, and require the two to agree
/// bit-for-bit. Split out of the corpus loop so a design that cannot live in the oracle corpus can
/// still be measured against wasm by the same code — see `a_clothoid_is_bit_identical_on_wasm`.
fn assert_wasm_matches_native(
    root: &Path,
    design: &str,
    l1_value: &serde_json::Value,
    resolve_params_value: &serde_json::Value,
    params: &EmitParams,
) {
    let l1: Design = serde_json::from_value(l1_value.clone()).unwrap();
    let resolve_params: ResolveParams =
        serde_json::from_value(resolve_params_value.clone()).unwrap();

    // Native resolve
    let native_tp = resolve(&l1, &resolve_params);

    // Native emit
    let native_gcode = emit(&native_tp, params);

    // Run Node to get the wasm resolve and emit
    let ops_json = serde_json::to_string(&l1_value["ops"]).unwrap();
    let resolve_params_json = serde_json::to_string(resolve_params_value).unwrap();

    // Wasm resolve (as raw binary via resolve_binary)
    let js_resolve_code = format!(
        "const dry = require('./web/pkg-node/dry_wasm.js'); console.log(Buffer.from(dry.resolve_binary(JSON.stringify({ops_json}), JSON.stringify({resolve_params_json}))).toString('hex'));"
    );
    let node_resolve_output = Command::new("node")
        .arg("-e")
        .arg(&js_resolve_code)
        .current_dir(root)
        .output()
        .expect("failed to execute node for resolve_binary");
    assert!(
        node_resolve_output.status.success(),
        "node execution for resolve_binary failed: {}",
        String::from_utf8_lossy(&node_resolve_output.stderr)
    );

    let wasm_tp_hex = String::from_utf8(node_resolve_output.stdout).unwrap();
    let wasm_tp_bytes = decode_hex(&wasm_tp_hex);
    let wasm_tp = Toolpath::from_bytes(&wasm_tp_bytes)
        .unwrap_or_else(|e| panic!("failed to decode wasm Toolpath from binary bytes: {e}"));

    // Wasm emit
    let kinematics_str = match params.kinematics {
        dry_core::Kinematics::Ac { .. } => "ac",
        dry_core::Kinematics::Bc { .. } => "bc",
        dry_core::Kinematics::Ab { .. } => "ab",
    };
    let js_emit_code = format!(
        "const dry = require('./web/pkg-node/dry_wasm.js'); console.log(JSON.stringify(dry.resolve_gcode(JSON.stringify({ops_json}), JSON.stringify({resolve_params_json}), {}, {}, {}, '{kinematics_str}')));",
        params.relative_e, params.travel_g1_e0, params.five_axis
    );
    let node_emit_output = Command::new("node")
        .arg("-e")
        .arg(&js_emit_code)
        .current_dir(root)
        .output()
        .expect("failed to execute node for resolve_gcode");
    assert!(
        node_emit_output.status.success(),
        "node execution for resolve_gcode failed: {}",
        String::from_utf8_lossy(&node_emit_output.stderr)
    );
    let wasm_gcode_json = String::from_utf8(node_emit_output.stdout).unwrap();
    let wasm_gcode: Vec<String> = serde_json::from_str(&wasm_gcode_json)
        .unwrap_or_else(|e| panic!("failed to parse wasm gcode JSON: {e}"));

    // Compare native and wasm Toolpath (exact bit-identity comparison)
    assert_eq!(
        native_tp, wasm_tp,
        "[{design}] native and wasm resolved Toolpaths differ (not bit-identical)"
    );

    // Compare native and wasm G-code
    assert_eq!(
        native_gcode, wasm_gcode,
        "[{design}] native and wasm emitted G-code differ (not bit-identical)"
    );
}

fn build_wasm_for_node(root: &Path) {
    let build_status = Command::new("bash")
        .arg("web/build.sh")
        .arg("nodejs")
        .arg("web/pkg-node")
        .current_dir(root)
        .status()
        .expect("failed to run bash web/build.sh");
    assert!(build_status.success(), "wasm build failed");
}

#[test]
fn wasm_and_native_math_are_bit_identical() {
    let root = workspace_root();

    // 1. Build the wasm package for Node
    build_wasm_for_node(&root);

    // 2. Iterate through all conformance fixtures
    let dir = corpus_dir();
    let mut checked = 0;
    for entry in fs::read_dir(&dir).expect("conformance/gcode exists") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let file_content = fs::read_to_string(&path).unwrap();
        let fx_val: serde_json::Value =
            serde_json::from_str(&file_content).unwrap_or_else(|e| panic!("parse {path:?}: {e}"));

        let design = fx_val["design"].as_str().unwrap().to_string();
        let params: EmitParams = serde_json::from_value(fx_val["params"].clone()).unwrap();
        assert_wasm_matches_native(
            &root,
            &design,
            &fx_val["l1"],
            &fx_val["resolve_params"],
            &params,
        );

        checked += 1;
    }
    assert!(checked >= 1, "no fixtures found in {dir:?}");
    println!("Verified bit-identical math backend behavior on {checked} conformance fixtures!");
}

#[test]
fn a_clothoid_is_bit_identical_on_wasm() {
    // `proofs/resolve-clothoid-numeric-profile-v0.toml` declares wasm32-unknown-unknown as a target,
    // and the clothoid's claim to native/wasm parity is that every transcendental it touches
    // (`hypot`, `atan2`, `sqrt`, `tan`, and the polynomial series) is `libm`. That was an argument,
    // not a measurement, because the corpus the parity harness walks is generated from the
    // FullControl oracle and the oracle has no clothoid — a fixture there would have to carry a
    // hand-written `expected`, which is Dry checking Dry in the one corpus whose value is that it
    // is not.
    //
    // So the design lives here instead, and goes through the same comparison: resolve to binary IR
    // and emit to g-code, natively and under Node, byte for byte. Two corners of opposite
    // handedness, an inherited Z, and a blend that consumes a whole leg, so the sampling, the
    // mirrored second half and the dropped empty leg are all in the compared bytes.
    let root = workspace_root();
    build_wasm_for_node(&root);

    let l1 = serde_json::json!({
        "ops": [
            {"op": "geometry", "width": 0.6, "height": 0.2},
            {"op": "extruder", "on": true},
            {"op": "move", "x": 0.0, "y": 0.0, "z": 0.2},
            {"op": "clothoid", "corner_x": 20.0, "corner_y": 0.0,
             "x": 20.0, "y": 20.0, "z": 1.2, "blend": 4.0},
            {"op": "clothoid", "corner_x": 0.0, "corner_y": 20.0,
             "x": 0.0, "y": 35.0, "z": null, "blend": 15.0}
        ]
    });
    let resolve_params = serde_json::json!({
        "print_speed": 1000.0, "travel_speed": 3000.0, "dia": 1.75
    });
    assert_wasm_matches_native(
        &root,
        "clothoid-corners",
        &l1,
        &resolve_params,
        &EmitParams::default(),
    );
}

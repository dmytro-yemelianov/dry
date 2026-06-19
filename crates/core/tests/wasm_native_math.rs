use dry_core::{emit, resolve, Design, EmitParams, ResolveParams, Toolpath};
use std::fs;
use std::path::PathBuf;
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

#[test]
fn wasm_and_native_math_are_bit_identical() {
    let root = workspace_root();

    // 1. Build the wasm package for Node
    let build_status = Command::new("bash")
        .arg("web/build.sh")
        .arg("nodejs")
        .arg("web/pkg-node")
        .current_dir(&root)
        .status()
        .expect("failed to run bash web/build.sh");
    assert!(build_status.success(), "wasm build failed");

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
        let l1: Design = serde_json::from_value(fx_val["l1"].clone()).unwrap();
        let resolve_params: ResolveParams =
            serde_json::from_value(fx_val["resolve_params"].clone()).unwrap();
        let params: EmitParams = serde_json::from_value(fx_val["params"].clone()).unwrap();

        // Native resolve
        let native_tp = resolve(&l1, &resolve_params);

        // Native emit
        let native_gcode = emit(&native_tp, &params);

        // Run Node to get the wasm resolve and emit
        let ops_json = serde_json::to_string(&fx_val["l1"]["ops"]).unwrap();
        let resolve_params_json = serde_json::to_string(&fx_val["resolve_params"]).unwrap();

        // Wasm resolve (as raw binary via resolve_binary)
        let js_resolve_code = format!(
            "const dry = require('./web/pkg-node/dry_wasm.js'); console.log(Buffer.from(dry.resolve_binary(JSON.stringify({}), JSON.stringify({}))).toString('hex'));",
            ops_json, resolve_params_json
        );
        let node_resolve_output = Command::new("node")
            .arg("-e")
            .arg(&js_resolve_code)
            .current_dir(&root)
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
            dry_core::Kinematics::Ac => "ac",
            dry_core::Kinematics::Bc => "bc",
            dry_core::Kinematics::Ab => "ab",
        };
        let js_emit_code = format!(
            "const dry = require('./web/pkg-node/dry_wasm.js'); console.log(JSON.stringify(dry.resolve_gcode(JSON.stringify({}), JSON.stringify({}), {}, {}, {}, '{}')));",
            ops_json,
            resolve_params_json,
            params.relative_e,
            params.travel_g1_e0,
            params.five_axis,
            kinematics_str
        );
        let node_emit_output = Command::new("node")
            .arg("-e")
            .arg(&js_emit_code)
            .current_dir(&root)
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
            "[{}] native and wasm resolved Toolpaths differ (not bit-identical)",
            design
        );

        // Compare native and wasm G-code
        assert_eq!(
            native_gcode, wasm_gcode,
            "[{}] native and wasm emitted G-code differ (not bit-identical)",
            design
        );

        checked += 1;
    }
    assert!(checked >= 1, "no fixtures found in {:?}", dir);
    println!(
        "Verified bit-identical math backend behavior on {} conformance fixtures!",
        checked
    );
}

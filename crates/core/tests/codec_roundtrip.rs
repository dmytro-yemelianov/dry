//! Conformance: the binary (columnar) Dry IR encoding round-trips losslessly (`from(to(ir)) == ir`)
//! for every oracle design, and is materially more compact than JSON (the P0.3 gate: ≥3× smaller on a
//! large fixture — here the ~120-segment spiral vase).

use dry_core::Toolpath;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn gcode_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/gcode")
}

/// Load the L2 IR embedded in a gcode fixture (`{"ir": {version, segments}}`).
fn load_ir(path: &std::path::Path) -> Toolpath {
    let v: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    serde_json::from_value(v.get("ir").cloned().unwrap()).unwrap()
}

#[test]
fn binary_round_trips_every_design() {
    let mut checked = 0;
    for entry in fs::read_dir(gcode_dir()).expect("conformance/gcode exists") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let tp = load_ir(&path);
        let bytes = tp.to_bytes();
        let back = Toolpath::from_bytes(&bytes).expect("decodes");
        assert_eq!(back, tp, "round-trip must be lossless for {path:?}");
        checked += 1;
    }
    assert!(checked >= 1, "no fixtures");
}

#[test]
fn binary_is_at_least_3x_smaller_than_json() {
    let tp = load_ir(&gcode_dir().join("spiral_vase.json"));
    assert!(tp.segments.len() >= 100, "spiral_vase should be large");
    let json = tp.to_json();
    let bytes = tp.to_bytes();
    assert!(
        bytes.len() * 3 <= json.len(),
        "binary {} bytes vs json {} bytes ({:.2}×)",
        bytes.len(),
        json.len(),
        json.len() as f64 / bytes.len() as f64
    );
    eprintln!(
        "spiral_vase: binary {} B vs json {} B ({:.2}× smaller)",
        bytes.len(),
        json.len(),
        json.len() as f64 / bytes.len() as f64
    );
}

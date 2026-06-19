//! Conformance: `resolve` the FullControl oracle's *own* L1 design (emitted into each fixture) to L2,
//! then `emit`, and assert the g-code matches the oracle **byte-for-byte** — for every design,
//! including the ~120-segment parametric spiral vase. This proves the deposition + arc math on real
//! geometry, clean-room (Dry resolves the oracle's design; it does not re-author it by hand).

use dry_core::{emit, resolve, simulate, Design, EmitParams, ResolveParams};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize)]
struct GcodeFixture {
    design: String,
    l1: Design,
    resolve_params: ResolveParams,
    params: EmitParams,
    expected: Vec<String>,
}

#[derive(Deserialize)]
struct SimFixture {
    expected: SimExpected,
}
#[derive(Deserialize)]
struct SimExpected {
    total_time_s: f64,
    extruded_volume: f64,
    segment_count: u64,
}

fn dir(kind: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../conformance/{kind}"))
}

#[test]
fn resolve_then_emit_matches_the_oracle() {
    let mut checked = 0;
    for entry in fs::read_dir(dir("gcode")).expect("conformance/gcode exists") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let fx: GcodeFixture = serde_json::from_str(&fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|e| panic!("parse {path:?}: {e}"));

        let tp = resolve(&fx.l1, &fx.resolve_params);
        let got = emit(&tp, &fx.params);
        assert_eq!(
            got, fx.expected,
            "[{}] resolve→emit must match the oracle g-code",
            fx.design
        );

        // resolve→simulate parity for the same design
        let sim: SimFixture = serde_json::from_str(
            &fs::read_to_string(dir("simulate").join(format!("{}.json", fx.design))).unwrap(),
        )
        .unwrap();
        let m = simulate(&tp);
        assert_eq!(
            m.segment_count, sim.expected.segment_count,
            "[{}] segment_count",
            fx.design
        );
        assert!(
            (m.total_time_s.value() - sim.expected.total_time_s).abs() < 1e-9,
            "[{}] time",
            fx.design
        );
        assert!(
            (m.extruded_volume.value() - sim.expected.extruded_volume).abs() < 1e-9,
            "[{}] volume",
            fx.design
        );
        checked += 1;
    }
    assert!(checked >= 1, "no fixtures");
    eprintln!(
        "resolve→emit conformance: {checked} designs reproduced the oracle (incl spiral_vase)"
    );
}

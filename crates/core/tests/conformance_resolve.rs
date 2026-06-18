//! Conformance: author a design in Dry's L1, `resolve` it to L2, `emit`, and assert the g-code matches
//! the FullControl oracle's for the equivalent design (the `conformance/gcode/<name>.json` fixtures).
//! This proves the deposition + arc math, clean-room — Dry *authors* a design and reproduces the
//! oracle's output. simulate parity over the resolved toolpath is checked too.

use dry_core::resolve::Op::*;
use dry_core::{emit, resolve, simulate, Design, EmitParams, ResolveParams};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

fn d(ops: Vec<dry_core::Op>) -> Design {
    Design { ops }
}

/// The L1 re-authoring of each oracle design (same geometry the oracle built in FullControl).
fn design(name: &str) -> Design {
    let g = || Geometry {
        width: 0.6,
        height: 0.2,
    };
    let on = || Extruder { on: true };
    let off = || Extruder { on: false };
    let mv = |x, y, z| Move {
        x: Some(x),
        y: Some(y),
        z: Some(z),
    };
    match name {
        "square" => d(vec![
            g(),
            on(),
            mv(0., 0., 0.2),
            mv(10., 0., 0.2),
            mv(10., 10., 0.2),
            mv(0., 10., 0.2),
            mv(0., 0., 0.2),
        ]),
        "travel_mix" => d(vec![
            g(),
            on(),
            mv(0., 0., 0.2),
            mv(8., 0., 0.2),
            off(),
            mv(20., 0., 0.2),
            on(),
            mv(28., 0., 0.2),
        ]),
        "ramp_speed" => d(vec![
            g(),
            on(),
            mv(0., 0., 0.3),
            Speed { print: 2400. },
            mv(15., 0., 0.3),
            Speed { print: 1200. },
            mv(15., 9., 0.3),
        ]),
        "arc_ccw" => d(vec![
            g(),
            on(),
            mv(10., 0., 0.2),
            Arc {
                cx: 0.,
                cy: 0.,
                x: Some(0.),
                y: Some(10.),
                z: None,
                clockwise: false,
            },
            mv(0., 20., 0.2),
        ]),
        "arc_cw" => d(vec![
            g(),
            on(),
            mv(0., 10., 0.2),
            Arc {
                cx: 0.,
                cy: 0.,
                x: Some(10.),
                y: Some(0.),
                z: None,
                clockwise: true,
            },
        ]),
        "arcs_mix" => d(vec![
            g(),
            on(),
            mv(20., 5., 0.4),
            Speed { print: 1800. },
            Arc {
                cx: 10.,
                cy: 5.,
                x: Some(0.),
                y: Some(5.),
                z: None,
                clockwise: true,
            },
            mv(0., 15., 0.4),
            Arc {
                cx: 10.,
                cy: 15.,
                x: Some(20.),
                y: Some(15.),
                z: None,
                clockwise: true,
            },
        ]),
        other => panic!("no L1 design authored for {other}"),
    }
}

#[derive(Deserialize)]
struct GcodeFixture {
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

fn corpus(kind: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../conformance/{kind}/{name}.json"))
}

#[test]
fn resolve_then_emit_matches_the_oracle() {
    let names = [
        "square",
        "travel_mix",
        "ramp_speed",
        "arc_ccw",
        "arc_cw",
        "arcs_mix",
    ];
    let params = ResolveParams::default();
    for name in names {
        let tp = resolve(&design(name), &params);

        // emit byte-for-byte vs the oracle g-code
        let want: GcodeFixture =
            serde_json::from_str(&fs::read_to_string(corpus("gcode", name)).unwrap()).unwrap();
        let got = emit(&tp, &EmitParams::default());
        assert_eq!(
            got, want.expected,
            "[{name}] resolve→emit must match the oracle g-code"
        );

        // simulate parity over the resolved toolpath
        let sim: SimFixture =
            serde_json::from_str(&fs::read_to_string(corpus("simulate", name)).unwrap()).unwrap();
        let m = simulate(&tp);
        assert_eq!(
            m.segment_count, sim.expected.segment_count,
            "[{name}] segment_count"
        );
        assert!(
            (m.total_time_s - sim.expected.total_time_s).abs() < 1e-9,
            "[{name}] time"
        );
        assert!(
            (m.extruded_volume - sim.expected.extruded_volume).abs() < 1e-9,
            "[{name}] volume"
        );
    }
}

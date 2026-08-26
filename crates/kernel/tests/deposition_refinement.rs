//! Native refinement checks for the exact-rational deposition fixture model.

use drymachina_kernel::{resolve_checked, Design, Op, ResolveParams};
use serde::Deserialize;

const FIXTURES: &str = include_str!("../../../proofs/fixtures/deposition-refinement-v0.json");

#[derive(Debug, Deserialize)]
struct FixtureDocument {
    schema_version: u32,
    model: String,
    model_checks: bool,
    cases: Vec<FixtureCase>,
}

#[derive(Debug, Deserialize)]
struct FixtureCase {
    id: String,
    travel: bool,
    length: NumberOrRational,
    width: NumberOrRational,
    height: NumberOrRational,
    flow: NumberOrRational,
    expected_volume: NumberOrRational,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum NumberOrRational {
    Float(f64),
    Rational { numerator: i64, denominator: u64 },
}

impl NumberOrRational {
    fn to_f64(&self) -> f64 {
        match *self {
            Self::Float(value) => value,
            Self::Rational {
                numerator,
                denominator,
            } => numerator as f64 / denominator as f64,
        }
    }
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-12 * expected.abs().max(1.0)
}

#[test]
fn native_resolve_refines_generated_deposition_corpus() {
    let document: FixtureDocument =
        serde_json::from_str(FIXTURES).expect("valid deposition fixture JSON");
    assert_eq!(document.schema_version, 1);
    assert_eq!(document.model, "deposition-refinement-v0");
    assert!(document.model_checks);
    assert_eq!(document.cases.len(), 4);

    let params = ResolveParams::default();
    let filament_area = std::f64::consts::PI * (params.dia / 2.0).powi(2);
    for fixture in &document.cases {
        let length = fixture.length.to_f64();
        let width = fixture.width.to_f64();
        let height = fixture.height.to_f64();
        let flow = fixture.flow.to_f64();
        let expected_volume = fixture.expected_volume.to_f64();
        let mut ops = vec![Op::Geometry {
            width: Some(width),
            height: Some(height),
        }];
        // Establish the explicit origin because production L1 starts with an optional position.
        ops.push(Op::Move {
            x: Some(0.0),
            y: Some(0.0),
            z: Some(0.0),
        });
        if !fixture.travel {
            ops.push(Op::Extruder { on: true });
            if flow != 1.0 {
                ops.push(Op::Flow { ratio: flow });
            }
        }
        ops.push(Op::Move {
            x: Some(length),
            y: Some(0.0),
            z: Some(0.0),
        });

        let toolpath = resolve_checked(&Design { ops }, &params)
            .unwrap_or_else(|error| panic!("{} failed resolve_checked: {error}", fixture.id));
        assert_eq!(toolpath.segments.len(), 2, "{} segment count", fixture.id);
        let segment = toolpath.segments.last().expect("motion segment");
        assert!(
            close(segment.volume.0, expected_volume),
            "{} volume actual={} expected={}",
            fixture.id,
            segment.volume.0,
            expected_volume
        );
        assert!(
            close(segment.filament.0, expected_volume / filament_area),
            "{} filament",
            fixture.id
        );
        assert_eq!(segment.travel, fixture.travel, "{} travel", fixture.id);
    }
}

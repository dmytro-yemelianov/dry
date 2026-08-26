//! Refinement test between Lean resolve-channels semantics and production Rust resolve_checked.

use drymachina_kernel::{resolve_checked, Design, Op, ResolveParams};
use serde::Deserialize;

const FIXTURES: &str = include_str!("../../../proofs/fixtures/resolve-channels-refinement-v0.json");

#[derive(Debug, Deserialize)]
struct FixtureDocument {
    schema_version: u32,
    model: String,
    model_checks: bool,
    cases: Vec<Fixture>,
}

#[derive(Debug, Deserialize)]
struct Fixture {
    id: String,
    ops: Vec<FixtureOp>,
    expected: Expected,
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
            NumberOrRational::Float(v) => v,
            NumberOrRational::Rational {
                numerator,
                denominator,
            } => numerator as f64 / denominator as f64,
        }
    }
}

#[derive(Debug, Deserialize)]
struct FixtureOp {
    #[serde(rename = "type")]
    op_type: String,
    finish: Option<Point>,
    speed: Option<NumberOrRational>,
    seconds: Option<NumberOrRational>,
    nozzle: Option<NumberOrRational>,
    ratio: Option<NumberOrRational>,
    index: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct Point {
    x: NumberOrRational,
    y: NumberOrRational,
    z: NumberOrRational,
}

#[derive(Debug, Deserialize)]
struct Expected {
    emitted_count: usize,
    segments: Vec<FixtureSegment>,
}

#[derive(Debug, Deserialize)]
struct FixtureSegment {
    _kind: Option<String>,
    temperature: Option<NumberOrRational>,
    fan: Option<NumberOrRational>,
    flow: Option<NumberOrRational>,
    tool: Option<u32>,
    dwell_seconds: Option<NumberOrRational>,
}

fn build_design(ops: &[FixtureOp]) -> Design {
    let mut core_ops = Vec::new();
    for op in ops {
        match op.op_type.as_str() {
            "move" => {
                let finish = op.finish.as_ref().expect("move op must have finish");
                core_ops.push(Op::Move {
                    x: Some(finish.x.to_f64()),
                    y: Some(finish.y.to_f64()),
                    z: Some(finish.z.to_f64()),
                });
            }
            "dwell" => {
                let seconds = op
                    .seconds
                    .as_ref()
                    .expect("dwell op must have seconds")
                    .to_f64();
                core_ops.push(Op::Dwell { seconds });
            }
            "temperature" => {
                let nozzle = op
                    .nozzle
                    .as_ref()
                    .expect("temperature op must have nozzle")
                    .to_f64();
                core_ops.push(Op::Temperature { nozzle });
            }
            "fan" => {
                let speed = op.speed.as_ref().expect("fan op must have speed").to_f64();
                core_ops.push(Op::Fan { speed });
            }
            "flow" => {
                let ratio = op.ratio.as_ref().expect("flow op must have ratio").to_f64();
                core_ops.push(Op::Flow { ratio });
            }
            "tool" => {
                let index = op.index.expect("tool op must have index");
                core_ops.push(Op::Tool { index });
            }
            other => panic!("unknown op type in fixture: {other}"),
        }
    }
    Design { ops: core_ops }
}

#[test]
fn native_resolve_channels_refines_generated_lean_corpus() {
    let document: FixtureDocument =
        serde_json::from_str(FIXTURES).expect("valid resolve-channels fixture JSON");
    assert_eq!(document.schema_version, 1);
    assert_eq!(document.model, "resolve-channels-refinement-v0");
    assert!(document.model_checks);
    assert_eq!(document.cases.len(), 6);

    for fixture in &document.cases {
        let design = build_design(&fixture.ops);
        let toolpath = resolve_checked(&design, &ResolveParams::default())
            .unwrap_or_else(|err| panic!("{} failed resolve_checked: {err}", fixture.id));

        assert_eq!(
            toolpath.segments.len(),
            fixture.expected.emitted_count,
            "{} emitted segment count",
            fixture.id
        );

        for (idx, (seg, exp)) in toolpath
            .segments
            .iter()
            .zip(&fixture.expected.segments)
            .enumerate()
        {
            assert_eq!(
                seg.temperature.map(|v| v.to_bits()),
                exp.temperature.as_ref().map(|v| v.to_f64().to_bits()),
                "{} seg[{idx}] temperature",
                fixture.id
            );
            assert_eq!(
                seg.fan.map(|v| v.to_bits()),
                exp.fan.as_ref().map(|v| v.to_f64().to_bits()),
                "{} seg[{idx}] fan",
                fixture.id
            );
            assert_eq!(
                seg.flow.map(|v| v.to_bits()),
                exp.flow.as_ref().map(|v| v.to_f64().to_bits()),
                "{} seg[{idx}] flow",
                fixture.id
            );
            assert_eq!(seg.tool, exp.tool, "{} seg[{idx}] tool", fixture.id);
            assert_eq!(
                seg.dwell_s.map(|v| v.to_bits()),
                exp.dwell_seconds.as_ref().map(|v| v.to_f64().to_bits()),
                "{} seg[{idx}] dwell_s",
                fixture.id
            );
        }
    }
}

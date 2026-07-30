//! Refinement test between Lean resolve-orientation semantics and production Rust resolve_checked and verify.

use dry_core::{resolve_checked, verify, Contracts, Design, Op, ResolveParams};
use serde::Deserialize;

const FIXTURES: &str =
    include_str!("../../../proofs/fixtures/resolve-orientation-refinement-v0.json");

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
struct FixtureOp {
    #[serde(rename = "type")]
    op_type: String,
    finish: Option<Point>,
    vector: Option<Vector>,
}

#[derive(Debug, Deserialize)]
struct Point {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Debug, Deserialize)]
struct Vector {
    i: Scalar,
    j: Scalar,
    k: Scalar,
}

#[derive(Debug, Deserialize)]
struct Scalar {
    numerator: i64,
    denominator: u64,
}

impl Scalar {
    fn to_f64(&self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }
}

#[derive(Debug, Deserialize)]
struct Expected {
    resolve_accepts: bool,
    emitted_count: usize,
    segment_orientations: Vec<Option<Vector>>,
    verifier_findings: Vec<String>,
}

fn build_design(ops: &[FixtureOp]) -> Design {
    let mut core_ops = Vec::new();
    for op in ops {
        match op.op_type.as_str() {
            "move" => {
                let finish = op.finish.as_ref().expect("move op must have finish");
                core_ops.push(Op::Move {
                    x: Some(finish.x),
                    y: Some(finish.y),
                    z: Some(finish.z),
                });
            }
            "orient" => {
                let vector = op.vector.as_ref().expect("orient op must have vector");
                core_ops.push(Op::Orient {
                    i: vector.i.to_f64(),
                    j: vector.j.to_f64(),
                    k: vector.k.to_f64(),
                });
            }
            other => panic!("unknown op type in fixture: {other}"),
        }
    }
    Design { ops: core_ops }
}

#[test]
fn native_resolve_orientation_refines_generated_lean_corpus() {
    let document: FixtureDocument =
        serde_json::from_str(FIXTURES).expect("valid resolve-orientation fixture JSON");
    assert_eq!(document.schema_version, 1);
    assert_eq!(document.model, "resolve-orientation-refinement-v0");
    assert!(document.model_checks);
    assert_eq!(document.cases.len(), 5);

    for fixture in &document.cases {
        let design = build_design(&fixture.ops);
        let resolved = resolve_checked(&design, &ResolveParams::default());
        assert_eq!(
            resolved.is_ok(),
            fixture.expected.resolve_accepts,
            "{} resolve acceptance",
            fixture.id
        );

        let Ok(toolpath) = resolved else {
            let error = resolved.expect_err("rejected fixture must return an error");
            assert!(
                error.to_string().contains("non-zero magnitude")
                    || error.to_string().contains("must be finite"),
                "{} rejection message: {error}",
                fixture.id
            );
            continue;
        };

        assert_eq!(
            toolpath.segments.len(),
            fixture.expected.emitted_count,
            "{} emitted segment count",
            fixture.id
        );

        for (idx, (seg, expected_orient)) in toolpath
            .segments
            .iter()
            .zip(&fixture.expected.segment_orientations)
            .enumerate()
        {
            match expected_orient {
                None => {
                    assert!(
                        seg.orientation.is_none(),
                        "{} seg[{idx}] expected None orientation, got {:?}",
                        fixture.id,
                        seg.orientation
                    );
                }
                Some(v) => {
                    let actual = seg.orientation.unwrap_or_else(|| {
                        panic!("{} seg[{idx}] expected orientation", fixture.id)
                    });
                    assert_eq!(
                        actual[0].to_bits(),
                        v.i.to_f64().to_bits(),
                        "{} seg[{idx}] orientation i",
                        fixture.id
                    );
                    assert_eq!(
                        actual[1].to_bits(),
                        v.j.to_f64().to_bits(),
                        "{} seg[{idx}] orientation j",
                        fixture.id
                    );
                    assert_eq!(
                        actual[2].to_bits(),
                        v.k.to_f64().to_bits(),
                        "{} seg[{idx}] orientation k",
                        fixture.id
                    );
                }
            }
        }

        let report = verify(&toolpath, &Contracts::default());
        let has_non_unit_finding = report
            .findings
            .iter()
            .any(|f| f.rule == "orientation-not-unit");

        let expects_non_unit_finding = fixture
            .expected
            .verifier_findings
            .iter()
            .any(|f| f == "orientation-not-unit");

        assert_eq!(
            has_non_unit_finding, expects_non_unit_finding,
            "{} verifier finding match",
            fixture.id
        );
    }
}

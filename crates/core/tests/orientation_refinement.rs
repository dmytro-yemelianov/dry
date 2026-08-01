//! Selected-corpus refinement between the exact Lean orientation contract and the native resolver
//! and verifier. The generated fixture intentionally distinguishes nonzero acceptance from unit
//! classification.

use dry_core::{resolve_checked, verify, Contracts, Design, Op, ResolveParams};
use serde::Deserialize;

const FIXTURES: &str =
    include_str!("../../../proofs/fixtures/orientation-contract-refinement-v0.json");

#[derive(Debug, Deserialize)]
struct FixtureDocument {
    schema_version: u32,
    model: String,
    model_checks: bool,
    unit_policy: String,
    native_unit_tolerance: f64,
    cases: Vec<Fixture>,
}

#[derive(Debug, Deserialize)]
struct Fixture {
    id: String,
    vector: Vector,
    expected: Expected,
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
    finding: String,
}

fn design(vector: &Vector) -> Design {
    Design {
        ops: vec![
            Op::Orient {
                i: vector.i.to_f64(),
                j: vector.j.to_f64(),
                k: vector.k.to_f64(),
            },
            Op::Move {
                x: Some(0.0),
                y: Some(0.0),
                z: Some(0.0),
            },
            Op::Move {
                x: Some(1.0),
                y: Some(0.0),
                z: Some(0.0),
            },
        ],
    }
}

#[test]
fn native_orientation_contract_refines_generated_lean_corpus() {
    let document: FixtureDocument =
        serde_json::from_str(FIXTURES).expect("valid orientation-contract fixture JSON");
    assert_eq!(document.schema_version, 1);
    assert_eq!(document.model, "orientation-contract-refinement-v0");
    assert!(document.model_checks);
    assert_eq!(
        document.unit_policy,
        "exact-rational-squared-norm-equals-one"
    );
    assert_eq!(document.native_unit_tolerance.to_bits(), 1e-6_f64.to_bits());
    assert_eq!(document.cases.len(), 6);

    for fixture in &document.cases {
        let resolved = resolve_checked(&design(&fixture.vector), &ResolveParams::default());
        assert_eq!(
            resolved.is_ok(),
            fixture.expected.resolve_accepts,
            "{} resolve outcome",
            fixture.id
        );

        let Ok(toolpath) = resolved else {
            let error = resolved.expect_err("rejected fixture must carry an error");
            assert!(
                error.to_string().contains("non-zero magnitude"),
                "{} rejection: {error}",
                fixture.id
            );
            assert_eq!(fixture.expected.finding, "not_evaluated");
            continue;
        };

        let has_non_unit_finding = verify(&toolpath, &Contracts::default())
            .findings
            .iter()
            .any(|finding| finding.rule == "orientation-not-unit");
        match fixture.expected.finding.as_str() {
            "none" => assert!(
                !has_non_unit_finding,
                "{} unexpected orientation-not-unit finding",
                fixture.id
            ),
            "orientation-not-unit" => assert!(
                has_non_unit_finding,
                "{} missing orientation-not-unit finding",
                fixture.id
            ),
            other => panic!(
                "{} has invalid finding expectation for an accepted vector: {other}",
                fixture.id
            ),
        }
    }
}

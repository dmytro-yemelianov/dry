use dry_core::{expand_features_with_limits, ExpandLimits, FeatureProgram, Op};
use serde::Deserialize;

const FIXTURES: &str = include_str!("../../../proofs/fixtures/feature-refinement-v0.json");

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
    limits: FixtureLimits,
    program: FeatureProgram,
    expected: Expected,
}

#[derive(Debug, Deserialize)]
struct FixtureLimits {
    max_ops: usize,
    max_nodes: usize,
    max_depth: usize,
}

impl From<&FixtureLimits> for ExpandLimits {
    fn from(value: &FixtureLimits) -> Self {
        Self {
            max_ops: value.max_ops,
            max_nodes: value.max_nodes,
            max_depth: value.max_depth,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum Expected {
    Ok { ops: Vec<ObservedOp> },
    Error { code: String, message: String },
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
enum ObservedOp {
    Tool { index: u32 },
    Move { x: f64, y: f64, z: f64 },
    ManualGcode { text: String },
}

#[derive(Debug, PartialEq)]
struct ObservedFailure {
    code: &'static str,
    message: String,
}

#[derive(Debug, PartialEq)]
enum Observation {
    Ok(Vec<ObservedOp>),
    Error(ObservedFailure),
}

fn normalize_op(op: &Op) -> ObservedOp {
    match op {
        Op::Tool { index } => ObservedOp::Tool { index: *index },
        Op::Move {
            x: Some(x),
            y: Some(y),
            z: Some(z),
        } => ObservedOp::Move {
            x: *x,
            y: *y,
            z: *z,
        },
        Op::ManualGcode { text } => ObservedOp::ManualGcode { text: text.clone() },
        other => panic!("refinement fixture emitted unsupported operation: {other:?}"),
    }
}

fn classify_failure(message: &str) -> &'static str {
    if message.contains("max feature depth") {
        "max-depth"
    } else if message.contains("max expanded nodes") {
        "max-nodes"
    } else if message.contains("max expanded ops") {
        "max-ops"
    } else if message.contains("is undefined; features must be locally self-contained") {
        "undefined-coordinate"
    } else if message.contains("manual_gcode cannot be transformed safely") {
        "transformed-manual"
    } else {
        "unclassified"
    }
}

fn observe(fixture: &Fixture) -> Observation {
    match expand_features_with_limits(&fixture.program, (&fixture.limits).into()) {
        Ok(design) => Observation::Ok(design.ops.iter().map(normalize_op).collect()),
        Err(error) => {
            let message = error.to_string();
            Observation::Error(ObservedFailure {
                code: classify_failure(&message),
                message,
            })
        }
    }
}

#[test]
fn rust_feature_expansion_refines_checked_lean_fixtures() {
    let document: FixtureDocument =
        serde_json::from_str(FIXTURES).expect("valid feature-refinement fixture JSON");
    assert_eq!(document.schema_version, 1);
    assert_eq!(document.model, "feature-refinement-v0");
    assert!(document.model_checks);
    assert_eq!(document.cases.len(), 11);

    for fixture in &document.cases {
        let first = observe(fixture);
        let second = observe(fixture);
        assert_eq!(
            first, second,
            "{} produced a nondeterministic observation",
            fixture.id
        );

        match (&fixture.expected, first) {
            (Expected::Ok { ops: expected }, Observation::Ok(actual)) => {
                assert_eq!(&actual, expected, "{} operation trace", fixture.id);
            }
            (
                Expected::Error {
                    code: expected_code,
                    message: expected_message,
                },
                Observation::Error(actual),
            ) => {
                assert_eq!(actual.code, expected_code, "{} failure code", fixture.id);
                assert_eq!(
                    &actual.message, expected_message,
                    "{} failure message",
                    fixture.id
                );
            }
            (expected, actual) => {
                panic!(
                    "{} outcome mismatch: expected {expected:?}, observed {actual:?}",
                    fixture.id
                );
            }
        }
    }
}

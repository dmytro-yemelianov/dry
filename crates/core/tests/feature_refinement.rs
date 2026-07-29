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

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
enum ObservedOp {
    Tool {
        index: u32,
    },
    Move {
        x: f64,
        y: f64,
        z: f64,
    },
    Arc {
        cx: f64,
        cy: f64,
        x: f64,
        y: f64,
        z: f64,
        clockwise: bool,
    },
    Spline {
        points: Vec<[f64; 3]>,
    },
    Orient {
        i: f64,
        j: f64,
        k: f64,
    },
    ManualGcode {
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct ObservedFailure {
    code: String,
    message: String,
}

#[derive(Clone, Debug, PartialEq)]
enum Observation {
    Ok(Vec<ObservedOp>),
    Error(ObservedFailure),
}

fn full_point(point: &[Option<f64>; 3]) -> [f64; 3] {
    point.map(|axis| axis.expect("expanded fixture point must define every axis"))
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
        Op::Arc {
            cx,
            cy,
            x: Some(x),
            y: Some(y),
            z: Some(z),
            clockwise,
        } => ObservedOp::Arc {
            cx: *cx,
            cy: *cy,
            x: *x,
            y: *y,
            z: *z,
            clockwise: *clockwise,
        },
        Op::Spline { points } => ObservedOp::Spline {
            points: points.iter().map(full_point).collect(),
        },
        Op::Orient { i, j, k } => ObservedOp::Orient {
            i: *i,
            j: *j,
            k: *k,
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
    } else if message.contains("requires a fully defined local start point") {
        "undefined-start"
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
                code: classify_failure(&message).to_owned(),
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
    assert_eq!(document.cases.len(), 17);

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
                assert_eq!(&actual.code, expected_code, "{} failure code", fixture.id);
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

#[derive(Clone, Copy, Debug)]
enum SemanticMutant {
    ReverseGroup,
    RepeatOnlyOnce,
    EvaluateZeroRepeatChild,
    ResetLocalPosition,
    CheckArcEndBeforeStart,
    CheckSplinePointBeforeStart,
    TranslateOrientation,
    AllowTransformedManual,
    AllowOneExtraOp,
    AllowOneExtraNode,
    CountNodeBeforeDepth,
}

fn failure(code: &str, message: &str) -> Observation {
    Observation::Error(ObservedFailure {
        code: code.to_owned(),
        message: message.to_owned(),
    })
}

fn mutate_observation(mutant: SemanticMutant, baseline: &Observation) -> Observation {
    match mutant {
        SemanticMutant::ReverseGroup => {
            let Observation::Ok(ops) = baseline else {
                panic!("reverse-group control requires a successful trace");
            };
            let mut reversed = ops.clone();
            reversed.reverse();
            Observation::Ok(reversed)
        }
        SemanticMutant::RepeatOnlyOnce => {
            let Observation::Ok(ops) = baseline else {
                panic!("repeat-once control requires a successful trace");
            };
            Observation::Ok(ops.iter().take(2).cloned().collect())
        }
        SemanticMutant::EvaluateZeroRepeatChild => failure(
            "undefined-coordinate",
            "features[0].instances[0].ops[0].y is undefined; features must be locally self-contained",
        ),
        SemanticMutant::ResetLocalPosition => failure(
            "undefined-coordinate",
            "features[0].ops[1].y is undefined; features must be locally self-contained",
        ),
        SemanticMutant::CheckArcEndBeforeStart => failure(
            "undefined-coordinate",
            "features[0].ops[0].y is undefined; features must be locally self-contained",
        ),
        SemanticMutant::CheckSplinePointBeforeStart => failure(
            "undefined-coordinate",
            "features[0].ops[0].points[0].y is undefined; features must be locally self-contained",
        ),
        SemanticMutant::TranslateOrientation => {
            Observation::Ok(vec![ObservedOp::Orient {
                i: 11.0,
                j: 2.0,
                k: 3.0,
            }])
        }
        SemanticMutant::AllowTransformedManual => {
            Observation::Ok(vec![ObservedOp::ManualGcode {
                text: "G28".to_owned(),
            }])
        }
        SemanticMutant::AllowOneExtraOp => Observation::Ok(vec![
            ObservedOp::Tool { index: 1 },
            ObservedOp::Tool { index: 2 },
            ObservedOp::Tool { index: 3 },
        ]),
        SemanticMutant::AllowOneExtraNode => Observation::Ok(vec![
            ObservedOp::Tool { index: 1 },
            ObservedOp::Tool { index: 1 },
            ObservedOp::Tool { index: 1 },
        ]),
        SemanticMutant::CountNodeBeforeDepth => failure(
            "max-nodes",
            "features[0].children[0] exceeds max expanded nodes (1)",
        ),
    }
}

#[test]
fn refinement_corpus_distinguishes_declared_semantic_mutants() {
    let document: FixtureDocument =
        serde_json::from_str(FIXTURES).expect("valid feature-refinement fixture JSON");
    let controls = [
        (SemanticMutant::ReverseGroup, "group-source-order"),
        (SemanticMutant::RepeatOnlyOnce, "repeat-count-and-order"),
        (
            SemanticMutant::EvaluateZeroRepeatChild,
            "repeat-zero-skips-invalid-child",
        ),
        (
            SemanticMutant::ResetLocalPosition,
            "move-inherits-local-position",
        ),
        (
            SemanticMutant::CheckArcEndBeforeStart,
            "arc-requires-local-start-before-end",
        ),
        (
            SemanticMutant::CheckSplinePointBeforeStart,
            "spline-requires-local-start-before-points",
        ),
        (
            SemanticMutant::TranslateOrientation,
            "orientation-ignores-translation",
        ),
        (
            SemanticMutant::AllowTransformedManual,
            "transformed-manual-gcode",
        ),
        (
            SemanticMutant::AllowOneExtraOp,
            "operation-budget-first-excess",
        ),
        (
            SemanticMutant::AllowOneExtraNode,
            "node-budget-first-excess",
        ),
        (
            SemanticMutant::CountNodeBeforeDepth,
            "depth-budget-before-node-visit",
        ),
    ];

    for (mutant, fixture_id) in controls {
        let fixture = document
            .cases
            .iter()
            .find(|fixture| fixture.id == fixture_id)
            .unwrap_or_else(|| panic!("missing fixture {fixture_id}"));
        let baseline = observe(fixture);
        let mutated = mutate_observation(mutant, &baseline);
        assert_ne!(
            baseline, mutated,
            "{fixture_id} failed to distinguish {mutant:?}"
        );
    }
}

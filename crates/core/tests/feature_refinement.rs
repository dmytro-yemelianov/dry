use dry_core::{
    expand_features_with_limits, ExpandLimits, FeatureNode, FeaturePose, FeatureProgram, Op,
};
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
    program: FixtureProgram,
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
struct FixtureProgram {
    features: Vec<FixtureNode>,
}

impl FixtureProgram {
    fn to_core(&self) -> Result<FeatureProgram, String> {
        Ok(FeatureProgram {
            features: self
                .features
                .iter()
                .map(FixtureNode::to_core)
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum FixtureNode {
    Feature {
        #[serde(default)]
        name: Option<String>,
        pose: FixturePose,
        ops: Vec<FixtureOp>,
    },
    Group {
        children: Vec<FixtureNode>,
    },
    Repeat {
        count: u32,
        step: FixturePose,
        child: Box<FixtureNode>,
    },
}

impl FixtureNode {
    fn to_core(&self) -> Result<FeatureNode, String> {
        match self {
            Self::Feature { name, pose, ops } => Ok(FeatureNode::Feature {
                name: name.clone(),
                pose: pose.to_core()?,
                ops: ops
                    .iter()
                    .map(FixtureOp::to_core)
                    .collect::<Result<_, _>>()?,
            }),
            Self::Group { children } => Ok(FeatureNode::Group {
                children: children
                    .iter()
                    .map(FixtureNode::to_core)
                    .collect::<Result<_, _>>()?,
            }),
            Self::Repeat { count, step, child } => Ok(FeatureNode::Repeat {
                count: *count,
                step: step.to_core()?,
                child: Box::new(child.to_core()?),
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
struct FixturePose {
    x: FixtureScalar,
    y: FixtureScalar,
    z: FixtureScalar,
    rotate_z_deg: FixtureScalar,
}

impl FixturePose {
    fn to_core(&self) -> Result<FeaturePose, String> {
        Ok(FeaturePose {
            x: self.x.to_f64()?,
            y: self.y.to_f64()?,
            z: self.z.to_f64()?,
            rotate_z_deg: self.rotate_z_deg.to_f64()?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum FixtureScalar {
    Finite(f64),
    Token(String),
}

impl FixtureScalar {
    fn to_f64(&self) -> Result<f64, String> {
        match self {
            Self::Finite(value) => Ok(*value),
            Self::Token(token) if token == "NaN" => Ok(f64::NAN),
            Self::Token(token) if token == "inf" => Ok(f64::INFINITY),
            Self::Token(token) if token == "-inf" => Ok(f64::NEG_INFINITY),
            Self::Token(token) => Err(format!("unsupported fixture scalar token {token:?}")),
        }
    }
}

fn optional_scalar(value: &Option<FixtureScalar>) -> Result<Option<f64>, String> {
    value.as_ref().map(FixtureScalar::to_f64).transpose()
}

fn partial_point(point: &[Option<FixtureScalar>; 3]) -> Result<[Option<f64>; 3], String> {
    Ok([
        optional_scalar(&point[0])?,
        optional_scalar(&point[1])?,
        optional_scalar(&point[2])?,
    ])
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum FixtureOp {
    Tool {
        index: u32,
    },
    Move {
        x: Option<FixtureScalar>,
        y: Option<FixtureScalar>,
        z: Option<FixtureScalar>,
    },
    Arc {
        cx: f64,
        cy: f64,
        x: Option<FixtureScalar>,
        y: Option<FixtureScalar>,
        z: Option<FixtureScalar>,
        clockwise: bool,
    },
    Spline {
        points: Vec<[Option<FixtureScalar>; 3]>,
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

impl FixtureOp {
    fn to_core(&self) -> Result<Op, String> {
        match self {
            Self::Tool { index } => Ok(Op::Tool { index: *index }),
            Self::Move { x, y, z } => Ok(Op::Move {
                x: optional_scalar(x)?,
                y: optional_scalar(y)?,
                z: optional_scalar(z)?,
            }),
            Self::Arc {
                cx,
                cy,
                x,
                y,
                z,
                clockwise,
            } => Ok(Op::Arc {
                cx: *cx,
                cy: *cy,
                x: optional_scalar(x)?,
                y: optional_scalar(y)?,
                z: optional_scalar(z)?,
                clockwise: *clockwise,
            }),
            Self::Spline { points } => Ok(Op::Spline {
                points: points.iter().map(partial_point).collect::<Result<_, _>>()?,
            }),
            Self::Orient { i, j, k } => Ok(Op::Orient {
                i: *i,
                j: *j,
                k: *k,
            }),
            Self::ManualGcode { text } => Ok(Op::ManualGcode { text: text.clone() }),
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
    } else if message.contains(".name must not be empty") {
        "empty-name"
    } else if message.contains("must be finite")
        && (message.contains(".pose.") || message.contains(".step."))
    {
        "non-finite-pose"
    } else if message.contains("is undefined; features must be locally self-contained") {
        "undefined-coordinate"
    } else if message.contains("must be finite") {
        "non-finite-coordinate"
    } else if message.contains("requires a fully defined local start point") {
        "undefined-start"
    } else if message.contains("manual_gcode cannot be transformed safely") {
        "transformed-manual"
    } else {
        "unclassified"
    }
}

fn observe(fixture: &Fixture) -> Observation {
    let program = fixture
        .program
        .to_core()
        .unwrap_or_else(|error| panic!("{} has invalid fixture syntax: {error}", fixture.id));
    match expand_features_with_limits(&program, (&fixture.limits).into()) {
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
    assert_eq!(document.cases.len(), 24);

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
    ValidatePoseBeforeName,
    ReversePoseFieldOrder,
    SkipZeroRepeatStepValidation,
    CheckFiniteBeforeMissing,
    AllowNonfiniteMove,
    AllowNonfiniteSpline,
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
        SemanticMutant::ValidatePoseBeforeName => failure(
            "non-finite-pose",
            "features[0].pose.x must be finite, got NaN",
        ),
        SemanticMutant::ReversePoseFieldOrder => failure(
            "non-finite-pose",
            "features[0].pose.z must be finite, got NaN",
        ),
        SemanticMutant::SkipZeroRepeatStepValidation => Observation::Ok(Vec::new()),
        SemanticMutant::CheckFiniteBeforeMissing => failure(
            "non-finite-coordinate",
            "features[0].ops[0].y must be finite, got NaN",
        ),
        SemanticMutant::AllowNonfiniteMove => Observation::Ok(vec![ObservedOp::Move {
            x: 1.0,
            y: f64::INFINITY,
            z: f64::NAN,
        }]),
        SemanticMutant::AllowNonfiniteSpline => Observation::Ok(vec![
            ObservedOp::Move {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            ObservedOp::Spline {
                points: vec![[1.0, f64::INFINITY, 0.0]],
            },
        ]),
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
        (
            SemanticMutant::ValidatePoseBeforeName,
            "empty-name-before-pose",
        ),
        (SemanticMutant::ReversePoseFieldOrder, "pose-field-order"),
        (
            SemanticMutant::SkipZeroRepeatStepValidation,
            "repeat-step-validated-at-zero-count",
        ),
        (
            SemanticMutant::CheckFiniteBeforeMissing,
            "undefined-before-later-nonfinite-coordinate",
        ),
        (
            SemanticMutant::AllowNonfiniteMove,
            "nonfinite-move-coordinate",
        ),
        (
            SemanticMutant::AllowNonfiniteSpline,
            "nonfinite-spline-point",
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

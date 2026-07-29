import Dry.Semantics.CheckedExpansion
import Lean.Data.Json

/-!
# Lean-generated Rust feature-refinement fixtures

The cases below are manually assigned expected success or first-error results, evaluated by the
checked Lean model and exported as a shared JSON corpus. The repository checker rejects the document
unless every model result agrees with its assigned expectation.

JSON strings `NaN`, `inf` and `-inf` are fixture-only tokens. The independent Rust adapter converts
them to IEEE-754 values before invoking the production feature expander.
-/

namespace Dry.Tests.FeatureRefinementFixtures

open Dry.Semantics.ExpandFeatures
open Dry.Semantics.CheckedExpansion

def generous : Limits :=
  ⟨100, 100, 10⟩

def point (x y z : Option Scalar) : PartialPoint :=
  ⟨x, y, z⟩

def pose
    (x : Scalar := 0)
    (y : Scalar := 0)
    (z : Scalar := 0)
    (rotateZDeg : Scalar := 0) : Pose :=
  ⟨x, y, z, rotateZDeg⟩

def feature (value : Pose) (ops : List SourceOp) : CheckedNode :=
  .feature none value ops

def namedFeature
    (name : Option String)
    (value : Pose)
    (ops : List SourceOp) : CheckedNode :=
  .feature name value ops

def program (features : List CheckedNode) : CheckedProgram :=
  ⟨features⟩

structure Fixture where
  id : String
  limits : Limits
  program : CheckedProgram
  expected : Except Failure (List OutputOp)

def fixtures : List Fixture :=
  [
    {
      id := "group-source-order"
      limits := generous
      program := program [
        .group [
          feature 0 [.tool 10],
          feature 0 [.tool 20]
        ]
      ]
      expected := .ok [.tool 10, .tool 20]
    },
    {
      id := "repeat-count-and-order"
      limits := generous
      program := program [
        .repeat 2 10 (
          .group [
            feature 0 [.tool 1],
            feature 0 [.tool 2]
          ])
      ]
      expected := .ok [.tool 1, .tool 2, .tool 1, .tool 2]
    },
    {
      id := "repeat-zero-skips-invalid-child"
      limits := generous
      program := program [
        .repeat 0 10 (
          feature 0 [.move (point (some 1) none (some 0))])
      ]
      expected := .ok []
    },
    {
      id := "identity-manual-gcode"
      limits := generous
      program := program [
        feature 0 [.manualGcode "M400"]
      ]
      expected := .ok [.manualGcode "M400"]
    },
    {
      id := "move-inherits-local-position"
      limits := generous
      program := program [
        feature 0 [
          .move (point (some 1) (some 2) (some 3)),
          .move (point (some 4) none none)
        ]
      ]
      expected := .ok [
        .move ⟨1, 2, 3⟩,
        .move ⟨4, 2, 3⟩
      ]
    },
    {
      id := "arc-requires-local-start-before-end"
      limits := generous
      program := program [
        feature 0 [
          .arc 0 0 (point (some 1) none (some 0)) false
        ]
      ]
      expected := .error ⟨.undefinedStart,
        "features[0].ops[0] requires a fully defined local start point"⟩
    },
    {
      id := "arc-after-local-start"
      limits := generous
      program := program [
        feature 0 [
          .move (point (some 0) (some 0) (some 0)),
          .arc 1 2 (point (some 2) (some 3) none) true
        ]
      ]
      expected := .ok [
        .move ⟨0, 0, 0⟩,
        .arc 1 2 ⟨2, 3, 0⟩ true
      ]
    },
    {
      id := "spline-requires-local-start-before-points"
      limits := generous
      program := program [
        feature 0 [
          .spline [point (some 1) none (some 0)]
        ]
      ]
      expected := .error ⟨.undefinedStart,
        "features[0].ops[0] requires a fully defined local start point"⟩
    },
    {
      id := "empty-spline-needs-no-start"
      limits := generous
      program := program [
        feature 0 [.spline []]
      ]
      expected := .ok [.spline []]
    },
    {
      id := "spline-points-inherit-locally"
      limits := generous
      program := program [
        feature 0 [
          .move (point (some 0) (some 1) (some 2)),
          .spline [
            point (some 1) none none,
            point (some 2) (some 3) none
          ]
        ]
      ]
      expected := .ok [
        .move ⟨0, 1, 2⟩,
        .spline [⟨1, 1, 2⟩, ⟨2, 3, 2⟩]
      ]
    },
    {
      id := "orientation-ignores-translation"
      limits := generous
      program := program [
        feature 10 [.orient 1 2 3]
      ]
      expected := .ok [.orient 1 2 3]
    },
    {
      id := "undefined-local-coordinate"
      limits := generous
      program := program [
        feature 0 [.move (point (some 1) none (some 0))]
      ]
      expected := .error ⟨.undefinedCoordinate,
        "features[0].ops[0].y is undefined; features must be locally self-contained"⟩
    },
    {
      id := "transformed-manual-gcode"
      limits := generous
      program := program [
        feature 10 [.manualGcode "G28"]
      ]
      expected := .error ⟨.transformedManual,
        "features[0].ops[0].manual_gcode cannot be transformed safely"⟩
    },
    {
      id := "source-order-first-error"
      limits := generous
      program := program [
        .group [
          feature 0 [.move (point (some 1) none (some 0))],
          feature 10 [.manualGcode "G28"]
        ]
      ]
      expected := .error ⟨.undefinedCoordinate,
        "features[0].children[0].ops[0].y is undefined; features must be locally self-contained"⟩
    },
    {
      id := "operation-budget-first-excess"
      limits := ⟨2, 100, 10⟩
      program := program [
        feature 0 [.tool 1, .tool 2, .tool 3]
      ]
      expected := .error ⟨.maxOps,
        "features[0].ops[2] exceeds max expanded ops (2)"⟩
    },
    {
      id := "node-budget-first-excess"
      limits := ⟨10, 2, 10⟩
      program := program [
        .repeat 3 0 (feature 0 [.tool 1])
      ]
      expected := .error ⟨.maxNodes,
        "features[0].instances[1] exceeds max expanded nodes (2)"⟩
    },
    {
      id := "depth-budget-before-node-visit"
      limits := ⟨10, 1, 0⟩
      program := program [
        .group [feature 0 [.tool 1]]
      ]
      expected := .error ⟨.maxDepth,
        "features[0].children[0] exceeds max feature depth (0)"⟩
    },
    {
      id := "empty-name-before-pose"
      limits := generous
      program := program [
        namedFeature (some "") (pose (.nonFinite "NaN")) [.tool 1]
      ]
      expected := .error ⟨.emptyName,
        "features[0].name must not be empty"⟩
    },
    {
      id := "nonfinite-feature-pose-x"
      limits := generous
      program := program [
        feature (pose (.nonFinite "NaN")) [.tool 1]
      ]
      expected := .error ⟨.nonFinitePose,
        "features[0].pose.x must be finite, got NaN"⟩
    },
    {
      id := "pose-field-order"
      limits := generous
      program := program [
        feature (pose 0 (.nonFinite "inf") (.nonFinite "NaN")) [.tool 1]
      ]
      expected := .error ⟨.nonFinitePose,
        "features[0].pose.y must be finite, got inf"⟩
    },
    {
      id := "repeat-step-validated-at-zero-count"
      limits := generous
      program := program [
        .repeat 0 (pose (.nonFinite "-inf"))
          (feature 0 [.move (point (some 1) none (some 0))])
      ]
      expected := .error ⟨.nonFinitePose,
        "features[0].step.x must be finite, got -inf"⟩
    },
    {
      id := "nonfinite-move-coordinate"
      limits := generous
      program := program [
        feature 0 [
          .move (point (some 1) (some (.nonFinite "inf"))
            (some (.nonFinite "NaN")))
        ]
      ]
      expected := .error ⟨.nonFiniteCoordinate,
        "features[0].ops[0].y must be finite, got inf"⟩
    },
    {
      id := "undefined-before-later-nonfinite-coordinate"
      limits := generous
      program := program [
        feature 0 [
          .move (point none (some (.nonFinite "NaN")) (some 0))
        ]
      ]
      expected := .error ⟨.undefinedCoordinate,
        "features[0].ops[0].x is undefined; features must be locally self-contained"⟩
    },
    {
      id := "nonfinite-spline-point"
      limits := generous
      program := program [
        feature 0 [
          .move (point (some 0) (some 0) (some 0)),
          .spline [
            point (some 1) (some (.nonFinite "inf")) none
          ]
        ]
      ]
      expected := .error ⟨.nonFiniteCoordinate,
        "features[0].ops[1].points[0].y must be finite, got inf"⟩
    },
    {
      id := "arc-end-before-nonfinite-centre"
      limits := generous
      program := program [
        feature 0 [
          .move (point (some 0) (some 0) (some 0)),
          .arc (.nonFinite "NaN") 0
            (point (some 1) (some (.nonFinite "inf")) none) false
        ]
      ]
      expected := .error ⟨.nonFiniteCoordinate,
        "features[0].ops[1].y must be finite, got inf"⟩
    },
    {
      id := "arc-centre-field-order"
      limits := generous
      program := program [
        feature 0 [
          .move (point (some 0) (some 0) (some 0)),
          .arc (.nonFinite "inf") (.nonFinite "NaN")
            (point (some 1) (some 2) none) false
        ]
      ]
      expected := .error ⟨.nonFiniteCoordinate,
        "features[0].ops[1].cx must be finite, got inf"⟩
    },
    {
      id := "nonfinite-orientation"
      limits := generous
      program := program [
        feature 0 [
          .orient (.nonFinite "NaN") 0 1
        ]
      ]
      expected := .error ⟨.nonFiniteCoordinate,
        "features[0].ops[0].i must be finite, got NaN"⟩
    },
    {
      id := "orientation-field-order"
      limits := generous
      program := program [
        feature 0 [
          .orient 1 (.nonFinite "inf") (.nonFinite "NaN")
        ]
      ]
      expected := .error ⟨.nonFiniteCoordinate,
        "features[0].ops[0].j must be finite, got inf"⟩
    }
  ]

def resultMatches
    (actual expected : Except Failure (List OutputOp)) : Bool :=
  match actual, expected with
  | .ok actualOps, .ok expectedOps => actualOps == expectedOps
  | .error actualFailure, .error expectedFailure =>
      actualFailure == expectedFailure
  | _, _ => false

def modelChecks : Bool :=
  fixtures.all fun fixture =>
    resultMatches
      (evaluate fixture.limits fixture.program)
      fixture.expected

open Lean

def listJson (encode : α → Json) (values : List α) : Json :=
  .arr (values.toArray.map encode)

def scalarJson : Scalar → Json
  | .finite value => .num value
  | .nonFinite rendered => .str rendered

def optionScalarJson : Option Scalar → Json
  | none => .null
  | some value => scalarJson value

def poseJson (value : Pose) : Json :=
  Json.mkObj [
    ("x", scalarJson value.x),
    ("y", scalarJson value.y),
    ("z", scalarJson value.z),
    ("rotate_z_deg", scalarJson value.rotateZDeg)
  ]

def sourceOpJson : SourceOp → Json
  | .tool index =>
      Json.mkObj [
        ("op", .str "tool"),
        ("index", .num index)
      ]
  | .move value =>
      Json.mkObj [
        ("op", .str "move"),
        ("x", optionScalarJson value.x),
        ("y", optionScalarJson value.y),
        ("z", optionScalarJson value.z)
      ]
  | .arc cx cy finish clockwise =>
      Json.mkObj [
        ("op", .str "arc"),
        ("cx", scalarJson cx),
        ("cy", scalarJson cy),
        ("x", optionScalarJson finish.x),
        ("y", optionScalarJson finish.y),
        ("z", optionScalarJson finish.z),
        ("clockwise", .bool clockwise)
      ]
  | .spline points =>
      Json.mkObj [
        ("op", .str "spline"),
        ("points", .arr (points.toArray.map fun value =>
          .arr #[
            optionScalarJson value.x,
            optionScalarJson value.y,
            optionScalarJson value.z
          ]))
      ]
  | .orient i j k =>
      Json.mkObj [
        ("op", .str "orient"),
        ("i", scalarJson i),
        ("j", scalarJson j),
        ("k", scalarJson k)
      ]
  | .manualGcode text =>
      Json.mkObj [
        ("op", .str "manual_gcode"),
        ("text", .str text)
      ]

mutual

  def nodeJson : CheckedNode → Json
    | .feature name value ops =>
        Json.mkObj ([
          ("kind", .str "feature"),
          ("pose", poseJson value),
          ("ops", listJson sourceOpJson ops)
        ] ++
          match name with
          | none => []
          | some text => [("name", .str text)])
    | .group children =>
        Json.mkObj [
          ("kind", .str "group"),
          ("children", .arr (nodesJson children).toArray)
        ]
    | .repeat count step child =>
        Json.mkObj [
          ("kind", .str "repeat"),
          ("count", .num count),
          ("step", poseJson step),
          ("child", nodeJson child)
        ]

  def nodesJson : List CheckedNode → List Json
    | [] => []
    | node :: rest => nodeJson node :: nodesJson rest

end

def programJson (value : CheckedProgram) : Json :=
  Json.mkObj [("features", .arr (nodesJson value.features).toArray)]

def limitsJson (value : Limits) : Json :=
  Json.mkObj [
    ("max_ops", .num value.maxOps),
    ("max_nodes", .num value.maxNodes),
    ("max_depth", .num value.maxDepth)
  ]

def outputOpJson : OutputOp → Json
  | .tool index =>
      Json.mkObj [
        ("op", .str "tool"),
        ("index", .num index)
      ]
  | .move value =>
      Json.mkObj [
        ("op", .str "move"),
        ("x", .num value.x),
        ("y", .num value.y),
        ("z", .num value.z)
      ]
  | .arc cx cy finish clockwise =>
      Json.mkObj [
        ("op", .str "arc"),
        ("cx", .num cx),
        ("cy", .num cy),
        ("x", .num finish.x),
        ("y", .num finish.y),
        ("z", .num finish.z),
        ("clockwise", .bool clockwise)
      ]
  | .spline points =>
      Json.mkObj [
        ("op", .str "spline"),
        ("points", .arr (points.toArray.map fun value =>
          .arr #[.num value.x, .num value.y, .num value.z]))
      ]
  | .orient i j k =>
      Json.mkObj [
        ("op", .str "orient"),
        ("i", .num i),
        ("j", .num j),
        ("k", .num k)
      ]
  | .manualGcode text =>
      Json.mkObj [
        ("op", .str "manual_gcode"),
        ("text", .str text)
      ]

def failureCodeLabel : FailureCode → String
  | .maxDepth => "max-depth"
  | .maxNodes => "max-nodes"
  | .maxOps => "max-ops"
  | .emptyName => "empty-name"
  | .nonFinitePose => "non-finite-pose"
  | .undefinedCoordinate => "undefined-coordinate"
  | .nonFiniteCoordinate => "non-finite-coordinate"
  | .undefinedStart => "undefined-start"
  | .transformedManual => "transformed-manual"

def expectedJson : Except Failure (List OutputOp) → Json
  | .ok ops =>
      Json.mkObj [
        ("outcome", .str "ok"),
        ("ops", listJson outputOpJson ops)
      ]
  | .error failure =>
      Json.mkObj [
        ("outcome", .str "error"),
        ("code", .str (failureCodeLabel failure.code)),
        ("message", .str failure.message)
      ]

def fixtureJson (fixture : Fixture) : Json :=
  Json.mkObj [
    ("id", .str fixture.id),
    ("limits", limitsJson fixture.limits),
    ("program", programJson fixture.program),
    ("expected", expectedJson fixture.expected)
  ]

def fixtureDocument : Json :=
  Json.mkObj [
    ("schema_version", .num 1),
    ("model", .str "feature-refinement-v0"),
    ("model_checks", .bool modelChecks),
    ("cases", listJson fixtureJson fixtures)
  ]

def render : String :=
  Json.pretty fixtureDocument 100

end Dry.Tests.FeatureRefinementFixtures

def main : IO Unit :=
  IO.println Dry.Tests.FeatureRefinementFixtures.render

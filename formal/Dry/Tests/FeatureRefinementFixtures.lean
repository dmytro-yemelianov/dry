import Dry.Semantics.CheckedExpansion
import Lean.Data.Json

/-!
# Lean-generated Rust feature-refinement fixtures

The cases below are manually assigned expected success or first-error results, evaluated by the
checked Lean model and exported as a shared JSON corpus. The repository checker rejects the document
unless every model result agrees with its assigned expectation.
-/

namespace Dry.Tests.FeatureRefinementFixtures

open Dry.Semantics.ExpandFeatures
open Dry.Semantics.CheckedExpansion

def generous : Limits :=
  ⟨100, 100, 10⟩

def point (x y z : Option Nat) : PartialPoint :=
  ⟨x, y, z⟩

def feature (pose : Nat) (ops : List SourceOp) : Node Nat SourceOp :=
  .feature pose ops

def program (features : List (Node Nat SourceOp)) : Program Nat SourceOp :=
  ⟨features⟩

structure Fixture where
  id : String
  limits : Limits
  program : Program Nat SourceOp
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
      limits := ⟨10, 100, 0⟩
      program := program [
        .group [feature 0 [.tool 1]]
      ]
      expected := .error ⟨.maxDepth,
        "features[0].children[0] exceeds max feature depth (0)"⟩
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

def optionNatJson : Option Nat → Json
  | none => .null
  | some value => .num value

def sourceOpJson : SourceOp → Json
  | .tool index =>
      Json.mkObj [
        ("op", .str "tool"),
        ("index", .num index)
      ]
  | .move value =>
      Json.mkObj [
        ("op", .str "move"),
        ("x", optionNatJson value.x),
        ("y", optionNatJson value.y),
        ("z", optionNatJson value.z)
      ]
  | .manualGcode text =>
      Json.mkObj [
        ("op", .str "manual_gcode"),
        ("text", .str text)
      ]

mutual

  def nodeJson : Node Nat SourceOp → Json
    | .feature pose ops =>
        Json.mkObj [
          ("kind", .str "feature"),
          ("pose", Json.mkObj [("x", .num pose)]),
          ("ops", listJson sourceOpJson ops)
        ]
    | .group children =>
        Json.mkObj [
          ("kind", .str "group"),
          ("children", .arr (nodesJson children).toArray)
        ]
    | .repeat count step child =>
        Json.mkObj [
          ("kind", .str "repeat"),
          ("count", .num count),
          ("step", Json.mkObj [("x", .num step)]),
          ("child", nodeJson child)
        ]

  def nodesJson : List (Node Nat SourceOp) → List Json
    | [] => []
    | node :: rest => nodeJson node :: nodesJson rest

end

def programJson (value : Program Nat SourceOp) : Json :=
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
  | .manualGcode text =>
      Json.mkObj [
        ("op", .str "manual_gcode"),
        ("text", .str text)
      ]

def failureCodeLabel : FailureCode → String
  | .maxDepth => "max-depth"
  | .maxNodes => "max-nodes"
  | .maxOps => "max-ops"
  | .undefinedCoordinate => "undefined-coordinate"
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

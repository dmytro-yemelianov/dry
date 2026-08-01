import Dry.Semantics.CompositionTreeRefinement
import Lean.Data.Json
import Mathlib.Tactic

/-!
# Lean-generated nested transform-application refinement fixtures

These fixtures close a selected-corpus gap between the parenthesized Feature/Repeat tree witnesses
and the local native application witnesses. An executable integer quarter-turn model expands
nested `Repeat → Repeat → Feature` programs containing points, Arc centres and orientation vectors.
Rust consumes the generated programs through the production feature expander and compares every
result with the exact model under ceilings that are stricter than the published tree-application
budgets.

This remains finite implementation evidence. It does not establish the range, rounding or libm
premises for arbitrary nested programs.
-/

namespace Dry.Tests.NestedApplicationFixtures

structure Point where
  x : Int
  y : Int
  z : Int
deriving BEq, Repr

def Point.add (left right : Point) : Point :=
  ⟨left.x + right.x, left.y + right.y, left.z + right.z⟩

structure Pose where
  translation : Point
  quarterTurns : Nat
deriving BEq, Repr

def Pose.identity : Pose :=
  ⟨⟨0, 0, 0⟩, 0⟩

structure Transform where
  translation : Point
  quarterTurns : Nat
deriving BEq, Repr

def Transform.identity : Transform :=
  ⟨⟨0, 0, 0⟩, 0⟩

def rotate (quarterTurns : Nat) (point : Point) : Point :=
  match quarterTurns % 4 with
  | 0 => point
  | 1 => ⟨-point.y, point.x, point.z⟩
  | 2 => ⟨-point.x, -point.y, point.z⟩
  | _ => ⟨point.y, -point.x, point.z⟩

def Transform.fromPose (pose : Pose) : Transform :=
  ⟨pose.translation, pose.quarterTurns % 4⟩

/-- Parent-first composition: `outer(inner(point))`. -/
def Transform.compose (outer inner : Transform) : Transform :=
  {
    translation :=
      (rotate outer.quarterTurns inner.translation).add outer.translation
    quarterTurns := (outer.quarterTurns + inner.quarterTurns) % 4
  }

def Transform.applyPoint (transform : Transform) (point : Point) : Point :=
  (rotate transform.quarterTurns point).add transform.translation

def Transform.applyVector (transform : Transform) (vector : Point) : Point :=
  rotate transform.quarterTurns vector

def repeatInstance (step : Transform) : Nat → Transform
  | 0 => .identity
  | index + 1 => (repeatInstance step index).compose step

inductive Op where
  | move (point : Point)
  | arc (centreX centreY : Int) (endpoint : Point) (clockwise : Bool)
  | orient (vector : Point)
deriving BEq, Repr

def Op.apply (transform : Transform) : Op → Op
  | .move point => .move (transform.applyPoint point)
  | .arc centreX centreY endpoint clockwise =>
      let centre := transform.applyPoint ⟨centreX, centreY, 0⟩
      .arc centre.x centre.y (transform.applyPoint endpoint) clockwise
  | .orient vector => .orient (transform.applyVector vector)

inductive Node where
  | feature (pose : Pose) (ops : List Op)
  | repeat (count : Nat) (step : Pose) (child : Node)
deriving BEq, Repr

def Node.expand (node : Node) (parent : Transform := .identity) : List Op :=
  match node with
  | .feature pose ops =>
      ops.map (Op.apply (parent.compose (Transform.fromPose pose)))
  | .repeat count step child =>
      (List.range count).flatMap fun index =>
        child.expand
          (parent.compose (repeatInstance (Transform.fromPose step) index))

structure Fixture where
  id : String
  root : Node
  expected : List Op

def translationPose (x y z : Int) : Pose :=
  ⟨⟨x, y, z⟩, 0⟩

def rotationPose (quarterTurns : Nat) : Pose :=
  ⟨⟨0, 0, 0⟩, quarterTurns⟩

def nested (ops : List Op) : Node :=
  .repeat 2 (translationPose 100 0 0)
    (.repeat 2 (rotationPose 1)
      (.feature (translationPose 10 0 3) ops))

def fixtures : List Fixture :=
  [
    {
      id := "nested-application-point"
      root := nested [.move ⟨5, 2, 7⟩]
      expected := [
        .move ⟨15, 2, 10⟩,
        .move ⟨-2, 15, 10⟩,
        .move ⟨115, 2, 10⟩,
        .move ⟨98, 15, 10⟩
      ]
    },
    {
      id := "nested-application-arc-center"
      root := nested [
        .move ⟨0, 0, 0⟩,
        .arc 4 8 ⟨6, 9, 10⟩ true
      ]
      expected := [
        .move ⟨10, 0, 3⟩,
        .arc 14 8 ⟨16, 9, 13⟩ true,
        .move ⟨0, 10, 3⟩,
        .arc (-8) 14 ⟨-9, 16, 13⟩ true,
        .move ⟨110, 0, 3⟩,
        .arc 114 8 ⟨116, 9, 13⟩ true,
        .move ⟨100, 10, 3⟩,
        .arc 92 14 ⟨91, 16, 13⟩ true
      ]
    },
    {
      id := "nested-application-orientation"
      root := nested [.orient ⟨1, -1, 1⟩]
      expected := [
        .orient ⟨1, -1, 1⟩,
        .orient ⟨1, 1, 1⟩,
        .orient ⟨1, -1, 1⟩,
        .orient ⟨1, 1, 1⟩
      ]
    }
  ]

def modelChecks : Bool :=
  fixtures.all fun fixture =>
    fixture.root.expand == fixture.expected

theorem modelChecks_eq_true : modelChecks = true := by
  native_decide

def observationPointXYCeiling : ℚ := 1 / 2 ^ 28
def observationPointZCeiling : ℚ := 1 / 2 ^ 30
def observationOrientationXYCeiling : ℚ := 1 / 2 ^ 29
def profilePointXYCeiling : ℚ := 2 ^ 31
def profilePointZCeiling : ℚ := 1 / 2 ^ 12
def profileOrientationXYCeiling : ℚ := 1 / 2 ^ 8

/-- The selected observation tolerances imply the published tree-application ceilings. -/
theorem observationCeilings_within_profile :
    observationPointXYCeiling ≤ profilePointXYCeiling ∧
      observationPointZCeiling ≤ profilePointZCeiling ∧
      observationOrientationXYCeiling ≤ profileOrientationXYCeiling := by
  norm_num [observationPointXYCeiling, observationPointZCeiling,
    observationOrientationXYCeiling, profilePointXYCeiling,
    profilePointZCeiling, profileOrientationXYCeiling]

theorem nestedApplicationFixtureChecks :
    modelChecks = true ∧
      observationPointXYCeiling ≤ profilePointXYCeiling ∧
      observationPointZCeiling ≤ profilePointZCeiling ∧
      observationOrientationXYCeiling ≤ profileOrientationXYCeiling :=
  ⟨modelChecks_eq_true, observationCeilings_within_profile⟩

open Lean

def pointJson (point : Point) : Json :=
  .arr #[.num point.x, .num point.y, .num point.z]

def poseJson (pose : Pose) : Json :=
  Json.mkObj [
    ("x", .num pose.translation.x),
    ("y", .num pose.translation.y),
    ("z", .num pose.translation.z),
    ("rotate_z_deg", .num ((pose.quarterTurns * 90 : Nat) : JsonNumber))
  ]

def opJson : Op → Json
  | .move point =>
      Json.mkObj [
        ("op", .str "move"),
        ("x", .num point.x),
        ("y", .num point.y),
        ("z", .num point.z)
      ]
  | .arc centreX centreY endpoint clockwise =>
      Json.mkObj [
        ("op", .str "arc"),
        ("cx", .num centreX),
        ("cy", .num centreY),
        ("x", .num endpoint.x),
        ("y", .num endpoint.y),
        ("z", .num endpoint.z),
        ("clockwise", .bool clockwise)
      ]
  | .orient vector =>
      Json.mkObj [
        ("op", .str "orient"),
        ("i", .num vector.x),
        ("j", .num vector.y),
        ("k", .num vector.z)
      ]

mutual

  def nodeJson : Node → Json
    | .feature pose ops =>
        Json.mkObj [
          ("kind", .str "feature"),
          ("pose", poseJson pose),
          ("ops", .arr (ops.toArray.map opJson))
        ]
    | .repeat count step child =>
        Json.mkObj [
          ("kind", .str "repeat"),
          ("count", .num count),
          ("step", poseJson step),
          ("child", nodeJson child)
        ]

end

def pow2NegativeJson (exponent : Nat) : Json :=
  .num ⟨(5 ^ exponent : Nat), exponent⟩

def budgetJson (id : String) (ceiling : Json) : Json :=
  Json.mkObj [
    ("id", .str id),
    ("ceiling", ceiling)
  ]

def fixtureJson (fixture : Fixture) : Json :=
  Json.mkObj [
    ("id", .str fixture.id),
    ("limits", Json.mkObj [
      ("max_ops", .num 100),
      ("max_nodes", .num 100),
      ("max_depth", .num 10)
    ]),
    ("program", Json.mkObj [
      ("features", .arr #[nodeJson fixture.root])
    ]),
    ("expected_ops", .arr (fixture.expected.toArray.map opJson))
  ]

def fixtureDocument : Json :=
  Json.mkObj [
    ("schema_version", .num 1),
    ("model", .str "nested-application-refinement-v0"),
    ("model_checks", .bool modelChecks),
    ("profile_budgets", Json.mkObj [
      ("point_xy", budgetJson
        "FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.BUDGET.COMPOSITION_TREE_POINT_COMPONENT_ABS_ERROR_MM"
        (.num (2 ^ 31 : Nat))),
      ("orientation_xy", budgetJson
        "FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.BUDGET.COMPOSITION_TREE_ORIENTATION_COMPONENT_ABS_ERROR"
        (pow2NegativeJson 8))
    ]),
    ("derived_profile_ceilings", Json.mkObj [
      ("point_z", pow2NegativeJson 12)
    ]),
    ("observation_ceilings", Json.mkObj [
      ("point_xy", pow2NegativeJson 28),
      ("point_z", pow2NegativeJson 30),
      ("orientation_xy", pow2NegativeJson 29),
      ("orientation_z", .num 0)
    ]),
    ("cases", .arr (fixtures.toArray.map fixtureJson))
  ]

def render : String :=
  Json.pretty fixtureDocument 100

end Dry.Tests.NestedApplicationFixtures

def main : IO Unit :=
  IO.println Dry.Tests.NestedApplicationFixtures.render

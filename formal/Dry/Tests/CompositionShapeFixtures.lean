import Dry.Semantics.CompositionTreeRefinement
import Lean.Data.Json

/-!
# Lean-generated composition-shape refinement fixtures

The ordinary feature-refinement corpus intentionally uses only exact natural-number X translations.
These additional fixtures use an executable integer quarter-turn model to distinguish parent-first
composition from reversed operands. Rust consumes the same programs and compares their binary64
endpoints within the declared `1e-12` observation tolerance.

This corpus checks expression shape, not a general trigonometric accuracy theorem.
-/

namespace Dry.Tests.CompositionShapeFixtures

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

def repeatInstance (step : Transform) : Nat → Transform
  | 0 => .identity
  | index + 1 => (repeatInstance step index).compose step

inductive Node where
  | feature (pose : Pose) (points : List Point)
  | repeat (count : Nat) (step : Pose) (child : Node)
deriving BEq, Repr

def Node.expand (node : Node) (parent : Transform := .identity) : List Point :=
  match node with
  | .feature pose points =>
      points.map (parent.compose (Transform.fromPose pose)).applyPoint
  | .repeat count step child =>
      (List.range count).flatMap fun index =>
        child.expand
          (parent.compose (repeatInstance (Transform.fromPose step) index))

structure Fixture where
  id : String
  root : Node
  expected : List Point

def translationPose (x y z : Int) : Pose :=
  ⟨⟨x, y, z⟩, 0⟩

def rotationPose (quarterTurns : Nat) : Pose :=
  ⟨⟨0, 0, 0⟩, quarterTurns⟩

def fixtures : List Fixture :=
  [
    {
      id := "feature-compose-parent-first"
      root :=
        .repeat 2 (rotationPose 1)
          (.feature (translationPose 10 0 0) [⟨0, 0, 0⟩])
      expected := [⟨10, 0, 0⟩, ⟨0, 10, 0⟩]
    },
    {
      id := "repeat-compose-parent-first"
      root :=
        .repeat 2 (translationPose 100 0 0)
          (.repeat 2 (rotationPose 1)
            (.feature .identity [⟨10, 0, 0⟩]))
      expected := [
        ⟨10, 0, 0⟩,
        ⟨0, 10, 0⟩,
        ⟨110, 0, 0⟩,
        ⟨100, 10, 0⟩
      ]
    }
  ]

def modelChecks : Bool :=
  fixtures.all fun fixture =>
    fixture.root.expand == fixture.expected

theorem modelChecks_eq_true : modelChecks = true := by
  native_decide

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

mutual

  def nodeJson : Node → Json
    | .feature pose points =>
        Json.mkObj [
          ("kind", .str "feature"),
          ("pose", poseJson pose),
          ("ops", .arr (points.toArray.map fun point =>
            Json.mkObj [
              ("op", .str "move"),
              ("x", .num point.x),
              ("y", .num point.y),
              ("z", .num point.z)
            ]))
        ]
    | .repeat count step child =>
        Json.mkObj [
          ("kind", .str "repeat"),
          ("count", .num count),
          ("step", poseJson step),
          ("child", nodeJson child)
        ]

end

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
    ("expected_points", .arr (fixture.expected.toArray.map pointJson)),
    ("tolerance", .num ⟨1, 12⟩)
  ]

def fixtureDocument : Json :=
  Json.mkObj [
    ("schema_version", .num 1),
    ("model", .str "composition-shape-refinement-v0"),
    ("model_checks", .bool modelChecks),
    ("cases", .arr (fixtures.toArray.map fixtureJson))
  ]

def render : String :=
  Json.pretty fixtureDocument 100

end Dry.Tests.CompositionShapeFixtures

def main : IO Unit :=
  IO.println Dry.Tests.CompositionShapeFixtures.render

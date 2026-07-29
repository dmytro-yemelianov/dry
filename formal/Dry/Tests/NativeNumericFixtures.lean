import Lean.Data.Json
import Mathlib.Analysis.Real.Pi.Bounds

/-!
# Lean-generated native feature numeric interval fixtures

These cardinal-angle cases make selected native `f64` boundaries executable without claiming a
universal implementation proof. Radian reference intervals use Mathlib's checked 20-decimal real-π
bounds. Published binary64 ceilings are emitted exactly as terminating decimal powers of two.

The compose cases also carry exact quarter-turn results. The Rust consumer checks the local compose
operation graph against the exact dyadic values of the actual binary64 input transforms, rather than
quietly comparing one rounded Rust expression with another.
-/

namespace Dry.Tests.NativeNumericFixtures

structure Point where
  x : Int
  y : Int
  z : Int
deriving BEq, Repr

def Point.add (left right : Point) : Point :=
  ⟨left.x + right.x, left.y + right.y, left.z + right.z⟩

structure Pose where
  translation : Point
  quarterTurns : Int
deriving BEq, Repr

def Pose.identity : Pose :=
  ⟨⟨0, 0, 0⟩, 0⟩

def rotate (quarterTurns : Int) (point : Point) : Point :=
  match quarterTurns % 4 with
  | 0 => point
  | 1 => ⟨-point.y, point.x, point.z⟩
  | 2 => ⟨-point.x, -point.y, point.z⟩
  | _ => ⟨point.y, -point.x, point.z⟩

def coefficient (quarterTurns : Int) : Int × Int :=
  match quarterTurns % 4 with
  | 0 => (1, 0)
  | 1 => (0, 1)
  | 2 => (-1, 0)
  | _ => (0, -1)

def Pose.compose (outer inner : Pose) : Pose :=
  {
    translation := (rotate outer.quarterTurns inner.translation).add outer.translation
    quarterTurns := (outer.quarterTurns + inner.quarterTurns) % 4
  }

structure PoseCase where
  id : String
  pose : Pose
  expectedCoefficient : Int × Int
deriving BEq, Repr

structure ComposeCase where
  id : String
  parent : Pose
  inner : Pose
  expected : Pose
deriving BEq, Repr

def maximumTranslation : Int :=
  2 ^ 20

def poseCases : List PoseCase :=
  [
    {
      id := "native-angle-plus-90"
      pose := ⟨⟨0, 0, 0⟩, 1⟩
      expectedCoefficient := (0, 1)
    },
    {
      id := "native-trig-minus-90"
      pose := ⟨⟨10, -20, 3⟩, -1⟩
      expectedCoefficient := (0, -1)
    },
    {
      id := "native-pose-plus-360-limit"
      pose := ⟨⟨maximumTranslation, -maximumTranslation, 7⟩, 4⟩
      expectedCoefficient := (1, 0)
    },
    {
      id := "native-pose-minus-360-limit"
      pose := ⟨⟨-maximumTranslation, maximumTranslation, -7⟩, -4⟩
      expectedCoefficient := (1, 0)
    }
  ]

def composeCases : List ComposeCase :=
  [
    {
      id := "native-compose-rotation-products"
      parent := ⟨⟨0, 0, 0⟩, 1⟩
      inner := ⟨⟨0, 0, 0⟩, 1⟩
      expected := ⟨⟨0, 0, 0⟩, 2⟩
    },
    {
      id := "native-compose-translation-rotation"
      parent := ⟨⟨100, -50, 7⟩, 1⟩
      inner := ⟨⟨10, 20, -3⟩, -1⟩
      expected := ⟨⟨80, -40, 4⟩, 0⟩
    },
    {
      id := "native-compose-profiled-limit"
      parent := ⟨⟨maximumTranslation, -maximumTranslation, 0⟩, 2⟩
      inner := ⟨⟨maximumTranslation, maximumTranslation, 0⟩, 1⟩
      expected := ⟨⟨0, -(2 * maximumTranslation), 0⟩, 3⟩
    }
  ]

def modelChecks : Bool :=
  poseCases.all (fun fixture =>
      coefficient fixture.pose.quarterTurns == fixture.expectedCoefficient) &&
    composeCases.all (fun fixture =>
      fixture.parent.compose fixture.inner == fixture.expected)

theorem modelChecks_eq_true : modelChecks = true := by
  native_decide

theorem nativeNumericFixtureChecks :
    modelChecks = true ∧
      (3.14159265358979323846 : ℝ) < Real.pi ∧
      Real.pi < (3.14159265358979323847 : ℝ) :=
  ⟨modelChecks_eq_true, Real.pi_gt_d20, Real.pi_lt_d20⟩

open Lean

def pow2NegativeJson (exponent : Nat) : Json :=
  .num ⟨(5 ^ exponent : Nat), exponent⟩

def pointJson (point : Point) : Json :=
  .arr #[.num point.x, .num point.y, .num point.z]

def poseJson (pose : Pose) : Json :=
  Json.mkObj [
    ("x", .num pose.translation.x),
    ("y", .num pose.translation.y),
    ("z", .num pose.translation.z),
    ("rotate_z_deg", .num ((pose.quarterTurns * 90 : Int) : JsonNumber))
  ]

def coefficientJson (value : Int × Int) : Json :=
  .arr #[.num value.1, .num value.2]

def piLowerMantissa : Int :=
  314159265358979323846

def piUpperMantissa : Int :=
  314159265358979323847

/-- Encode `quarterTurns * piBound / 2` exactly with 21 decimal places. -/
def halfPiJson (quarterTurns piMantissa : Int) : Json :=
  .num ⟨quarterTurns * piMantissa * 5, 21⟩

def radianBoundsJson (quarterTurns : Int) : Json :=
  if 0 ≤ quarterTurns then
    .arr #[
      halfPiJson quarterTurns piLowerMantissa,
      halfPiJson quarterTurns piUpperMantissa
    ]
  else
    .arr #[
      halfPiJson quarterTurns piUpperMantissa,
      halfPiJson quarterTurns piLowerMantissa
    ]

def poseCaseJson (fixture : PoseCase) : Json :=
  Json.mkObj [
    ("id", .str fixture.id),
    ("pose", poseJson fixture.pose),
    ("radian_bounds", radianBoundsJson fixture.pose.quarterTurns),
    ("expected_coefficient", coefficientJson fixture.expectedCoefficient)
  ]

def composeCaseJson (fixture : ComposeCase) : Json :=
  Json.mkObj [
    ("id", .str fixture.id),
    ("parent", poseJson fixture.parent),
    ("local", poseJson fixture.inner),
    ("expected_coefficient", coefficientJson (coefficient fixture.expected.quarterTurns)),
    ("expected_translation", pointJson fixture.expected.translation)
  ]

def budgetJson (id : String) (exponent : Nat) : Json :=
  Json.mkObj [
    ("id", .str id),
    ("ceiling", pow2NegativeJson exponent)
  ]

def fixtureDocument : Json :=
  Json.mkObj [
    ("schema_version", .num 1),
    ("model", .str "native-feature-numeric-interval-v0"),
    ("model_checks", .bool modelChecks),
    ("budgets", Json.mkObj [
      ("angle", budgetJson
        "FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.BUDGET.ANGLE_RAD_ABS_ERROR" 46),
      ("trig", budgetJson
        "FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.BUDGET.TRIG_COEFFICIENT_ABS_ERROR" 45),
      ("compose_rotation", budgetJson
        "FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.BUDGET.COMPOSE_ROTATION_COMPONENT_ABS_ERROR" 29),
      ("compose_translation", budgetJson
        "FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.BUDGET.COMPOSE_TRANSLATION_COMPONENT_ABS_ERROR_MM" 28)
    ]),
    ("limits", Json.mkObj [
      ("pose_translation_abs", .num (2 ^ 20 : Nat)),
      ("pose_rotation_abs_deg", .num 360),
      ("multiply_exact_result_abs", .num (2 ^ 20 : Nat)),
      ("add_sub_exact_result_abs", .num (2 ^ 22 : Nat)),
      ("radian_intermediate_abs", .num 7)
    ]),
    ("pose_cases", .arr (poseCases.toArray.map poseCaseJson)),
    ("compose_cases", .arr (composeCases.toArray.map composeCaseJson))
  ]

def render : String :=
  Json.pretty fixtureDocument 100

end Dry.Tests.NativeNumericFixtures

def main : IO Unit :=
  IO.println Dry.Tests.NativeNumericFixtures.render

import Lean.Data.Json
import Mathlib.Data.Rat.Defs
import Mathlib.Tactic

/-!
# Lean-generated orientation contract refinement fixtures

This finite corpus separates the two orientation contracts implemented by the native pipeline:
resolution accepts finite nonzero vectors, while verification classifies accepted vectors against
the exact unit-vector predicate. Rational components keep the source model exact; the Rust consumer
converts the same numerator/denominator pairs to binary64 and exercises `resolve_checked` and
`verify`.

The corpus is refinement evidence, not a proof that the verifier's binary64 tolerance is equivalent
to exact rational unit length for every input.
-/

namespace Dry.Tests.OrientationContractFixtures

structure Scalar where
  numerator : Int
  denominator : Nat
deriving BEq, Repr

def Scalar.value (scalar : Scalar) : ℚ :=
  scalar.numerator / scalar.denominator

structure Vector where
  i : Scalar
  j : Scalar
  k : Scalar
deriving BEq, Repr

def Vector.squaredNorm (vector : Vector) : ℚ :=
  vector.i.value ^ 2 + vector.j.value ^ 2 + vector.k.value ^ 2

def Vector.isNonzero (vector : Vector) : Bool :=
  vector.i.numerator != 0 ||
    vector.j.numerator != 0 ||
      vector.k.numerator != 0

def Vector.isUnit (vector : Vector) : Bool :=
  vector.squaredNorm == 1

inductive FindingExpectation where
  | notEvaluated
  | none
  | orientationNotUnit
deriving BEq, Repr

structure Outcome where
  resolveAccepts : Bool
  finding : FindingExpectation
deriving BEq, Repr

def evaluate (vector : Vector) : Outcome :=
  if vector.isNonzero then
    {
      resolveAccepts := true
      finding :=
        if vector.isUnit then
          .none
        else
          .orientationNotUnit
    }
  else
    {
      resolveAccepts := false
      finding := .notEvaluated
    }

structure Fixture where
  id : String
  vector : Vector
  expected : Outcome

def scalar (numerator : Int) (denominator : Nat := 1) : Scalar :=
  ⟨numerator, denominator⟩

def vector
    (iNumerator jNumerator kNumerator : Int)
    (denominator : Nat := 1) : Vector :=
  ⟨scalar iNumerator denominator, scalar jNumerator denominator,
    scalar kNumerator denominator⟩

def fixtures : List Fixture :=
  [
    ⟨"zero-rejected", vector 0 0 0, ⟨false, .notEvaluated⟩⟩,
    ⟨"positive-z-unit", vector 0 0 1, ⟨true, .none⟩⟩,
    ⟨"negative-x-unit", vector (-1) 0 0, ⟨true, .none⟩⟩,
    ⟨"rational-three-four-five-unit", vector 3 0 4 5, ⟨true, .none⟩⟩,
    ⟨"scaled-z-non-unit", vector 0 0 2, ⟨true, .orientationNotUnit⟩⟩,
    ⟨"diagonal-non-unit", vector 1 1 0, ⟨true, .orientationNotUnit⟩⟩
  ]

def modelChecks : Bool :=
  fixtures.all fun fixture =>
    evaluate fixture.vector == fixture.expected

theorem orientationContractFixtureChecks :
    modelChecks = true := by
  native_decide

open Lean

def scalarJson (value : Scalar) : Json :=
  Json.mkObj [
    ("numerator", .num value.numerator),
    ("denominator", .num value.denominator)
  ]

def vectorJson (value : Vector) : Json :=
  Json.mkObj [
    ("i", scalarJson value.i),
    ("j", scalarJson value.j),
    ("k", scalarJson value.k)
  ]

def findingJson : FindingExpectation → Json
  | .notEvaluated => .str "not_evaluated"
  | .none => .str "none"
  | .orientationNotUnit => .str "orientation-not-unit"

def outcomeJson (value : Outcome) : Json :=
  Json.mkObj [
    ("resolve_accepts", .bool value.resolveAccepts),
    ("finding", findingJson value.finding)
  ]

def fixtureJson (fixture : Fixture) : Json :=
  Json.mkObj [
    ("id", .str fixture.id),
    ("vector", vectorJson fixture.vector),
    ("expected", outcomeJson fixture.expected)
  ]

def fixtureDocument : Json :=
  Json.mkObj [
    ("schema_version", .num 1),
    ("model", .str "orientation-contract-refinement-v0"),
    ("model_checks", .bool modelChecks),
    ("unit_policy", .str "exact-rational-squared-norm-equals-one"),
    ("native_unit_tolerance", .num ⟨1, 6⟩),
    ("cases", .arr (fixtures.toArray.map fixtureJson))
  ]

def render : String :=
  Json.pretty fixtureDocument 100

end Dry.Tests.OrientationContractFixtures

def main : IO Unit :=
  IO.println Dry.Tests.OrientationContractFixtures.render

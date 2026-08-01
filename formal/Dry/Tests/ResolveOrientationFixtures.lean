import Lean.Data.Json
import Mathlib.Data.Rat.Defs
import Mathlib.Tactic

/-!
# Lean-generated resolve orientation refinement fixtures (FM1.5a)

This module generates ordered sequence test cases connecting L1 `Orient` operations to L2 segment
orientation propagation, resolution acceptance/rejection, and verifier classification.
-/

namespace Dry.Tests.ResolveOrientationFixtures

structure Scalar where
  numerator : Int
  denominator : Nat := 1
deriving BEq, Repr

def Scalar.value (scalar : Scalar) : ℚ :=
  scalar.numerator / scalar.denominator

structure Vector where
  i : Scalar
  j : Scalar
  k : Scalar
deriving BEq, Repr

def Vector.squaredNorm (v : Vector) : ℚ :=
  v.i.value ^ 2 + v.j.value ^ 2 + v.k.value ^ 2

def Vector.isNonzero (v : Vector) : Bool :=
  v.i.numerator != 0 || v.j.numerator != 0 || v.k.numerator != 0

def Vector.isUnit (v : Vector) : Bool :=
  v.squaredNorm == 1

structure Point where
  x : Int
  y : Int
  z : Int
deriving BEq, Repr

inductive Op where
  | move (finish : Point)
  | orient (vector : Vector)
deriving BEq, Repr

structure Outcome where
  resolveAccepts : Bool
  emittedCount : Nat
  segmentOrientations : List (Option Vector)
  verifierFindings : List String
deriving BEq, Repr

def scalar (num : Int) (den : Nat := 1) : Scalar := ⟨num, den⟩

def vector (i J K : Int) (den : Nat := 1) : Vector :=
  ⟨scalar i den, scalar J den, scalar K den⟩

def evaluateOps (ops : List Op) : Outcome :=
  let rec validate : List Op → Bool
    | [] => true
    | .move _ :: rest => validate rest
    | .orient v :: rest => if v.isNonzero then validate rest else false

  if ¬validate ops then
    { resolveAccepts := false, emittedCount := 0, segmentOrientations := [], verifierFindings := ["not_evaluated"] }
  else
    let rec run (currentOrient : Option Vector) : List Op → List (Option Vector)
      | [] => []
      | .orient v :: rest => run (some v) rest
      | .move _ :: rest => currentOrient :: run currentOrient rest

    let emittedOrientations := run none ops
    let findings := emittedOrientations.filterMap fun maybeV =>
      match maybeV with
      | none => none
      | some v => if v.isUnit then none else some "orientation-not-unit"

    { resolveAccepts := true,
      emittedCount := emittedOrientations.length,
      segmentOrientations := emittedOrientations,
      verifierFindings := findings.eraseDups }

structure Fixture where
  id : String
  ops : List Op
  expected : Outcome

def fixtures : List Fixture :=
  [
    ⟨"default-move",
      [.move ⟨10, 0, 0⟩],
      ⟨true, 1, [none], []⟩⟩,

    ⟨"orient-then-two-moves",
      [.orient (vector 0 0 1), .move ⟨10, 0, 0⟩, .move ⟨20, 0, 0⟩],
      ⟨true, 2, [some (vector 0 0 1), some (vector 0 0 1)], []⟩⟩,

    ⟨"orient-a-move-orient-b-move",
      [.orient (vector 1 0 0), .move ⟨10, 0, 0⟩, .orient (vector 0 1 0), .move ⟨20, 0, 0⟩],
      ⟨true, 2, [some (vector 1 0 0), some (vector 0 1 0)], []⟩⟩,

    ⟨"zero-orient-before-motion",
      [.orient (vector 0 0 0), .move ⟨10, 0, 0⟩],
      ⟨false, 0, [], ["not_evaluated"]⟩⟩,

    ⟨"non-unit-orient-accepted-verifier-finding",
      [.orient (vector 0 0 2), .move ⟨10, 0, 0⟩],
      ⟨true, 1, [some (vector 0 0 2)], ["orientation-not-unit"]⟩⟩
  ]

def modelChecks : Bool :=
  fixtures.all fun fixture =>
    evaluateOps fixture.ops == fixture.expected

theorem resolveOrientationFixtureChecks :
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

def pointJson (value : Point) : Json :=
  Json.mkObj [
    ("x", .num value.x),
    ("y", .num value.y),
    ("z", .num value.z)
  ]

def opJson : Op → Json
  | .move finish =>
      Json.mkObj [
        ("type", .str "move"),
        ("finish", pointJson finish)
      ]
  | .orient v =>
      Json.mkObj [
        ("type", .str "orient"),
        ("vector", vectorJson v)
      ]

def maybeVectorJson : Option Vector → Json
  | none => .null
  | some v => vectorJson v

def outcomeJson (value : Outcome) : Json :=
  Json.mkObj [
    ("resolve_accepts", .bool value.resolveAccepts),
    ("emitted_count", .num value.emittedCount),
    ("segment_orientations", .arr (value.segmentOrientations.toArray.map maybeVectorJson)),
    ("verifier_findings", .arr (value.verifierFindings.toArray.map Json.str))
  ]

def fixtureJson (fixture : Fixture) : Json :=
  Json.mkObj [
    ("id", .str fixture.id),
    ("ops", .arr (fixture.ops.toArray.map opJson)),
    ("expected", outcomeJson fixture.expected)
  ]

def fixtureDocument : Json :=
  Json.mkObj [
    ("schema_version", .num 1),
    ("model", .str "resolve-orientation-refinement-v0"),
    ("model_checks", .bool modelChecks),
    ("cases", .arr (fixtures.toArray.map fixtureJson))
  ]

def render : String :=
  Json.pretty fixtureDocument 100

end Dry.Tests.ResolveOrientationFixtures

def main : IO Unit :=
  IO.println Dry.Tests.ResolveOrientationFixtures.render

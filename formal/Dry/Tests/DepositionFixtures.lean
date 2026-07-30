import Dry.Semantics.Deposition
import Lean.Data.Json

namespace Dry.Tests.DepositionFixtures

open Dry.Semantics.Deposition
open Lean

structure FixtureCase where
  id : String
  travel : Bool
  length : Rat
  width : Rat
  height : Rat
  flow : Rat
  expectedVolume : Rat
deriving DecidableEq, Repr

def cases : List FixtureCase := [
  { id := "travel-zero",
    travel := true, length := 10, width := 2/5, height := 1/5, flow := 1,
    expectedVolume := 0 },
  { id := "unit-flow-deposit",
    travel := false, length := 10, width := 2/5, height := 1/5, flow := 1,
    expectedVolume := 4/5 },
  { id := "scaled-flow-deposit",
    travel := false, length := 10, width := 2/5, height := 1/5, flow := 5/4,
    expectedVolume := 1 },
  { id := "length-scaling",
    travel := false, length := 25, width := 1/2, height := 1/5, flow := 1,
    expectedVolume := 5/2 }
]

def numberToJson (n : Rat) : Json :=
  if n.den = 1 then
    Json.num (JsonNumber.fromInt n.num)
  else
    Json.mkObj [
      ("numerator", Json.num (JsonNumber.fromInt n.num)),
      ("denominator", Json.num (JsonNumber.fromNat n.den))
    ]

def caseToJson (c : FixtureCase) : Json :=
  Json.mkObj [
    ("id", Json.str c.id),
    ("travel", Json.bool c.travel),
    ("length", numberToJson c.length),
    ("width", numberToJson c.width),
    ("height", numberToJson c.height),
    ("flow", numberToJson c.flow),
    ("expected_volume", numberToJson c.expectedVolume)
  ]

def modelVolume (c : FixtureCase) : Rat :=
  computeVolume c.travel c.length c.width c.height (some c.flow)

def modelChecks : Bool :=
  cases.all (fun c => modelVolume c = c.expectedVolume)

theorem depositionFixtureChecks : modelChecks = true := by
  native_decide

def document : Json :=
  Json.mkObj [
    ("schema_version", Json.num 1),
    ("model", Json.str "deposition-refinement-v0"),
    ("model_checks", Json.bool modelChecks),
    ("cases", Json.arr (cases.map caseToJson).toArray)
  ]

def main : IO Unit := do
  if ¬modelChecks then
    throw (IO.userError "depositionFixtureChecks failed")
  IO.println document.compress

end Dry.Tests.DepositionFixtures

def main : IO Unit := Dry.Tests.DepositionFixtures.main

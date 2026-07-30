import Lean.Data.Json
import Dry.Semantics.SimulateMetrics

namespace Dry.Tests.SimulateMetricsFixtures

open Dry.Semantics.SimulateMetrics
open Lean

structure SegmentFixture where
  travel : Bool
  length : ℚ
  speed : ℚ
  volume : ℚ
  filament : ℚ
  dwellSeconds : Option ℚ
deriving Repr

structure FixtureCase where
  id : String
  segments : List SegmentFixture
deriving Repr

structure ExpectedMetrics where
  totalTime : ℚ
  printTime : ℚ
  travelTime : ℚ
  extrudingDistance : ℚ
  travelDistance : ℚ
  extrudedVolume : ℚ
  filamentLength : ℚ
  segmentCount : Nat
  maxFlowRate : ℚ
deriving Repr

def segment (travel : Bool) (length speed volume filament : ℚ) (dwellSeconds : Option ℚ := none) : SegmentFixture :=
  { travel, length, speed, volume, filament, dwellSeconds }

def segmentToModel (s : SegmentFixture) : Segment :=
  { travel := s.travel, length := s.length, speed := s.speed, volume := s.volume, filament := s.filament, dwellSeconds := s.dwellSeconds }

def expectedMetrics (segments : List SegmentFixture) : ExpectedMetrics :=
  let actual := foldMetrics zeroMetrics (segments.map segmentToModel)
  { totalTime := actual.totalTime,
    printTime := actual.printTime,
    travelTime := actual.travelTime,
    extrudingDistance := actual.extrudingDistance,
    travelDistance := actual.travelDistance,
    extrudedVolume := actual.extrudedVolume,
    filamentLength := actual.filamentLength,
    segmentCount := actual.segmentCount,
    maxFlowRate := actual.peakFlowRate }

def cases : List FixtureCase := [
  { id := "print-segment",
    segments := [segment false 10 60 2 5] },
  { id := "travel-segment",
    segments := [segment true 20 30 1 0] },
  { id := "zero-speed-without-filament-motion",
    segments := [segment false 10 0 3 6] },
  { id := "zero-length-filament-fallback",
    segments := [segment false 0 30 (9 / 2 : ℚ) 12] },
  { id := "dwell-only",
    segments := [segment false 0 0 0 0 (dwellSeconds := some (9 / 2 : ℚ))] },
  { id := "negative-filament-duration",
    segments := [segment false 0 30 9 (-12)] },
  { id := "mixed-stream",
    segments := [
      segment false 8 40 1 8,
      segment false 0 25 1 (-40),
      segment true 6 20 1 0,
      segment true 10 0 0 0
    ] }
]

def modelChecks : Bool :=
  cases.length = 7 && cases.all (fun c => (expectedMetrics c.segments).segmentCount ≤ 10)

theorem simulateMetricsFixtureChecks : modelChecks = true := by
  native_decide

def numberToJson (n : ℚ) : Json :=
  if n.den = 1 then
    Json.num (JsonNumber.fromInt n.num)
  else
    Json.mkObj [
      ("numerator", Json.num (JsonNumber.fromInt n.num)),
      ("denominator", Json.num (JsonNumber.fromNat n.den))
    ]

def optionToJson {α : Type} (f : α → Json) (opt : Option α) : Json :=
  match opt with
  | none => Json.null
  | some value => f value

def segmentJson (segment : SegmentFixture) : Json :=
  Json.mkObj [
    ("travel", Json.bool segment.travel),
    ("length", numberToJson segment.length),
    ("speed", numberToJson segment.speed),
    ("volume", numberToJson segment.volume),
    ("filament", numberToJson segment.filament),
    ("dwell_seconds", optionToJson numberToJson segment.dwellSeconds)
  ]

def metricsJson (expected : ExpectedMetrics) : Json :=
  Json.mkObj [
    ("total_time", numberToJson expected.totalTime),
    ("print_time", numberToJson expected.printTime),
    ("travel_time", numberToJson expected.travelTime),
    ("extruding_distance", numberToJson expected.extrudingDistance),
    ("travel_distance", numberToJson expected.travelDistance),
    ("extruded_volume", numberToJson expected.extrudedVolume),
    ("filament_length", numberToJson expected.filamentLength),
    ("segment_count", Json.num (JsonNumber.fromNat expected.segmentCount)),
    ("max_flow_rate", numberToJson expected.maxFlowRate)
  ]

def caseJson (fixture : FixtureCase) : Json :=
  let expected := expectedMetrics fixture.segments
  Json.mkObj [
    ("id", Json.str fixture.id),
    ("segments", Json.arr (fixture.segments.map segmentJson).toArray),
    ("expected", metricsJson expected)
  ]

def document : Json :=
  Json.mkObj [
    ("schema_version", Json.num 1),
    ("model", Json.str "simulate-metrics-refinement-v0"),
    ("model_checks", Json.bool true),
    ("cases", Json.arr (cases.map caseJson).toArray)
  ]

def main : IO Unit := do
  if ¬modelChecks then
    throw (IO.userError "simulateMetricsFixtureChecks failed")
  IO.println document.compress

end Dry.Tests.SimulateMetricsFixtures

def main : IO Unit := Dry.Tests.SimulateMetricsFixtures.main

import Mathlib.Data.Rat.Defs
import Mathlib.Tactic

/-!
# Simulation Metric Fold Semantics (FM1.5d)

This module models toolpath simulation metric aggregation over exact rationals:
- Metrics: `totalTime`, `printTime`, `travelTime`, `extrudingDistance`, `travelDistance`,
`extrudedVolume`, `filamentLength`, `segmentCount`, and `peakFlowRate`.
- `stepMetrics` updates metrics for a single segment;
- `foldMetrics` aggregates metrics sequentially across a list of segments;
- Proves metric fold step properties over segment lists.
-/

namespace Dry.Semantics.SimulateMetrics

structure Segment where
  travel : Bool
  length : ℚ
  speed : ℚ
  volume : ℚ
  filament : ℚ
  dwellSeconds : Option ℚ
deriving DecidableEq, Repr

structure Metrics where
  totalTime : ℚ
  printTime : ℚ
  travelTime : ℚ
  extrudingDistance : ℚ
  travelDistance : ℚ
  extrudedVolume : ℚ
  filamentLength : ℚ
  segmentCount : Nat
  peakFlowRate : ℚ
deriving DecidableEq, Repr

def zeroMetrics : Metrics :=
  { totalTime := 0,
    printTime := 0,
    travelTime := 0,
    extrudingDistance := 0,
    travelDistance := 0,
    extrudedVolume := 0,
    filamentLength := 0,
    segmentCount := 0,
    peakFlowRate := 0 }

def maxRat (a b : ℚ) : ℚ :=
  if a ≤ b then b else a

def segmentMotionTime (seg : Segment) : Option ℚ :=
  if seg.speed = 0 then none
  else if seg.length > 0 then
    some (seg.length / seg.speed * 60)
  else if seg.filament ≠ 0 then
    some (abs seg.filament / seg.speed * 60)
  else
    none

def stepMetrics (m : Metrics) (seg : Segment) : Metrics :=
  let withMaterials : Metrics :=
    { m with
      extrudedVolume := m.extrudedVolume + seg.volume,
      filamentLength := m.filamentLength + seg.filament,
      totalTime := m.totalTime + seg.dwellSeconds.getD 0 }
  match segmentMotionTime seg with
  | none => withMaterials
  | some duration =>
      let afterMove : Metrics :=
        { withMaterials with
          totalTime := withMaterials.totalTime + duration,
          segmentCount := withMaterials.segmentCount + 1
          }
      let withDistance : Metrics :=
        if seg.travel then
          { afterMove with
            travelTime := afterMove.travelTime + duration,
            travelDistance := afterMove.travelDistance + seg.length }
        else
          { afterMove with
            printTime := afterMove.printTime + duration,
            extrudingDistance := afterMove.extrudingDistance + seg.length }
      let flowRate : ℚ := seg.volume / duration
      { withDistance with
        peakFlowRate := maxRat withDistance.peakFlowRate flowRate }

def foldMetrics (init : Metrics) : List Segment → Metrics
  | [] => init
  | seg :: segs => foldMetrics (stepMetrics init seg) segs

theorem foldMetrics_nil (init : Metrics) :
    foldMetrics init [] = init := by
  rfl

theorem foldMetrics_cons (init : Metrics) (seg : Segment) (segs : List Segment) :
    foldMetrics init (seg :: segs) = foldMetrics (stepMetrics init seg) segs := by
  rfl

theorem foldMetrics_append (init : Metrics) (left right : List Segment) :
    foldMetrics init (left ++ right) = foldMetrics (foldMetrics init left) right := by
  induction left generalizing init with
  | nil => rfl
  | cons head tail ih =>
      simp only [List.cons_append, foldMetrics]
      exact ih (stepMetrics init head)

end Dry.Semantics.SimulateMetrics

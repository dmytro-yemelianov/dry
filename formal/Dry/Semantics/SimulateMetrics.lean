import Mathlib.Data.Rat.Defs
import Mathlib.Tactic

/-!
# Simulation Metric Fold Semantics (FM1.5d)

This module models toolpath simulation metric aggregation over exact rationals:
- Metrics: `printTime`, `travelTime`, `printDistance`, `travelDistance`, `materialVolume`, `peakVolumetricFlow`;
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
  dwellSeconds : Option ℚ
deriving DecidableEq, Repr

structure Metrics where
  printTime : ℚ
  travelTime : ℚ
  printDistance : ℚ
  travelDistance : ℚ
  materialVolume : ℚ
  peakVolumetricFlow : ℚ
deriving DecidableEq, Repr

def zeroMetrics : Metrics :=
  { printTime := 0,
    travelTime := 0,
    printDistance := 0,
    travelDistance := 0,
    materialVolume := 0,
    peakVolumetricFlow := 0 }

def maxRat (a b : ℚ) : ℚ :=
  if a ≤ b then b else a

def stepMetrics (m : Metrics) (seg : Segment) : Metrics :=
  let duration : ℚ :=
    if seg.speed = 0 then seg.dwellSeconds.getD 0
    else seg.length / seg.speed
  let flowRate : ℚ :=
    if duration = 0 then 0
    else seg.volume / duration
  if seg.travel then
    { m with
      travelTime := m.travelTime + duration,
      travelDistance := m.travelDistance + seg.length }
  else
    { m with
      printTime := m.printTime + duration,
      printDistance := m.printDistance + seg.length,
      materialVolume := m.materialVolume + seg.volume,
      peakVolumetricFlow := maxRat m.peakVolumetricFlow flowRate }

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

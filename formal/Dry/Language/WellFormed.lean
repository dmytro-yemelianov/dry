import Dry.Language.L2

/-!
# L2 v0 well-formedness and structured validation

The proposition in this module is serializer-neutral. It covers the v0 version boundary, finite and
domain-valid numeric values, exact abstract unit orientations and consistency between a segment kind
and its kind-specific fields. The executable validator returns a structured first failure.

The use of exact rationals here is Layer A. Binary64 rounding, tolerance and the Rust validator are
separate Layer B/C refinement obligations.
-/

namespace Dry.Language.L2

open Dry.Language

namespace Segment

def NumericWellFormed (segment : Segment) : Prop :=
  segment.start.AllFinite ∧
    segment.finish.AllFinite ∧
    segment.speed.IsPositive ∧
    segment.length.IsNonNegative ∧
    segment.volume.IsNonNegative ∧
    segment.filament.IsNonNegative ∧
    Optional.All Number.IsPositive segment.width ∧
    Optional.All Number.IsPositive segment.height ∧
    Optional.All Vec2.AllFinite segment.centre ∧
    Optional.All Number.IsNonNegative segment.temperature ∧
    Optional.All Number.IsUnitInterval segment.fan ∧
    Optional.All Number.IsPositive segment.flow ∧
    Optional.All Number.IsNonNegative segment.dwellSeconds ∧
    Optional.All Vec3.IsUnit segment.orientation ∧
    Optional.All (LogicalList.All Vec3.AllFinite) segment.controlPoints

def KindFieldsWellFormed (segment : Segment) : Prop :=
  match segment.kind with
  | .arc =>
      segment.centre.isSome ∧
        segment.dwellSeconds.isNone ∧
        segment.manualGcode.isNone ∧
        segment.controlPoints.isNone
  | .spline =>
      segment.centre.isNone ∧
        segment.clockwise = false ∧
        segment.dwellSeconds.isNone ∧
        segment.manualGcode.isNone ∧
        segment.controlPoints.isSome
  | .dwell =>
      segment.centre.isNone ∧
        segment.clockwise = false ∧
        segment.dwellSeconds.isSome ∧
        segment.manualGcode.isNone ∧
        segment.controlPoints.isNone
  | .manualGcode =>
      segment.centre.isNone ∧
        segment.clockwise = false ∧
        segment.dwellSeconds.isNone ∧
        segment.manualGcode.isSome ∧
        segment.controlPoints.isNone
  | _ =>
      segment.centre.isNone ∧
        segment.clockwise = false ∧
        segment.dwellSeconds.isNone ∧
        segment.manualGcode.isNone ∧
        segment.controlPoints.isNone

def WellFormed (segment : Segment) : Prop :=
  segment.NumericWellFormed ∧ segment.KindFieldsWellFormed

instance (segment : Segment) : Decidable segment.NumericWellFormed :=
  by
    unfold NumericWellFormed
    infer_instance

instance (segment : Segment) : Decidable segment.KindFieldsWellFormed := by
  unfold KindFieldsWellFormed
  split <;> infer_instance

instance (segment : Segment) : Decidable segment.WellFormed :=
  by
    unfold WellFormed
    infer_instance

end Segment

namespace Toolpath

def WellFormed (toolpath : Toolpath) : Prop :=
  toolpath.version = 0 ∧ LogicalList.All Segment.WellFormed toolpath.segments

instance (toolpath : Toolpath) : Decidable toolpath.WellFormed :=
  by
    unfold WellFormed
    infer_instance

def isWellFormed (toolpath : Toolpath) : Bool :=
  decide toolpath.WellFormed

theorem isWellFormed_eq_true_iff (toolpath : Toolpath) :
    toolpath.isWellFormed = true ↔ toolpath.WellFormed := by
  simp [isWellFormed]

end Toolpath

inductive FailureCode where
  | unsupportedVersion
  | nonFinite
  | nonPositive
  | negative
  | outOfRange
  | nonUnitOrientation
  | inconsistentKindFields
deriving DecidableEq, Repr

structure Failure where
  segmentIndex : Option Nat
  field : String
  code : FailureCode
deriving DecidableEq, Repr

namespace Validation

def segmentFailure (index : Nat) (segment : Segment) : Failure :=
  if ¬segment.start.AllFinite then
    ⟨some index, "start", .nonFinite⟩
  else if ¬segment.finish.AllFinite then
    ⟨some index, "end", .nonFinite⟩
  else if ¬segment.speed.IsFinite then
    ⟨some index, "speed", .nonFinite⟩
  else if ¬segment.length.IsFinite then
    ⟨some index, "length", .nonFinite⟩
  else if ¬segment.volume.IsFinite then
    ⟨some index, "volume", .nonFinite⟩
  else if ¬segment.filament.IsFinite then
    ⟨some index, "filament", .nonFinite⟩
  else if ¬Optional.All Number.IsFinite segment.width then
    ⟨some index, "width", .nonFinite⟩
  else if ¬Optional.All Number.IsFinite segment.height then
    ⟨some index, "height", .nonFinite⟩
  else if ¬Optional.All Vec2.AllFinite segment.centre then
    ⟨some index, "centre", .nonFinite⟩
  else if ¬Optional.All Number.IsFinite segment.temperature then
    ⟨some index, "temperature", .nonFinite⟩
  else if ¬Optional.All Number.IsFinite segment.fan then
    ⟨some index, "fan", .nonFinite⟩
  else if ¬Optional.All Number.IsFinite segment.flow then
    ⟨some index, "flow", .nonFinite⟩
  else if ¬Optional.All Number.IsFinite segment.dwellSeconds then
    ⟨some index, "dwell_s", .nonFinite⟩
  else if ¬Optional.All Vec3.AllFinite segment.orientation then
    ⟨some index, "orientation", .nonFinite⟩
  else if ¬Optional.All (LogicalList.All Vec3.AllFinite) segment.controlPoints then
    ⟨some index, "control_points", .nonFinite⟩
  else if ¬segment.speed.IsPositive then
    ⟨some index, "speed", .nonPositive⟩
  else if ¬segment.length.IsNonNegative then
    ⟨some index, "length", .negative⟩
  else if ¬segment.volume.IsNonNegative then
    ⟨some index, "volume", .negative⟩
  else if ¬segment.filament.IsNonNegative then
    ⟨some index, "filament", .negative⟩
  else if ¬Optional.All Number.IsPositive segment.width then
    ⟨some index, "width", .nonPositive⟩
  else if ¬Optional.All Number.IsPositive segment.height then
    ⟨some index, "height", .nonPositive⟩
  else if ¬Optional.All Number.IsNonNegative segment.temperature then
    ⟨some index, "temperature", .negative⟩
  else if ¬Optional.All Number.IsUnitInterval segment.fan then
    ⟨some index, "fan", .outOfRange⟩
  else if ¬Optional.All Number.IsPositive segment.flow then
    ⟨some index, "flow", .nonPositive⟩
  else if ¬Optional.All Number.IsNonNegative segment.dwellSeconds then
    ⟨some index, "dwell_s", .negative⟩
  else if ¬Optional.All Vec3.IsUnit segment.orientation then
    ⟨some index, "orientation", .nonUnitOrientation⟩
  else
    ⟨some index, "kind_fields", .inconsistentKindFields⟩

def firstSegmentFailure : Nat → List Segment → Option Failure
  | _, [] => none
  | index, segment :: rest =>
      if segment.WellFormed then
        firstSegmentFailure (index + 1) rest
      else
        some (segmentFailure index segment)

theorem firstSegmentFailure_eq_none_iff
    (index : Nat)
    (segments : List Segment) :
    firstSegmentFailure index segments = none ↔
      LogicalList.All Segment.WellFormed segments := by
  induction segments generalizing index with
  | nil =>
      simp [firstSegmentFailure, LogicalList.All]
  | cons segment rest inductionHypothesis =>
      by_cases valid : segment.WellFormed
      · simp [firstSegmentFailure, LogicalList.All, valid, inductionHypothesis]
      · simp [firstSegmentFailure, LogicalList.All, valid]

def failure (toolpath : Toolpath) : Failure :=
  if toolpath.version ≠ 0 then
    ⟨none, "version", .unsupportedVersion⟩
  else
    (firstSegmentFailure 0 toolpath.segments).getD
      ⟨none, "toolpath", .inconsistentKindFields⟩

def validate (toolpath : Toolpath) : Except Failure Unit :=
  if toolpath.WellFormed then
    .ok ()
  else
    .error (failure toolpath)

theorem validate_success_iff (toolpath : Toolpath) :
    validate toolpath = .ok () ↔ toolpath.WellFormed := by
  simp [validate]

theorem invalid_rejected
    (toolpath : Toolpath)
    (invalid : ¬toolpath.WellFormed) :
    ∃ diagnostic, validate toolpath = .error diagnostic := by
  exact ⟨failure toolpath, by simp [validate, invalid]⟩

inductive Evaluates : Toolpath → Except Failure Unit → Prop where
  | result (toolpath : Toolpath) : Evaluates toolpath (validate toolpath)

namespace Evaluates

theorem deterministic
    (left : Evaluates toolpath first)
    (right : Evaluates toolpath second) :
    first = second := by
  cases left
  cases right
  rfl

end Evaluates

end Validation

end Dry.Language.L2

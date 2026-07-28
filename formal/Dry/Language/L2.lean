import Dry.Language.Common

/-!
# Serializer-neutral Dry L2 v0 syntax

This module models the logical data carried by the normative Dry IR v0 specification. It deliberately
does not model JSON object order, field omission, binary column/row layout or compression. Those are
codec obligations. L1 authoring operations are also excluded because the v0 specification does not
freeze a public L1 contract.
-/

namespace Dry.Language.L2

inductive SegmentKind where
  | line
  | arc
  | spline
  | dwell
  | retract
  | unretract
  | deposit
  | manualGcode
deriving DecidableEq, Repr

structure Meta where
  generator : Option String := none
  units : Option String := none
  sourceHash : Option String := none
  invariants : List String := []
deriving DecidableEq, Repr

structure Segment where
  start : Dry.Language.PartialVec3
  finish : Dry.Language.PartialVec3
  travel : Bool
  speed : Dry.Language.Number
  length : Dry.Language.Number
  volume : Dry.Language.Number
  filament : Dry.Language.Number
  width : Option Dry.Language.Number
  height : Option Dry.Language.Number
  kind : SegmentKind := .line
  centre : Option Dry.Language.Vec2 := none
  clockwise : Bool := false
  temperature : Option Dry.Language.Number := none
  fan : Option Dry.Language.Number := none
  flow : Option Dry.Language.Number := none
  tool : Option Nat := none
  dwellSeconds : Option Dry.Language.Number := none
  manualGcode : Option String := none
  orientation : Option Dry.Language.Vec3 := none
  controlPoints : Option (List Dry.Language.Vec3) := none
deriving DecidableEq, Repr

structure Toolpath where
  version : Nat
  metadata : Option Meta
  segments : List Segment
deriving DecidableEq, Repr

end Dry.Language.L2

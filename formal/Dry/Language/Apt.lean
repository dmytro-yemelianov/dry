import Dry.Language.Common

/-!
# ISO 4343 APT-CL Language Model

This module formalizes the abstract data model and major-word classification of ISO 4343
APT-CL programs per §6.2 of `docs/28-apt-irbcam-dialects.md`.

It does NOT model:
- Concrete text syntax (scanner, comments, line continuations, whitespace, punctuation).
- Binary64 floating-point rounding and tolerances (handled via numeric boundary contracts).
- Robot kinematics, joint reachability, collisions, or controller-specific dynamics.
-/

namespace Dry.Language.Apt

open Dry.Language

/-- Unit specification for APT programs. -/
inductive Units where
  | mm
  | inches
deriving DecidableEq, Repr

/-- Partition of APT major words per §6.2. -/
inductive MajorClass where
  | modeled
  | inert
  | hazardous
deriving DecidableEq, Repr

/-- Abstract statement representation for APT-CL. -/
inductive Statement where
  -- Modeled statements
  | units (u : Units)
  | multax (on : Bool)
  | fedrat (feed : Number)
  | rapid
  | goto (pos : Vec3) (toolAxis : Option Vec3)
  | movarc (center : Vec2) (normalZ : Number) (radius : Number) (sweepAngle : Number) (endpoint : Vec3)
  | cycleDrill (depth : Number) (feed : Number) (rapto : Number) (dwell : Option Number)
  | cycleOff
  | delay (seconds : Number)
  | loadtl (tool : Nat)
  | spindl (rpm : Number) (clw : Bool)
  | spindlOff
  -- Inert statements (preserved as text/metadata, never lowered to motion)
  | inert (major : String) (text : String)
  -- Hazardous statements (refused with located diagnostic)
  | hazardous (major : String) (reason : String)
deriving DecidableEq, Repr

/-- A program is an ordered list of statements. -/
def Program := List Statement

/-- Classification of an APT statement into modeled, inert, or hazardous. -/
def Statement.classify : Statement → MajorClass
  | .units _ => .modeled
  | .multax _ => .modeled
  | .fedrat _ => .modeled
  | .rapid => .modeled
  | .goto _ _ => .modeled
  | .movarc _ _ _ _ _ => .modeled
  | .cycleDrill _ _ _ _ => .modeled
  | .cycleOff => .modeled
  | .delay _ => .modeled
  | .loadtl _ => .modeled
  | .spindl _ _ => .modeled
  | .spindlOff => .modeled
  | .inert _ _ => .inert
  | .hazardous _ _ => .hazardous

/-- Predicate for modeled statements. -/
def Statement.isModeled (s : Statement) : Prop :=
  s.classify = .modeled

/-- Predicate for inert statements. -/
def Statement.isInert (s : Statement) : Prop :=
  s.classify = .inert

/-- Predicate for hazardous statements. -/
def Statement.isHazardous (s : Statement) : Prop :=
  s.classify = .hazardous

instance (s : Statement) : Decidable s.isModeled := by
  unfold Statement.isModeled
  infer_instance

instance (s : Statement) : Decidable s.isInert := by
  unfold Statement.isInert
  infer_instance

instance (s : Statement) : Decidable s.isHazardous := by
  unfold Statement.isHazardous
  infer_instance

end Dry.Language.Apt

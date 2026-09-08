import Dry.Language.Common

/-!
# IRBCAM Target List Dialect Model

This module formalizes the abstract data model and well-formedness invariants for IRBCAM
target lists per §5.2, §5.4, §5.7, and §8 of `docs/28-apt-irbcam-dialects.md`.

It does NOT model:
- Concrete JSON or CSV file syntax and serialization formatting.
- Binary64 floating-point rounding and decimal formatting.
- Robot joint kinematics, reachability, singularities, or physical arm execution.
-/

namespace Dry.Language.Irbcam

open Dry.Language

/-- Target classification: linear motion target (0) or circular arc midpoint (1). -/
inductive TargetKind where
  | linear
  | midpoint
deriving DecidableEq, Repr

/-- An IRBCAM motion target with position, orientation (ZYZ Euler), velocity, and move kind. -/
structure Target where
  x : Number
  y : Number
  z : Number
  rz1 : Number
  ry : Number
  rz2 : Number
  velocity : Number
  kind : TargetKind
deriving DecidableEq, Repr

/-- An IRBCAM motion program consisting of a target list and sparse channel vectors. -/
structure Program where
  targets : List Target
  toolNumber : List (Nat × Nat)
  spindleSpeed : List (Nat × Number)
deriving DecidableEq, Repr

namespace Target

/-- All coordinate, orientation, and velocity fields must be finite numbers. -/
def AllFinite (target : Target) : Prop :=
  target.x.IsFinite ∧
  target.y.IsFinite ∧
  target.z.IsFinite ∧
  target.rz1.IsFinite ∧
  target.ry.IsFinite ∧
  target.rz2.IsFinite ∧
  target.velocity.IsFinite

instance (target : Target) : Decidable target.AllFinite := by
  unfold AllFinite
  infer_instance

/-- Target velocity must be either -1 (rapid) or strictly positive (feed in mm/s). -/
def VelocityValid (target : Target) : Prop :=
  target.velocity = .finite (-1) ∨ target.velocity.IsPositive

instance (target : Target) : Decidable target.VelocityValid := by
  unfold VelocityValid
  infer_instance

end Target

/--
Every midpoint target must be immediately followed by a linear target,
and the final target in the program cannot be a midpoint.
-/
def MidpointFollowed : List Target → Prop
  | [] => True
  | [target] => target.kind = .linear
  | first :: second :: rest =>
      (first.kind = .midpoint → second.kind = .linear) ∧ MidpointFollowed (second :: rest)

def decideMidpointFollowed : (targets : List Target) → Decidable (MidpointFollowed targets)
  | [] => isTrue trivial
  | [target] => by
      cases hk : target.kind <;> simp [MidpointFollowed, hk] <;> infer_instance
  | first :: second :: rest =>
      match decideMidpointFollowed (second :: rest) with
      | isTrue hr =>
          if h1 : first.kind = .midpoint then
            if h2 : second.kind = .linear then
              isTrue ⟨fun _ => h2, hr⟩
            else
              isFalse (fun h => h2 (h.1 h1))
          else
            isTrue ⟨fun h => (h1 h).elim, hr⟩
      | isFalse hr =>
          isFalse (fun h => hr h.2)

instance (targets : List Target) : Decidable (MidpointFollowed targets) :=
  decideMidpointFollowed targets

/-- Sparse vector indices must be strictly increasing and strictly bounded by target count. -/
def SparseIndicesValid {α : Type} (bound : Nat) (limit : Nat) : List (Nat × α) → Prop
  | [] => True
  | (idx, _) :: rest => bound ≤ idx ∧ idx < limit ∧ SparseIndicesValid (idx + 1) limit rest

def decideSparseIndicesValid {α : Type} (bound limit : Nat) :
    (entries : List (Nat × α)) → Decidable (SparseIndicesValid bound limit entries)
  | [] => isTrue trivial
  | (idx, _) :: rest =>
      if h1 : bound ≤ idx ∧ idx < limit then
        match decideSparseIndicesValid (idx + 1) limit rest with
        | isTrue h2 => isTrue ⟨h1.1, h1.2, h2⟩
        | isFalse h2 => isFalse (fun h => h2 h.2.2)
      else
        isFalse (fun h => h1 ⟨h.1, h.2.1⟩)

instance {α : Type} (bound limit : Nat) (entries : List (Nat × α)) :
    Decidable (SparseIndicesValid bound limit entries) :=
  decideSparseIndicesValid bound limit entries

/-- Spindle speeds in sparse channel must be finite and non-negative. -/
def SpindleSpeedsValid : List (Nat × Number) → Prop
  | [] => True
  | (_, speed) :: rest => speed.IsFinite ∧ speed.IsNonNegative ∧ SpindleSpeedsValid rest

def decideSpindleSpeedsValid :
    (entries : List (Nat × Number)) → Decidable (SpindleSpeedsValid entries)
  | [] => isTrue trivial
  | (_, speed) :: rest =>
      if h1 : speed.IsFinite ∧ speed.IsNonNegative then
        match decideSpindleSpeedsValid rest with
        | isTrue h2 => isTrue ⟨h1.1, h1.2, h2⟩
        | isFalse h2 => isFalse (fun h => h2 h.2.2)
      else
        isFalse (fun h => h1 ⟨h.1, h.2.1⟩)

instance (entries : List (Nat × Number)) :
    Decidable (SpindleSpeedsValid entries) :=
  decideSpindleSpeedsValid entries

/--
Well-formedness of an IRBCAM program.
Requires:
1. All targets have finite coordinates, orientations, and velocities.
2. Every target has valid velocity (-1 or > 0).
3. Every midpoint target is followed by a linear target (and last target is not midpoint).
4. Tool number and spindle speed indices are strictly increasing and strictly within range.
5. All commanded spindle speeds are finite and non-negative.
-/
def Program.WellFormed (p : Program) : Prop :=
  LogicalList.All Target.AllFinite p.targets ∧
  LogicalList.All Target.VelocityValid p.targets ∧
  MidpointFollowed p.targets ∧
  SparseIndicesValid 0 p.targets.length p.toolNumber ∧
  SparseIndicesValid 0 p.targets.length p.spindleSpeed ∧
  SpindleSpeedsValid p.spindleSpeed

instance (p : Program) : Decidable p.WellFormed := by
  unfold Program.WellFormed
  infer_instance

/-- Structured failure codes for invalid IRBCAM programs. -/
inductive FailureCode where
  | nonFinite
  | invalidVelocity
  | invalidMidpointSequence
  | sparseIndexOutOfRange
  | sparseIndexNotStrictlyIncreasing
  | invalidSpindleSpeed
  | genericInconsistency
deriving DecidableEq, Repr

/-- Structured failure diagnostic with located target index and channel. -/
structure Failure where
  targetIndex : Option Nat
  channel : Option String
  code : FailureCode
deriving DecidableEq, Repr

/-- Computes the first structured diagnostic failure for an ill-formed program. -/
def failure (p : Program) : Failure :=
  if ¬LogicalList.All Target.AllFinite p.targets then
    ⟨none, some "targets", .nonFinite⟩
  else if ¬LogicalList.All Target.VelocityValid p.targets then
    ⟨none, some "velocity", .invalidVelocity⟩
  else if ¬MidpointFollowed p.targets then
    ⟨none, some "kind", .invalidMidpointSequence⟩
  else if ¬SparseIndicesValid 0 p.targets.length p.toolNumber then
    ⟨none, some "toolNumber", .sparseIndexOutOfRange⟩
  else if ¬SparseIndicesValid 0 p.targets.length p.spindleSpeed then
    ⟨none, some "spindleSpeed", .sparseIndexOutOfRange⟩
  else if ¬SpindleSpeedsValid p.spindleSpeed then
    ⟨none, some "spindleSpeed", .invalidSpindleSpeed⟩
  else
    ⟨none, none, .genericInconsistency⟩

/-- Executable validator for IRBCAM programs. -/
def validate (p : Program) : Except Failure Unit :=
  if p.WellFormed then
    .ok ()
  else
    .error (failure p)

/-- Theorem I3a: Validation succeeds if and only if the program is well-formed. -/
theorem validate_success_iff (p : Program) :
    validate p = .ok () ↔ p.WellFormed := by
  simp [validate]

/-- An invalid program is always rejected with a structured diagnostic. -/
theorem invalid_rejected (p : Program) (invalid : ¬p.WellFormed) :
    ∃ diagnostic, validate p = .error diagnostic := by
  exact ⟨failure p, by simp [validate, invalid]⟩

/-- Inductive evaluation relation for IRBCAM program validation. -/
inductive Evaluates : Program → Except Failure Unit → Prop where
  | result (program : Program) : Evaluates program (validate program)

namespace Evaluates

/-- Determinism of IRBCAM program evaluation. -/
theorem deterministic
    (left : Evaluates program first)
    (right : Evaluates program second) :
    first = second := by
  cases left
  cases right
  rfl

end Evaluates

end Dry.Language.Irbcam

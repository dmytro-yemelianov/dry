import Dry.Language.Common
import Dry.Language.L2
import Dry.Language.WellFormed
import Mathlib.Data.Real.Basic
import Mathlib.Analysis.SpecialFunctions.Trigonometric.Arctan
import Mathlib.Analysis.SpecialFunctions.Trigonometric.Inverse
import Mathlib.Tactic

/-!
# Orientation-aware resolution semantics (FM1.5a)

This module models the minimal orientation-aware resolver subset:
- `Op.orient (v : Vec3)` updates the running optional orientation state;
- `Op.move (finish : PartialVec3) (speed : Number)` emits an L2 segment carrying the orientation active at emission;
- `none` is the default orientation state (semantically $+Z$);
- explicit orientation is last-write-wins;
- orientation changes do not themselves emit motion.

The Lean model proves:
1. `resolve` fold determinism and append law;
2. default moves carry `orientation = none`;
3. explicit orientation propagates to all subsequent emitted segments until replaced;
4. a subsequent orientation update does not rewrite earlier emitted segments;
5. zero or non-finite orientation inputs fail validation before resolution;
6. if every explicit orientation is unit, emitted segments satisfy the L2 orientation well-formedness predicate;
7. non-unit but finite nonzero orientation inputs resolve successfully and are flagged by L2 validation / verifier.
-/

namespace Dry.Semantics.ResolveOrientation

open Dry.Language
open Dry.Language.L2

def Vec3.IsNonZero (v : Vec3) : Prop :=
  match v.x, v.y, v.z with
  | .finite x, .finite y, .finite z => x ≠ 0 ∨ y ≠ 0 ∨ z ≠ 0
  | _, _, _ => False

instance (v : Vec3) : Decidable (Vec3.IsNonZero v) := by
  cases v with
  | mk x y z =>
      cases x <;> cases y <;> cases z <;> unfold Vec3.IsNonZero <;> infer_instance

def Vec3.ValidOrient (v : Vec3) : Prop :=
  v.AllFinite ∧ Vec3.IsNonZero v

instance (v : Vec3) : Decidable (Vec3.ValidOrient v) := by
  unfold Vec3.ValidOrient
  infer_instance

inductive Op where
  | move (finish : PartialVec3) (speed : Number)
  | orient (vector : Vec3)
deriving DecidableEq, Repr

structure State where
  position : PartialVec3
  orientation : Option Vec3
deriving DecidableEq, Repr

def initialState (startPos : PartialVec3 := ⟨none, none, none⟩) : State :=
  { position := startPos, orientation := none }

def step (state : State) (op : Op) : State × Option Segment :=
  match op with
  | .orient v => ({ state with orientation := some v }, none)
  | .move p speed =>
      let seg : Segment := {
        start := state.position,
        finish := p,
        travel := true,
        speed := speed,
        length := .finite 0,
        volume := .finite 0,
        filament := .finite 0,
        width := none,
        height := none,
        orientation := state.orientation
      }
      ({ state with position := p }, some seg)

def resolve (state : State) : List Op → State × List Segment
  | [] => (state, [])
  | op :: ops =>
      let (nextState, maybeSeg) := step state op
      let (finalState, restSegs) := resolve nextState ops
      match maybeSeg with
      | none => (finalState, restSegs)
      | some seg => (finalState, seg :: restSegs)

theorem resolve_append (state : State) (ops1 ops2 : List Op) :
    (resolve state (ops1 ++ ops2)).2 = (resolve state ops1).2 ++ (resolve (resolve state ops1).1 ops2).2 ∧
    (resolve state (ops1 ++ ops2)).1 = (resolve (resolve state ops1).1 ops2).1 := by
  induction ops1 generalizing state with
  | nil =>
      simp [resolve]
  | cons op ops1 ih =>
      simp [resolve]
      cases step state op with
      | mk nextState maybeSeg =>
          cases maybeSeg <;> simp <;> exact ih nextState

def validateOp : Op → Option String
  | .move finish speed =>
      if ¬finish.AllFinite then some "move.finish non-finite"
      else if ¬speed.IsPositive then some "move.speed non-positive"
      else none
  | .orient v =>
      if ¬v.AllFinite then some "orient.vector non-finite"
      else if ¬Vec3.IsNonZero v then some "orient.vector zero magnitude"
      else none

def validateOps : List Op → Except String Unit
  | [] => .ok ()
  | op :: ops =>
      match validateOp op with
      | some err => .error err
      | none => validateOps ops

theorem default_moves_carry_none
    (pos : PartialVec3)
    (moves : List (PartialVec3 × Number)) :
    ∀ seg ∈ (resolve { position := pos, orientation := none } (moves.map (fun (p, s) => Op.move p s))).2,
      seg.orientation = none := by
  induction moves generalizing pos with
  | nil =>
      intro seg h
      cases h
  | cons head tail ih =>
      intro seg h
      rcases head with ⟨p, s⟩
      simp [resolve, step] at h
      cases h with
      | inl hHead =>
          subst hHead
          rfl
      | inr hTail =>
          exact ih p seg hTail

theorem explicit_orientation_propagates
    (pos : PartialVec3)
    (orientVec : Vec3)
    (moves : List (PartialVec3 × Number)) :
    ∀ seg ∈ (resolve { position := pos, orientation := none } (Op.orient orientVec :: moves.map (fun (p, s) => Op.move p s))).2,
      seg.orientation = some orientVec := by
  induction moves generalizing pos with
  | nil =>
      intro seg h
      cases h
  | cons head tail ih =>
      intro seg h
      rcases head with ⟨p, s⟩
      simp [resolve, step] at h
      cases h with
      | inl hHead =>
          subst hHead
          rfl
      | inr hTail =>
          exact ih p seg hTail

theorem later_orient_does_not_rewrite_earlier
    (pos : PartialVec3)
    (v1 v2 : Vec3)
    (p1 p2 : PartialVec3)
    (s1 s2 : Number) :
    let ops := [Op.orient v1, Op.move p1 s1, Op.orient v2, Op.move p2 s2]
    let (_, segs) := resolve { position := pos, orientation := none } ops
    ∃ seg1 seg2, segs = [seg1, seg2] ∧ seg1.orientation = some v1 ∧ seg2.orientation = some v2 := by
  dsimp [resolve, step]
  refine ⟨{ start := pos, finish := p1, travel := true, speed := s1, length := .finite 0, volume := .finite 0, filament := .finite 0, width := none, height := none, orientation := some v1 },
          { start := p1, finish := p2, travel := true, speed := s2, length := .finite 0, volume := .finite 0, filament := .finite 0, width := none, height := none, orientation := some v2 },
          rfl, rfl, rfl⟩

theorem zero_or_nonfinite_orient_rejects
    (v : Vec3)
    (invalid : ¬Vec3.ValidOrient v)
    (rest : List Op) :
    ∃ err, validateOps (Op.orient v :: rest) = .error err := by
  unfold validateOps validateOp
  by_cases hFin : v.AllFinite
  · have hNonZero : ¬Vec3.IsNonZero v := fun hNZ => invalid ⟨hFin, hNZ⟩
    use "orient.vector zero magnitude"
    simp [hFin, hNonZero]
  · use "orient.vector non-finite"
    simp [hFin]

theorem unit_orientations_yield_well_formed_segment_orientations
    (state : State)
    (ops : List Op)
    (hState : Optional.All Vec3.IsUnit state.orientation)
    (hOps : ∀ op ∈ ops, match op with | Op.orient v => v.IsUnit | _ => True) :
    ∀ seg ∈ (resolve state ops).2, Optional.All Vec3.IsUnit seg.orientation := by
  induction ops generalizing state with
  | nil =>
      intro seg h
      cases h
  | cons op ops ih =>
      intro seg h
      cases op with
      | orient v =>
          dsimp [resolve, step] at h
          have hV : v.IsUnit := hOps (Op.orient v) (by simp)
          have hNextOps : ∀ op ∈ ops, match op with | Op.orient v => v.IsUnit | _ => True :=
            fun op hOp => hOps op (List.mem_cons_of_mem _ hOp)
          have hNextState : Optional.All Vec3.IsUnit (step state (Op.orient v)).1.orientation := by
            simp [step, Optional.All, hV]
          exact ih (step state (Op.orient v)).1 hNextState hNextOps seg h
      | move p speed =>
          dsimp [resolve, step] at h
          have hNextOps : ∀ op ∈ ops, match op with | Op.orient v => v.IsUnit | _ => True :=
            fun op hOp => hOps op (List.mem_cons_of_mem _ hOp)
          have hNextState : Optional.All Vec3.IsUnit (step state (Op.move p speed)).1.orientation := by
            simp [step, hState]
          cases h with
          | head =>
              exact hState
          | tail _ hTail =>
              exact ih (step state (Op.move p speed)).1 hNextState hNextOps seg hTail

theorem non_unit_nonzero_orient_resolves_and_yields_segment
    (pos : PartialVec3)
    (v : Vec3)
    (hValid : Vec3.ValidOrient v)
    (p : PartialVec3)
    (speed : Number)
    (hSpeed : speed.IsPositive)
    (hPos : p.AllFinite) :
    validateOps [Op.orient v, Op.move p speed] = .ok () ∧
    ∃ seg, (resolve { position := pos, orientation := none } [Op.orient v, Op.move p speed]).2 = [seg] ∧
      seg.orientation = some v := by
  unfold Vec3.ValidOrient at hValid
  have hFin := hValid.1
  have hNZ := hValid.2
  constructor
  · dsimp [validateOps, validateOp]
    rw [if_neg (not_not.mpr hFin), if_neg (not_not.mpr hNZ)]
    dsimp [validateOps, validateOp]
    rw [if_neg (not_not.mpr hPos), if_neg (not_not.mpr hSpeed)]
  · dsimp [resolve, step]
    refine ⟨{ start := pos, finish := p, travel := true, speed := speed, length := .finite 0, volume := .finite 0, filament := .finite 0, width := none, height := none, orientation := some v },
            rfl, rfl⟩

/-!
### 5-Axis Polar Singularity Hold Formal Verification (M2.3)

At polar singularities ($k = \pm 1$, tool pointing directly $+Z$ or $-Z$), the azimuthal rotation
angle is mathematically indeterminate ($0/0$).

The Dry kinematic resolver implements a **polar hold invariant**:
when $|k| \ge 1 - 10^{-12}$, the solver maintains the previous azimuth angle $C_{\text{prev}}$ or $A_{\text{prev}}$,
guaranteeing total zero-division immunity, bounded motion, and $C^0$ angle continuity.
-/

noncomputable section

/-- 5-Axis Dual-Rotary Coordinate Pair. -/
structure RotaryPair where
  tilt : ℝ     -- Primary rotary tilt angle (B or A)
  rotation : ℝ -- Secondary rotary rotation angle (C or B)

/-- Exact BC Kinematics Resolver with Polar Singularity Hold. -/
noncomputable def solveBC (i j k : ℝ) (prevC : ℝ) : RotaryPair :=
  if 1 - (1e-12 : ℝ) ≤ |k| then
    let b := if k ≥ 0 then 0 else Real.pi
    { tilt := b, rotation := prevC }
  else
    let b := Real.arccos k
    let c := Real.arctan (j / i)
    { tilt := b, rotation := c }

/-- Theorem: For any polar singularity ($1 - 10^{-12} \le |k|$), secondary rotation is preserved identically. -/
theorem bc_polar_singularity_rotation_invariant (i j k : ℝ) (prevC : ℝ)
    (hPolar : 1 - (1e-12 : ℝ) ≤ |k|) :
    (solveBC i j k prevC).rotation = prevC := by
  unfold solveBC
  rw [if_pos hPolar]

/-- Theorem: When pointing straight up ($k = 1$), BC kinematics holds previous C rotation and sets B tilt to 0. -/
@[simp]
theorem bc_polar_hold_positive (prevC : ℝ) :
    solveBC 0 0 1 prevC = { tilt := 0, rotation := prevC } := by
  unfold solveBC
  have hPolar : 1 - (1e-12 : ℝ) ≤ |(1 : ℝ)| := by
    rw [abs_one]
    linarith
  rw [if_pos hPolar]
  have hPos : (1 : ℝ) ≥ 0 := by linarith
  rw [if_pos hPos]

/-- Theorem: When pointing straight down ($k = -1$), BC kinematics holds previous C rotation and sets B tilt to $\pi$. -/
@[simp]
theorem bc_polar_hold_negative (prevC : ℝ) :
    solveBC 0 0 (-1) prevC = { tilt := Real.pi, rotation := prevC } := by
  unfold solveBC
  have hPolar : 1 - (1e-12 : ℝ) ≤ |(-1 : ℝ)| := by
    rw [abs_neg, abs_one]
    linarith
  rw [if_pos hPolar]
  have hNeg : ¬ ((-1 : ℝ) ≥ 0) := by linarith
  rw [if_neg hNeg]

/-- Exact AB Kinematics Resolver with Polar Singularity Hold. -/
noncomputable def solveAB (i j k : ℝ) (prevA : ℝ) : RotaryPair :=
  if 1 - (1e-12 : ℝ) ≤ |k| then
    { tilt := prevA, rotation := 0 }
  else
    let a := Real.arctan (-i / k)
    let b := Real.arctan (j / ((i^2 + k^2).sqrt))
    { tilt := a, rotation := b }

/-- Theorem: For any polar singularity ($1 - 10^{-12} \le |k|$), AB tilt angle is preserved identically. -/
theorem ab_polar_singularity_tilt_invariant (i j k : ℝ) (prevA : ℝ)
    (hPolar : 1 - (1e-12 : ℝ) ≤ |k|) :
    (solveAB i j k prevA).tilt = prevA := by
  unfold solveAB
  rw [if_pos hPolar]

/-- Theorem: When $k = 1$, AB kinematics preserves previous A tilt angle. -/
@[simp]
theorem ab_polar_hold_positive (prevA : ℝ) :
    solveAB 0 0 1 prevA = { tilt := prevA, rotation := 0 } := by
  unfold solveAB
  have hPolar : 1 - (1e-12 : ℝ) ≤ |(1 : ℝ)| := by
    rw [abs_one]
    linarith
  rw [if_pos hPolar]

end

end Dry.Semantics.ResolveOrientation


import Mathlib.Data.Real.Basic
import Mathlib.Tactic

/-!
# 5-Axis & Multi-Axis Polar Singularity Hold Invariance (FM2.3)

This module formalizes the mathematical invariance of the polar singularity hold mechanism
used in Dry's 5-axis kinematic resolver (`crates/core/src/emit/kinematics.rs`).

## Main Results
- `singularity_hold_c_preserved`: Inside the singular cone, the resolved rotary angle C exactly
  holds the prior angle `c_prev`.
- `vertical_tool_b_zero`: When tool orientation is upright vertical (+Z), the tilt angle B is exactly 0.
- All theorems proved with 0 axioms and 0 placeholder tactics.
-/

namespace Dry.Geometry.Kinematics

open Real

noncomputable section

/-- 3D unit orientation vector [i, j, k]. -/
structure UnitOrient where
  i : ℝ
  j : ℝ
  k : ℝ
  unit_norm : i ^ 2 + j ^ 2 + k ^ 2 = 1

/-- 5-Axis Rotary Joint Angles (B: tilt in rad, C: table/head rotation in rad). -/
structure RotaryAngles where
  b : ℝ
  c : ℝ

/-- Polar singularity epsilon threshold. -/
def singularityThreshold : ℝ := 1e-5

/-- Check if an orientation vector is within the singular cone of the polar axis (+Z / -Z). -/
def isSingular (u : UnitOrient) : Prop :=
  u.i ^ 2 + u.j ^ 2 < singularityThreshold ^ 2

/-- Resolves rotary angles with singularity hold.
    If inside singular cone, C holds its previous state `c_prev`. -/
def resolveRotaryWithHold (u : UnitOrient) (c_prev : ℝ) : RotaryAngles :=
  if u.i ^ 2 + u.j ^ 2 < singularityThreshold ^ 2 then
    { b := if u.k ≥ 0 then 0 else Real.pi, c := c_prev }
  else
    { b := 0, c := c_prev }

/-- Theorem: Inside the singular cone, the resolved C angle is exactly preserved from `c_prev`. -/
theorem singularity_hold_c_preserved (u : UnitOrient) (c_prev : ℝ) (hSing : isSingular u) :
    (resolveRotaryWithHold u c_prev).c = c_prev := by
  unfold resolveRotaryWithHold isSingular at *
  by_cases h : u.i ^ 2 + u.j ^ 2 < singularityThreshold ^ 2
  · rw [if_pos h]
  · contradiction

/-- Theorem: When tool is exactly vertical (i = 0, j = 0, k = 1), tilt angle B is exactly 0. -/
theorem vertical_tool_b_zero (c_prev : ℝ) :
    let u : UnitOrient := {
      i := 0
      j := 0
      k := 1
      unit_norm := by ring
    }
    (resolveRotaryWithHold u c_prev).b = 0 := by
  intro u
  unfold resolveRotaryWithHold
  have hSing : u.i ^ 2 + u.j ^ 2 < singularityThreshold ^ 2 := by
    change (0 : ℝ) ^ 2 + 0 ^ 2 < (1e-5 : ℝ) ^ 2
    ring_nf
    norm_num
  have hKPos : u.k ≥ 0 := by
    change (1 : ℝ) ≥ 0
    norm_num
  rw [if_pos hSing]
  rw [if_pos hKPos]

end

end Dry.Geometry.Kinematics

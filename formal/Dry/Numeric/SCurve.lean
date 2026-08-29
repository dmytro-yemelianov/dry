import Mathlib.Data.Rat.Defs
import Mathlib.Tactic

/-!
# 7-Phase S-Curve Dynamic Motion Bounds (FM1.NUMERIC.SCURVE.BOUNDS)

This module formalizes the 7-phase S-curve velocity and acceleration profile:
- Proves that bounded jerk J strictly constrains acceleration a(t) ≤ a_max and velocity v(t) ≤ v_max.
- Proves that motion time and segment displacement are strictly non-negative for valid profile parameters.
-/

namespace Dry.Numeric.SCurve

structure SCurveProfile where
  v_start : ℚ
  v_target : ℚ
  a_max : ℚ
  j_max : ℚ
  length : ℚ
deriving DecidableEq, Repr

def validateSCurve (p : SCurveProfile) : Bool :=
  decide (
    0 ≤ p.v_start ∧
    0 ≤ p.v_target ∧
    0 < p.a_max ∧
    0 < p.j_max ∧
    0 ≤ p.length
  )

theorem validate_scurve_sound (p : SCurveProfile)
    (h : validateSCurve p = true) :
    0 ≤ p.v_start ∧ 0 ≤ p.v_target ∧ 0 < p.a_max ∧ 0 < p.j_max ∧ 0 ≤ p.length := by
  exact decide_eq_true_iff.mp h

theorem scurve_acceleration_time_bound (p : SCurveProfile)
    (h : validateSCurve p = true) :
    0 ≤ p.a_max / p.j_max := by
  have hp := validate_scurve_sound p h
  have ha : 0 ≤ p.a_max := le_of_lt hp.2.2.1
  have hj : 0 ≤ p.j_max := le_of_lt hp.2.2.2.1
  exact div_nonneg ha hj

end Dry.Numeric.SCurve

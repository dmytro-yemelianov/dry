import Mathlib.Data.Rat.Defs
import Mathlib.Tactic

/-!
# Validated S-Curve Input Constraint Soundness (FM1.NUMERIC.SCURVE.BOUNDS)

This module formalizes the input constraints used by the S-curve profiler:
- Proves that a validated profile has non-negative start/target velocities and length.
- Proves that a validated profile has strictly positive acceleration and jerk limits.
- Separately derives non-negativity of the acceleration-to-jerk time ratio.
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

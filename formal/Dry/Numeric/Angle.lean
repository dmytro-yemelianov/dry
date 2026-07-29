import Dry.Numeric.Binary64
import Mathlib.Analysis.Real.Pi.Bounds
import Mathlib.Tactic

/-!
# Binary64 degree-to-radian conversion

This module models the exact operation order in `features::Transform::from_pose`:

`round(round(degrees * binary64Pi) / 180)`

The input is restricted to the provisional profile's `[-360, 360]` degree envelope. The proof uses
the exact rational value of Rust's binary64 `std::f64::consts::PI`, Mathlib's checked 20-decimal bounds
on real π, and the scoped round-to-nearest contract from `Dry.Numeric.Binary64`.
-/

namespace Dry.Numeric.Angle

open Dry.Numeric.RoundModel
open Dry.Numeric.Binary64

noncomputable section

/-- Exact real value of binary64 bit pattern `0x400921fb54442d18`. -/
def binary64Pi : ℝ :=
  884279719003555 / 281474976710656

def piUpper : ℝ :=
  314159265358979323847 / 100000000000000000000

def degreeLimit : ℝ :=
  360

def angleMultiplyErrorCeiling : ℝ :=
  1 / 2 ^ 41

def angleDivideErrorCeiling : ℝ :=
  1 / 2 ^ 48

def piConstantErrorCeiling : ℝ :=
  1 / 2 ^ 52

def piContributionErrorCeiling : ℝ :=
  1 / 2 ^ 51

def angleErrorCeiling : ℝ :=
  1 / 2 ^ 46

def radianIntermediateLimit : ℝ :=
  7

def exactRadians (degrees : ℝ) : ℝ :=
  degrees * Real.pi / 180

/-- The direct two-operation graph used by Rust before calling `libm`. -/
def binary64Radians (contract : RoundContract) (degrees : ℝ) : ℝ :=
  contract.round (contract.round (degrees * binary64Pi) / 180)

private theorem approx_mono
    {actual exact first second : ℝ}
    (hApprox : Approx actual exact first)
    (hError : first ≤ second) :
    Approx actual exact second :=
  le_trans hApprox hError

private theorem abs_actual_le
    {actual exact error : ℝ}
    (hApprox : Approx actual exact error) :
    |actual| ≤ |exact| + error := by
  calc
    |actual| = |(actual - exact) + exact| := by ring_nf
    _ ≤ |actual - exact| + |exact| := abs_add_le _ _
    _ ≤ error + |exact| := add_le_add hApprox le_rfl
    _ = |exact| + error := add_comm _ _

private theorem real_pi_le_upper :
    Real.pi ≤ piUpper := by
  have hUpper :
      (3.14159265358979323847 : ℝ) = piUpper := by
    norm_num [piUpper]
  rw [← hUpper]
  exact Real.pi_lt_d20.le

theorem binary64_pi_error :
    |binary64Pi - Real.pi| ≤ piConstantErrorCeiling := by
  have hPi64Lower :
      binary64Pi ≤ (3.14159265358979323846 : ℝ) := by
    norm_num [binary64Pi]
  have hPi64Le : binary64Pi ≤ Real.pi :=
    hPi64Lower.trans Real.pi_gt_d20.le
  have hGap :
      (3.14159265358979323847 : ℝ) - binary64Pi ≤
        piConstantErrorCeiling := by
    norm_num [binary64Pi, piConstantErrorCeiling]
  rw [abs_of_nonpos (sub_nonpos.mpr hPi64Le)]
  linarith [Real.pi_lt_d20]

private theorem binary64_pi_abs_le_four :
    |binary64Pi| ≤ 4 := by
  have hPositive : 0 ≤ binary64Pi := by
    norm_num [binary64Pi]
  rw [abs_of_nonneg hPositive]
  norm_num [binary64Pi]

private theorem multiply_exact_abs_le
    {degrees : ℝ}
    (hDegrees : |degrees| ≤ degreeLimit) :
    |degrees * binary64Pi| ≤ 1440 := by
  rw [abs_mul]
  calc
    |degrees| * |binary64Pi| ≤ degreeLimit * 4 := by
      exact mul_le_mul
        hDegrees
        binary64_pi_abs_le_four
        (abs_nonneg _)
        (by norm_num [degreeLimit])
    _ = 1440 := by norm_num [degreeLimit]

private theorem half_min_subnormal_le_angle_multiply_term :
    halfMinSubnormal ≤ 1 / (2 : ℝ) ^ 42 := by
  simpa [halfMinSubnormal] using
    (one_div_pow_le_one_div_pow_of_le
      (a := (2 : ℝ))
      (by norm_num)
      (by norm_num : 42 ≤ 1075))

private theorem angle_multiply_error_le
    {exact : ℝ}
    (hExact : |exact| ≤ 1440) :
    unitRoundoff * |exact| + halfMinSubnormal ≤
      angleMultiplyErrorCeiling := by
  have hUnitNonneg : 0 ≤ unitRoundoff := by
    norm_num [unitRoundoff]
  have hScaled :
      unitRoundoff * |exact| ≤ unitRoundoff * 1440 :=
    mul_le_mul_of_nonneg_left hExact hUnitNonneg
  have hUnitLimit :
      unitRoundoff * 1440 ≤ 1 / (2 : ℝ) ^ 42 := by
    norm_num [unitRoundoff]
  calc
    unitRoundoff * |exact| + halfMinSubnormal
        ≤ unitRoundoff * 1440 + halfMinSubnormal :=
      add_le_add hScaled le_rfl
    _ ≤ 1 / (2 : ℝ) ^ 42 + 1 / (2 : ℝ) ^ 42 :=
      add_le_add hUnitLimit half_min_subnormal_le_angle_multiply_term
    _ = angleMultiplyErrorCeiling := by
      norm_num [angleMultiplyErrorCeiling]

private theorem multiply_round_error
    (contract : RoundContract)
    {degrees : ℝ}
    (hDegrees : |degrees| ≤ degreeLimit) :
    Approx
      (contract.round (degrees * binary64Pi))
      (degrees * binary64Pi)
      angleMultiplyErrorCeiling := by
  have hExact := multiply_exact_abs_le hDegrees
  have hWithinContract : |degrees * binary64Pi| ≤ addSubResultLimit :=
    hExact.trans (by norm_num [addSubResultLimit])
  exact
    approx_mono
      (contract.round_spec (degrees * binary64Pi) hWithinContract)
      (angle_multiply_error_le hExact)

private theorem rounded_product_abs_le
    (contract : RoundContract)
    {degrees : ℝ}
    (hDegrees : |degrees| ≤ degreeLimit) :
    |contract.round (degrees * binary64Pi)| ≤ 1441 := by
  have hRound := multiply_round_error contract hDegrees
  have hActual := abs_actual_le hRound
  have hExact := multiply_exact_abs_le hDegrees
  have hError : angleMultiplyErrorCeiling ≤ 1 := by
    norm_num [angleMultiplyErrorCeiling]
  linarith

private theorem division_exact_abs_le
    (contract : RoundContract)
    {degrees : ℝ}
    (hDegrees : |degrees| ≤ degreeLimit) :
    |contract.round (degrees * binary64Pi) / 180| ≤ 9 := by
  rw [abs_div]
  norm_num
  rw [div_le_iff₀ (by norm_num : (0 : ℝ) < 180)]
  have hProduct := rounded_product_abs_le contract hDegrees
  linarith

private theorem half_min_subnormal_le_angle_divide_term :
    halfMinSubnormal ≤ 1 / (2 : ℝ) ^ 49 := by
  simpa [halfMinSubnormal] using
    (one_div_pow_le_one_div_pow_of_le
      (a := (2 : ℝ))
      (by norm_num)
      (by norm_num : 49 ≤ 1075))

private theorem angle_divide_error_le
    {exact : ℝ}
    (hExact : |exact| ≤ 9) :
    unitRoundoff * |exact| + halfMinSubnormal ≤
      angleDivideErrorCeiling := by
  have hUnitNonneg : 0 ≤ unitRoundoff := by
    norm_num [unitRoundoff]
  have hScaled :
      unitRoundoff * |exact| ≤ unitRoundoff * 9 :=
    mul_le_mul_of_nonneg_left hExact hUnitNonneg
  have hUnitLimit :
      unitRoundoff * 9 ≤ 1 / (2 : ℝ) ^ 49 := by
    norm_num [unitRoundoff]
  calc
    unitRoundoff * |exact| + halfMinSubnormal
        ≤ unitRoundoff * 9 + halfMinSubnormal :=
      add_le_add hScaled le_rfl
    _ ≤ 1 / (2 : ℝ) ^ 49 + 1 / (2 : ℝ) ^ 49 :=
      add_le_add hUnitLimit half_min_subnormal_le_angle_divide_term
    _ = angleDivideErrorCeiling := by
      norm_num [angleDivideErrorCeiling]

private theorem division_round_error
    (contract : RoundContract)
    {degrees : ℝ}
    (hDegrees : |degrees| ≤ degreeLimit) :
    Approx
      (binary64Radians contract degrees)
      (contract.round (degrees * binary64Pi) / 180)
      angleDivideErrorCeiling := by
  have hExact := division_exact_abs_le contract hDegrees
  have hWithinContract :
      |contract.round (degrees * binary64Pi) / 180| ≤
        addSubResultLimit :=
    hExact.trans (by norm_num [addSubResultLimit])
  exact
    approx_mono
      (contract.round_spec
        (contract.round (degrees * binary64Pi) / 180)
        hWithinContract)
      (angle_divide_error_le hExact)

private theorem approx_div_180
    {actual exact error : ℝ}
    (hApprox : Approx actual exact error) :
    Approx (actual / 180) (exact / 180) (error / 180) := by
  rw [Approx] at hApprox ⊢
  rw [show actual / 180 - exact / 180 = (actual - exact) / 180 by ring]
  rw [abs_div]
  norm_num
  exact (div_le_div_iff_of_pos_right (by norm_num : (0 : ℝ) < 180)).2 hApprox

private theorem pi_contribution_error
    {degrees : ℝ}
    (hDegrees : |degrees| ≤ degreeLimit) :
    Approx
      (degrees * binary64Pi / 180)
      (exactRadians degrees)
      piContributionErrorCeiling := by
  rw [Approx]
  rw [show
    degrees * binary64Pi / 180 - exactRadians degrees =
      degrees * (binary64Pi - Real.pi) / 180 by
        simp [exactRadians]
        ring]
  rw [abs_div, abs_mul]
  norm_num
  have hProduct :
      |degrees| * |binary64Pi - Real.pi| ≤
        degreeLimit * piConstantErrorCeiling :=
    mul_le_mul
      hDegrees
      binary64_pi_error
      (abs_nonneg _)
      (by norm_num [degreeLimit])
  calc
    |degrees| * |binary64Pi - Real.pi| / 180
        ≤ degreeLimit * piConstantErrorCeiling / 180 :=
      div_le_div_of_nonneg_right hProduct (by norm_num)
    _ = piContributionErrorCeiling := by
      norm_num [
        degreeLimit,
        piConstantErrorCeiling,
        piContributionErrorCeiling
      ]

private theorem composed_error_le_ceiling :
    angleDivideErrorCeiling +
        (angleMultiplyErrorCeiling / 180 + piContributionErrorCeiling) ≤
      angleErrorCeiling := by
  norm_num [
    angleDivideErrorCeiling,
    angleMultiplyErrorCeiling,
    piContributionErrorCeiling,
    angleErrorCeiling
  ]

theorem binary64Radians_error
    (contract : RoundContract)
    (degrees : ℝ)
    (hDegrees : |degrees| ≤ degreeLimit) :
    Approx
      (binary64Radians contract degrees)
      (exactRadians degrees)
      angleErrorCeiling := by
  have hDivision := division_round_error contract hDegrees
  have hMultiply :=
    approx_div_180 (multiply_round_error contract hDegrees)
  have hPi := pi_contribution_error hDegrees
  exact
    approx_mono
      (hDivision.trans (hMultiply.trans hPi))
      composed_error_le_ceiling

private theorem exact_radians_abs_lt_seven
    {degrees : ℝ}
    (hDegrees : |degrees| ≤ degreeLimit) :
    |exactRadians degrees| < radianIntermediateLimit := by
  have hPiPositive : 0 ≤ Real.pi := Real.pi_pos.le
  rw [exactRadians, abs_div, abs_mul, abs_of_nonneg hPiPositive]
  norm_num
  have hScaled :
      |degrees| * Real.pi ≤ degreeLimit * piUpper :=
    mul_le_mul
      hDegrees
      real_pi_le_upper
      hPiPositive
      (by norm_num [degreeLimit])
  have hNumeric :
      degreeLimit * piUpper / 180 <
        radianIntermediateLimit := by
    norm_num [degreeLimit, radianIntermediateLimit, piUpper]
  exact lt_of_le_of_lt (div_le_div_of_nonneg_right hScaled (by norm_num)) hNumeric

theorem binary64Radians_abs_lt_limit
    (contract : RoundContract)
    (degrees : ℝ)
    (hDegrees : |degrees| ≤ degreeLimit) :
    |binary64Radians contract degrees| < radianIntermediateLimit := by
  have hError := binary64Radians_error contract degrees hDegrees
  have hActual := abs_actual_le hError
  have hCeiling :
      angleErrorCeiling < radianIntermediateLimit -
        (degreeLimit * piUpper / 180) := by
    norm_num [angleErrorCeiling, radianIntermediateLimit, degreeLimit, piUpper]
  have hExactNumeric :
      |exactRadians degrees| ≤
        degreeLimit * piUpper / 180 := by
    have hPiPositive : 0 ≤ Real.pi := Real.pi_pos.le
    rw [exactRadians, abs_div, abs_mul, abs_of_nonneg hPiPositive]
    norm_num
    exact div_le_div_of_nonneg_right
      (mul_le_mul
        hDegrees
        real_pi_le_upper
        hPiPositive
        (by norm_num [degreeLimit]))
      (by norm_num)
  linarith

end

end Dry.Numeric.Angle

import Dry.Numeric.Angle
import Mathlib.Analysis.SpecialFunctions.Trigonometric.Bounds
import Mathlib.Tactic

/-!
# Profiled libm sine and cosine coefficients

This module imports the pinned `libm` 0.2.16 accuracy policy as an explicit assumption and derives
the absolute-error budget used by planar feature transforms.

The upstream release tests sine and cosine against an MPFR reference with a one-ULP allowance. That
test policy is not itself a universal proof of the implementation, so `LibmContract` keeps the bridge
visible. Over the proved `[-7, 7]` argument interval, the contract separates:

* at most `2^-52` between the libm output and the correctly rounded reference; and
* at most `2^-53` between that reference and exact real trigonometry.

Their sum is conservatively bounded by `2^-51`. Composing this with the checked degree-to-radian
error and the one-Lipschitz property of real sine and cosine gives an end-to-end `2^-45` coefficient
error over `[-360, 360]` degrees.
-/

namespace Dry.Numeric.Trig

open Dry.Numeric.RoundModel
open Dry.Numeric.Binary64
open Dry.Numeric.Angle

noncomputable section

/-- Conservative absolute step for one binary64 ULP when a trigonometric result is in `[-1, 1]`. -/
def oneUlpStepCeiling : ℝ :=
  1 / 2 ^ 52

/--
Conservative absolute error between an exact trigonometric value in `[-1, 1]` and its correctly
rounded binary64 reference.
-/
def correctRoundingErrorCeiling : ℝ :=
  1 / 2 ^ 53

/-- Same-input libm error derived from the imported one-ULP reference policy. -/
def libmErrorCeiling : ℝ :=
  1 / 2 ^ 51

/-- End-to-end degree-to-coefficient error, including binary64 degree conversion and libm. -/
def coefficientErrorCeiling : ℝ :=
  1 / 2 ^ 45

/--
The imported accuracy boundary for pinned `libm` 0.2.16.

`sinReference` and `cosReference` denote the correctly rounded binary64 MPFR reference values used
by the upstream test policy. Refinement of the Rust implementation to every field of this structure
remains an explicit, separate obligation.
-/
structure LibmContract where
  sin : ℝ → ℝ
  cos : ℝ → ℝ
  sinReference : ℝ → ℝ
  cosReference : ℝ → ℝ
  sin_one_ulp_spec :
    ∀ x,
      |x| ≤ radianIntermediateLimit →
        Approx (sin x) (sinReference x) oneUlpStepCeiling
  cos_one_ulp_spec :
    ∀ x,
      |x| ≤ radianIntermediateLimit →
        Approx (cos x) (cosReference x) oneUlpStepCeiling
  sin_reference_spec :
    ∀ x,
      |x| ≤ radianIntermediateLimit →
        Approx (sinReference x) (Real.sin x) correctRoundingErrorCeiling
  cos_reference_spec :
    ∀ x,
      |x| ≤ radianIntermediateLimit →
        Approx (cosReference x) (Real.cos x) correctRoundingErrorCeiling

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

private theorem imported_error_le_ceiling :
    oneUlpStepCeiling + correctRoundingErrorCeiling ≤ libmErrorCeiling := by
  norm_num [oneUlpStepCeiling, correctRoundingErrorCeiling, libmErrorCeiling]

theorem libmSin_error
    (contract : LibmContract)
    (x : ℝ)
    (hRange : |x| ≤ radianIntermediateLimit) :
    Approx (contract.sin x) (Real.sin x) libmErrorCeiling := by
  exact
    approx_mono
      ((contract.sin_one_ulp_spec x hRange).trans
        (contract.sin_reference_spec x hRange))
      imported_error_le_ceiling

theorem libmCos_error
    (contract : LibmContract)
    (x : ℝ)
    (hRange : |x| ≤ radianIntermediateLimit) :
    Approx (contract.cos x) (Real.cos x) libmErrorCeiling := by
  exact
    approx_mono
      ((contract.cos_one_ulp_spec x hRange).trans
        (contract.cos_reference_spec x hRange))
      imported_error_le_ceiling

theorem libmSin_abs_le
    (contract : LibmContract)
    (x : ℝ)
    (hRange : |x| ≤ radianIntermediateLimit) :
    |contract.sin x| ≤ 1 + libmErrorCeiling := by
  have hActual := abs_actual_le (libmSin_error contract x hRange)
  linarith [Real.abs_sin_le_one x]

theorem libmCos_abs_le
    (contract : LibmContract)
    (x : ℝ)
    (hRange : |x| ≤ radianIntermediateLimit) :
    |contract.cos x| ≤ 1 + libmErrorCeiling := by
  have hActual := abs_actual_le (libmCos_error contract x hRange)
  linarith [Real.abs_cos_le_one x]

def binary64Sin
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (degrees : ℝ) : ℝ :=
  libmContract.sin (binary64Radians roundContract degrees)

def binary64Cos
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (degrees : ℝ) : ℝ :=
  libmContract.cos (binary64Radians roundContract degrees)

private theorem sin_input_error
    (roundContract : RoundContract)
    (degrees : ℝ)
    (hDegrees : |degrees| ≤ degreeLimit) :
    Approx
      (Real.sin (binary64Radians roundContract degrees))
      (Real.sin (exactRadians degrees))
      angleErrorCeiling := by
  exact
    (Real.abs_sin_sub_sin_le
      (binary64Radians roundContract degrees)
      (exactRadians degrees)).trans
      (binary64Radians_error roundContract degrees hDegrees)

private theorem cos_input_error
    (roundContract : RoundContract)
    (degrees : ℝ)
    (hDegrees : |degrees| ≤ degreeLimit) :
    Approx
      (Real.cos (binary64Radians roundContract degrees))
      (Real.cos (exactRadians degrees))
      angleErrorCeiling := by
  exact
    (Real.abs_cos_sub_cos_le
      (binary64Radians roundContract degrees)
      (exactRadians degrees)).trans
      (binary64Radians_error roundContract degrees hDegrees)

private theorem composed_error_le_ceiling :
    libmErrorCeiling + angleErrorCeiling ≤ coefficientErrorCeiling := by
  norm_num [libmErrorCeiling, angleErrorCeiling, coefficientErrorCeiling]

theorem binary64Sin_error
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (degrees : ℝ)
    (hDegrees : |degrees| ≤ degreeLimit) :
    Approx
      (binary64Sin roundContract libmContract degrees)
      (Real.sin (exactRadians degrees))
      coefficientErrorCeiling := by
  have hRange :
      |binary64Radians roundContract degrees| ≤ radianIntermediateLimit :=
    (binary64Radians_abs_lt_limit roundContract degrees hDegrees).le
  exact
    approx_mono
      ((libmSin_error
          libmContract
          (binary64Radians roundContract degrees)
          hRange).trans
        (sin_input_error roundContract degrees hDegrees))
      composed_error_le_ceiling

theorem binary64Cos_error
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (degrees : ℝ)
    (hDegrees : |degrees| ≤ degreeLimit) :
    Approx
      (binary64Cos roundContract libmContract degrees)
      (Real.cos (exactRadians degrees))
      coefficientErrorCeiling := by
  have hRange :
      |binary64Radians roundContract degrees| ≤ radianIntermediateLimit :=
    (binary64Radians_abs_lt_limit roundContract degrees hDegrees).le
  exact
    approx_mono
      ((libmCos_error
          libmContract
          (binary64Radians roundContract degrees)
          hRange).trans
        (cos_input_error roundContract degrees hDegrees))
      composed_error_le_ceiling

def binary64Coefficients
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (degrees : ℝ) : ℝ × ℝ :=
  (binary64Cos roundContract libmContract degrees,
    binary64Sin roundContract libmContract degrees)

def exactCoefficients (degrees : ℝ) : ℝ × ℝ :=
  (Real.cos (exactRadians degrees), Real.sin (exactRadians degrees))

def CoefficientsApprox
    (actual exact : ℝ × ℝ)
    (error : ℝ) : Prop :=
  Approx actual.1 exact.1 error ∧ Approx actual.2 exact.2 error

theorem binary64Coefficients_error
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (degrees : ℝ)
    (hDegrees : |degrees| ≤ degreeLimit) :
    CoefficientsApprox
      (binary64Coefficients roundContract libmContract degrees)
      (exactCoefficients degrees)
      coefficientErrorCeiling :=
  ⟨binary64Cos_error roundContract libmContract degrees hDegrees,
    binary64Sin_error roundContract libmContract degrees hDegrees⟩

end

end Dry.Numeric.Trig

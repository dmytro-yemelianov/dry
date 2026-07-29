import Dry.Numeric.RoundModel
import Mathlib.Tactic

/-!
# Profiled binary64 local-operation bounds

This module instantiates the parametric rounding graph in `Dry.Numeric.RoundModel` with explicit
absolute-error ceilings for IEEE-754 binary64 round-to-nearest, ties-to-even.

The bridge is deliberately conditional. `RoundContract` records the standard binary64 rounding
property over the profile's largest exact basic-operation result. Refinement from Rust `f64` to this
contract, coefficient construction through `libm`, input-representation error and repeated-transform
accumulation remain separate obligations.
-/

namespace Dry.Numeric.Binary64

open Dry.Geometry.PlanarTransform
open Dry.Numeric.RoundModel

noncomputable section

def unitRoundoff : ℝ :=
  1 / 2 ^ 53

def halfMinSubnormal : ℝ :=
  1 / 2 ^ 1075

def multiplyResultLimit : ℝ :=
  2 ^ 20

def addSubResultLimit : ℝ :=
  2 ^ 22

def multiplyErrorCeiling : ℝ :=
  1 / 2 ^ 32

def addSubErrorCeiling : ℝ :=
  1 / 2 ^ 30

def vectorXYErrorCeiling : ℝ :=
  1 / 2 ^ 29

def pointXYErrorCeiling : ℝ :=
  1 / 2 ^ 28

/--
The scoped IEEE-754 premise used by this packet. `round` denotes conversion of an exact real basic
operation result to binary64 using round-to-nearest, ties-to-even. The error expression includes half
the minimum subnormal so that the premise also covers gradual underflow.
-/
structure RoundContract where
  round : ℝ → ℝ
  round_spec :
    ∀ exact,
      |exact| ≤ addSubResultLimit →
        Approx
          (round exact)
          exact
          (unitRoundoff * |exact| + halfMinSubnormal)

private theorem multiply_limit_le_add_sub_limit :
    multiplyResultLimit ≤ addSubResultLimit := by
  norm_num [multiplyResultLimit, addSubResultLimit]

private theorem half_min_subnormal_le_mul_term :
    halfMinSubnormal ≤ 1 / (2 : ℝ) ^ 33 := by
  simpa [halfMinSubnormal] using
    (one_div_pow_le_one_div_pow_of_le (a := (2 : ℝ)) (by norm_num) (by norm_num : 33 ≤ 1075))

private theorem half_min_subnormal_le_add_term :
    halfMinSubnormal ≤ 1 / (2 : ℝ) ^ 31 := by
  simpa [halfMinSubnormal] using
    (one_div_pow_le_one_div_pow_of_le (a := (2 : ℝ)) (by norm_num) (by norm_num : 31 ≤ 1075))

private theorem multiply_error_le_ceiling
    {exact : ℝ}
    (hExact : |exact| ≤ multiplyResultLimit) :
    unitRoundoff * |exact| + halfMinSubnormal ≤ multiplyErrorCeiling := by
  have hUnitNonneg : 0 ≤ unitRoundoff := by
    norm_num [unitRoundoff]
  have hScaled :
      unitRoundoff * |exact| ≤ unitRoundoff * multiplyResultLimit :=
    mul_le_mul_of_nonneg_left hExact hUnitNonneg
  have hUnitLimit :
      unitRoundoff * multiplyResultLimit = 1 / (2 : ℝ) ^ 33 := by
    norm_num [unitRoundoff, multiplyResultLimit]
  calc
    unitRoundoff * |exact| + halfMinSubnormal
        ≤ unitRoundoff * multiplyResultLimit + halfMinSubnormal :=
      add_le_add hScaled le_rfl
    _ ≤ 1 / (2 : ℝ) ^ 33 + 1 / (2 : ℝ) ^ 33 := by
      rw [hUnitLimit]
      exact add_le_add le_rfl half_min_subnormal_le_mul_term
    _ = multiplyErrorCeiling := by
      norm_num [multiplyErrorCeiling]

private theorem add_sub_error_le_ceiling
    {exact : ℝ}
    (hExact : |exact| ≤ addSubResultLimit) :
    unitRoundoff * |exact| + halfMinSubnormal ≤ addSubErrorCeiling := by
  have hUnitNonneg : 0 ≤ unitRoundoff := by
    norm_num [unitRoundoff]
  have hScaled :
      unitRoundoff * |exact| ≤ unitRoundoff * addSubResultLimit :=
    mul_le_mul_of_nonneg_left hExact hUnitNonneg
  have hUnitLimit :
      unitRoundoff * addSubResultLimit = 1 / (2 : ℝ) ^ 31 := by
    norm_num [unitRoundoff, addSubResultLimit]
  calc
    unitRoundoff * |exact| + halfMinSubnormal
        ≤ unitRoundoff * addSubResultLimit + halfMinSubnormal :=
      add_le_add hScaled le_rfl
    _ ≤ 1 / (2 : ℝ) ^ 31 + 1 / (2 : ℝ) ^ 31 := by
      rw [hUnitLimit]
      exact add_le_add le_rfl half_min_subnormal_le_add_term
    _ = addSubErrorCeiling := by
      norm_num [addSubErrorCeiling]

private theorem approx_mono
    {actual exact first second : ℝ}
    (hApprox : Approx actual exact first)
    (hError : first ≤ second) :
    Approx actual exact second :=
  le_trans hApprox hError

def profiledRound (contract : RoundContract) (limit exact : ℝ) : ℝ :=
  if |exact| ≤ limit then contract.round exact else exact

private theorem profiled_round_eq
    (contract : RoundContract)
    {limit exact : ℝ}
    (hExact : |exact| ≤ limit) :
    profiledRound contract limit exact = contract.round exact := by
  simp [profiledRound, hExact]

private theorem profiled_round_outside_eq
    (contract : RoundContract)
    {limit exact : ℝ}
    (hExact : ¬ |exact| ≤ limit) :
    profiledRound contract limit exact = exact := by
  simp [profiledRound, hExact]

private theorem profiled_add_sub_spec
    (contract : RoundContract)
    (exact : ℝ) :
    Approx
      (profiledRound contract addSubResultLimit exact)
      exact
      addSubErrorCeiling := by
  by_cases hExact : |exact| ≤ addSubResultLimit
  · rw [profiled_round_eq contract hExact]
    exact approx_mono (contract.round_spec exact hExact) (add_sub_error_le_ceiling hExact)
  · rw [profiled_round_outside_eq contract hExact]
    simp [Approx, addSubErrorCeiling]

private theorem profiled_multiply_spec
    (contract : RoundContract)
    (exact : ℝ) :
    Approx
      (profiledRound contract multiplyResultLimit exact)
      exact
      multiplyErrorCeiling := by
  by_cases hExact : |exact| ≤ multiplyResultLimit
  · rw [profiled_round_eq contract hExact]
    have hRound :=
      contract.round_spec exact (hExact.trans multiply_limit_le_add_sub_limit)
    exact approx_mono hRound (multiply_error_le_ceiling hExact)
  · rw [profiled_round_outside_eq contract hExact]
    simp [Approx, multiplyErrorCeiling]

/--
An `Ops` instance whose in-profile branch is binary64 rounding and whose out-of-profile branch is the
exact operation. The latter makes the uniform `Ops` contract total without claiming anything about
overflowing Rust operations. The `*_eq_round` lemmas below establish the binary64 connection exactly
where the numeric profile applies.
-/
def ops (contract : RoundContract) : Ops where
  add left right :=
    profiledRound contract addSubResultLimit (left + right)
  sub left right :=
    profiledRound contract addSubResultLimit (left - right)
  mul left right :=
    profiledRound contract multiplyResultLimit (left * right)
  addError := addSubErrorCeiling
  mulError := multiplyErrorCeiling
  addError_nonneg := by norm_num [addSubErrorCeiling]
  mulError_nonneg := by norm_num [multiplyErrorCeiling]
  add_spec left right := profiled_add_sub_spec contract (left + right)
  sub_spec left right := profiled_add_sub_spec contract (left - right)
  mul_spec left right := profiled_multiply_spec contract (left * right)

theorem ops_add_eq_round
    (contract : RoundContract)
    {left right : ℝ}
    (hResult : |left + right| ≤ addSubResultLimit) :
    (ops contract).add left right = contract.round (left + right) :=
  profiled_round_eq contract hResult

theorem ops_sub_eq_round
    (contract : RoundContract)
    {left right : ℝ}
    (hResult : |left - right| ≤ addSubResultLimit) :
    (ops contract).sub left right = contract.round (left - right) :=
  profiled_round_eq contract hResult

theorem ops_mul_eq_round
    (contract : RoundContract)
    {left right : ℝ}
    (hResult : |left * right| ≤ multiplyResultLimit) :
    (ops contract).mul left right = contract.round (left * right) :=
  profiled_round_eq contract hResult

/-- The direct binary64 operation graph for planar vector application. -/
def binary64ApplyVector
    (contract : RoundContract)
    (transform : Transform)
    (vector : Vec3) : Vec3 :=
  {
    x :=
      contract.round
        (contract.round (transform.c * vector.x) -
          contract.round (transform.s * vector.y))
    y :=
      contract.round
        (contract.round (transform.s * vector.x) +
          contract.round (transform.c * vector.y))
    z := vector.z
  }

/-- Every exact basic-operation result used by `binary64ApplyVector` is inside its profile limit. -/
def VectorGraphInRange
    (contract : RoundContract)
    (transform : Transform)
    (vector : Vec3) : Prop :=
  |transform.c * vector.x| ≤ multiplyResultLimit ∧
    |transform.s * vector.y| ≤ multiplyResultLimit ∧
      |contract.round (transform.c * vector.x) -
          contract.round (transform.s * vector.y)| ≤ addSubResultLimit ∧
        |transform.s * vector.x| ≤ multiplyResultLimit ∧
          |transform.c * vector.y| ≤ multiplyResultLimit ∧
            |contract.round (transform.s * vector.x) +
                contract.round (transform.c * vector.y)| ≤ addSubResultLimit

theorem binary64ApplyVector_eq_profiled
    (contract : RoundContract)
    (transform : Transform)
    (vector : Vec3)
    (hRange : VectorGraphInRange contract transform vector) :
    binary64ApplyVector contract transform vector =
      roundedApplyVector (ops contract) transform vector := by
  rcases hRange with ⟨hcx, hsy, hx, hsx, hcy, hy⟩
  ext
  · simp only [binary64ApplyVector, roundedApplyVector]
    rw [ops_mul_eq_round contract hcx, ops_mul_eq_round contract hsy]
    rw [ops_sub_eq_round contract hx]
  · simp only [binary64ApplyVector, roundedApplyVector]
    rw [ops_mul_eq_round contract hsx, ops_mul_eq_round contract hcy]
    rw [ops_add_eq_round contract hy]
  · rfl

/-- The direct binary64 operation graph for planar point application. -/
def binary64ApplyPoint
    (contract : RoundContract)
    (transform : Transform)
    (point : Vec3) : Vec3 :=
  let rotated := binary64ApplyVector contract transform point
  {
    x := contract.round (rotated.x + transform.translation.x)
    y := contract.round (rotated.y + transform.translation.y)
    z := contract.round (rotated.z + transform.translation.z)
  }

/-- Every exact basic-operation result used by `binary64ApplyPoint` is inside its profile limit. -/
def PointGraphInRange
    (contract : RoundContract)
    (transform : Transform)
    (point : Vec3) : Prop :=
  VectorGraphInRange contract transform point ∧
    |(binary64ApplyVector contract transform point).x + transform.translation.x| ≤
        addSubResultLimit ∧
      |(binary64ApplyVector contract transform point).y + transform.translation.y| ≤
          addSubResultLimit ∧
        |point.z + transform.translation.z| ≤ addSubResultLimit

theorem binary64ApplyPoint_eq_profiled
    (contract : RoundContract)
    (transform : Transform)
    (point : Vec3)
    (hRange : PointGraphInRange contract transform point) :
    binary64ApplyPoint contract transform point =
      roundedApplyPoint (ops contract) transform point := by
  rcases hRange with ⟨hVectorRange, hx, hy, hz⟩
  have hVector :=
    binary64ApplyVector_eq_profiled contract transform point hVectorRange
  ext
  · change
      contract.round
          ((binary64ApplyVector contract transform point).x + transform.translation.x) =
        (ops contract).add
          (roundedApplyVector (ops contract) transform point).x
          transform.translation.x
    rw [← hVector]
    exact (ops_add_eq_round contract hx).symm
  · change
      contract.round
          ((binary64ApplyVector contract transform point).y + transform.translation.y) =
        (ops contract).add
          (roundedApplyVector (ops contract) transform point).y
          transform.translation.y
    rw [← hVector]
    exact (ops_add_eq_round contract hy).symm
  · change contract.round (point.z + transform.translation.z) =
      (ops contract).add point.z transform.translation.z
    exact (ops_add_eq_round contract hz).symm

/-- The direct binary64 operation graph for planar transform composition. -/
def binary64Compose
    (contract : RoundContract)
    (outer inner : Transform) : Transform :=
  let coefficient :=
    binary64ApplyVector contract outer ⟨inner.c, inner.s, 0⟩
  {
    c := coefficient.x
    s := coefficient.y
    translation := binary64ApplyPoint contract outer inner.translation
  }

/-- Every exact basic-operation result used by `binary64Compose` is inside its profile limit. -/
def ComposeGraphInRange
    (contract : RoundContract)
    (outer inner : Transform) : Prop :=
  VectorGraphInRange contract outer ⟨inner.c, inner.s, 0⟩ ∧
    PointGraphInRange contract outer inner.translation

theorem binary64Compose_eq_profiled
    (contract : RoundContract)
    (outer inner : Transform)
    (hRange : ComposeGraphInRange contract outer inner) :
    binary64Compose contract outer inner =
      roundedCompose (ops contract) outer inner := by
  rcases hRange with ⟨hCoefficientRange, hTranslationRange⟩
  have hCoefficient :=
    binary64ApplyVector_eq_profiled
      contract outer ⟨inner.c, inner.s, 0⟩ hCoefficientRange
  have hTranslation :=
    binary64ApplyPoint_eq_profiled
      contract outer inner.translation hTranslationRange
  simpa [binary64Compose, roundedCompose] using
    congrArg₂
      (fun coefficient translation =>
        Transform.mk coefficient.x coefficient.y translation)
      hCoefficient
      hTranslation

theorem vector_error_formula_le_ceiling (contract : RoundContract) :
    vectorXYError (ops contract) ≤ vectorXYErrorCeiling := by
  norm_num [
    vectorXYError,
    ops,
    addSubErrorCeiling,
    multiplyErrorCeiling,
    vectorXYErrorCeiling
  ]

theorem point_error_formula_le_ceiling (contract : RoundContract) :
    pointXYError (ops contract) ≤ pointXYErrorCeiling := by
  norm_num [
    pointXYError,
    ops,
    addSubErrorCeiling,
    multiplyErrorCeiling,
    pointXYErrorCeiling
  ]

theorem applyVector_profiled_error
    (contract : RoundContract)
    (transform : Transform)
    (vector : Vec3) :
    VectorError
      (roundedApplyVector (ops contract) transform vector)
      (applyVector transform vector)
      vectorXYErrorCeiling
      0 := by
  have h := applyVector_error (ops contract) transform vector
  exact
    ⟨approx_mono h.1 (vector_error_formula_le_ceiling contract),
      approx_mono h.2.1 (vector_error_formula_le_ceiling contract),
      h.2.2⟩

theorem applyPoint_profiled_error
    (contract : RoundContract)
    (transform : Transform)
    (point : Vec3) :
    VectorError
      (roundedApplyPoint (ops contract) transform point)
      (applyPoint transform point)
      pointXYErrorCeiling
      addSubErrorCeiling := by
  have h := applyPoint_error (ops contract) transform point
  exact
    ⟨approx_mono h.1 (point_error_formula_le_ceiling contract),
      approx_mono h.2.1 (point_error_formula_le_ceiling contract),
      h.2.2⟩

theorem compose_profiled_error
    (contract : RoundContract)
    (outer inner : Transform) :
    TransformError
      (roundedCompose (ops contract) outer inner)
      (compose outer inner)
      vectorXYErrorCeiling
      pointXYErrorCeiling
      addSubErrorCeiling := by
  have h := compose_error (ops contract) outer inner
  exact
    ⟨approx_mono h.1 (vector_error_formula_le_ceiling contract),
      approx_mono h.2.1 (vector_error_formula_le_ceiling contract),
      approx_mono h.2.2.1 (point_error_formula_le_ceiling contract),
      approx_mono h.2.2.2.1 (point_error_formula_le_ceiling contract),
      h.2.2.2.2⟩

theorem binary64ApplyVector_error
    (contract : RoundContract)
    (transform : Transform)
    (vector : Vec3)
    (hRange : VectorGraphInRange contract transform vector) :
    VectorError
      (binary64ApplyVector contract transform vector)
      (applyVector transform vector)
      vectorXYErrorCeiling
      0 := by
  rw [binary64ApplyVector_eq_profiled contract transform vector hRange]
  exact applyVector_profiled_error contract transform vector

theorem binary64ApplyPoint_error
    (contract : RoundContract)
    (transform : Transform)
    (point : Vec3)
    (hRange : PointGraphInRange contract transform point) :
    VectorError
      (binary64ApplyPoint contract transform point)
      (applyPoint transform point)
      pointXYErrorCeiling
      addSubErrorCeiling := by
  rw [binary64ApplyPoint_eq_profiled contract transform point hRange]
  exact applyPoint_profiled_error contract transform point

theorem binary64Compose_error
    (contract : RoundContract)
    (outer inner : Transform)
    (hRange : ComposeGraphInRange contract outer inner) :
    TransformError
      (binary64Compose contract outer inner)
      (compose outer inner)
      vectorXYErrorCeiling
      pointXYErrorCeiling
      addSubErrorCeiling := by
  rw [binary64Compose_eq_profiled contract outer inner hRange]
  exact compose_profiled_error contract outer inner

end

end Dry.Numeric.Binary64

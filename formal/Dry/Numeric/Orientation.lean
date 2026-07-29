import Dry.Numeric.ApplicationAccumulation
import Mathlib.Analysis.InnerProductSpace.PiL2
import Mathlib.Analysis.SpecialFunctions.Trigonometric.Bounds
import Mathlib.Geometry.Euclidean.Angle.Unoriented.Basic
import Mathlib.Tactic

/-!
# Orientation validity and angular error

This module turns the existing componentwise orientation bound into a directional guarantee. Exact
tool orientations are unit vectors. If a computed vector is within `2^-8` in each planar component
and copies Z exactly, its Euclidean distance from the exact unit direction is at most `2^-7`; it is
therefore nonzero and its unoriented angular error is at most `1/4` radian.

The final tree theorem inherits all profile, range, rounding and imported-`libm` premises of
`binary64Tree_applyVector_error`. It does not claim that arbitrary Rust inputs meet those premises.
-/

namespace Dry.Numeric.Orientation

open scoped RealInnerProductSpace

open Dry.Geometry.PlanarTransform
open Dry.Numeric.RoundModel
open Dry.Numeric.Binary64
open Dry.Numeric.Accumulation
open Dry.Numeric.CompositionTree
open Dry.Numeric.ApplicationAccumulation

noncomputable section

abbrev E3 := EuclideanSpace ℝ (Fin 3)

def asE3 (vector : Vec3) : E3 :=
  EuclideanSpace.single 0 vector.x +
    (EuclideanSpace.single 1 vector.y +
      EuclideanSpace.single 2 vector.z)

def IsUnit (vector : Vec3) : Prop :=
  ‖asE3 vector‖ = 1

def IsNonzero (vector : Vec3) : Prop :=
  asE3 vector ≠ 0

def directionCosine (actual exact : Vec3) : ℝ :=
  ⟪asE3 actual, asE3 exact⟫ /
    (‖asE3 actual‖ * ‖asE3 exact‖)

def angularError (actual exact : Vec3) : ℝ :=
  Real.arccos (directionCosine actual exact)

def orientationDistanceCeiling : ℝ :=
  1 / 2 ^ 7

def orientationAngularErrorCeiling : ℝ :=
  1 / 4

@[simp]
theorem asE3_apply (vector : Vec3) :
    asE3 vector 0 = vector.x ∧
      asE3 vector 1 = vector.y ∧
      asE3 vector 2 = vector.z := by
  simp [asE3]

theorem asE3_sub (actual exact : Vec3) :
    asE3 actual - asE3 exact =
      EuclideanSpace.single 0 (actual.x - exact.x) +
        (EuclideanSpace.single 1 (actual.y - exact.y) +
          EuclideanSpace.single 2 (actual.z - exact.z)) := by
  ext index
  fin_cases index <;> simp [asE3]

theorem VectorError.distance
    {actual exact : Vec3}
    {xyError zError : ℝ}
    (hError : VectorError actual exact xyError zError) :
    ‖asE3 actual - asE3 exact‖ ≤
      2 * xyError + zError := by
  rw [asE3_sub]
  calc
    ‖EuclideanSpace.single (0 : Fin 3) (actual.x - exact.x) +
          (EuclideanSpace.single (1 : Fin 3) (actual.y - exact.y) +
            EuclideanSpace.single (2 : Fin 3) (actual.z - exact.z))‖
        ≤
      ‖EuclideanSpace.single (0 : Fin 3) (actual.x - exact.x)‖ +
        ‖EuclideanSpace.single (1 : Fin 3) (actual.y - exact.y) +
          EuclideanSpace.single (2 : Fin 3) (actual.z - exact.z)‖ :=
      norm_add_le _ _
    _ ≤
      ‖EuclideanSpace.single (0 : Fin 3) (actual.x - exact.x)‖ +
        (‖EuclideanSpace.single (1 : Fin 3) (actual.y - exact.y)‖ +
          ‖EuclideanSpace.single (2 : Fin 3) (actual.z - exact.z)‖) :=
      add_le_add_right (norm_add_le _ _) _
    _ =
      |actual.x - exact.x| +
        (|actual.y - exact.y| + |actual.z - exact.z|) := by
      simp [Real.norm_eq_abs]
    _ ≤ xyError + (xyError + zError) :=
      add_le_add hError.1 (add_le_add hError.2.1 hError.2.2)
    _ = 2 * xyError + zError := by ring

theorem unit_nonzero
    {vector : Vec3}
    (hUnit : IsUnit vector) :
    IsNonzero vector := by
  intro hZero
  have : ‖asE3 vector‖ = 0 := by rw [hZero, norm_zero]
  rw [hUnit] at this
  norm_num at this

private theorem cos_quarter_le_alignment :
    Real.cos (1 / 4 : ℝ) ≤ (127 / 129 : ℝ) := by
  have hBound := Real.cos_bound (x := (1 / 4 : ℝ)) (by norm_num)
  have hUpper :=
    (abs_le.mp hBound).2
  norm_num at hUpper ⊢
  linarith

theorem angularError_le_quarter
    {actual exact : Vec3}
    (hExactUnit : IsUnit exact)
    (hDistance :
      ‖asE3 actual - asE3 exact‖ ≤ orientationDistanceCeiling) :
    IsNonzero actual ∧
      angularError actual exact ≤ orientationAngularErrorCeiling := by
  let actualE := asE3 actual
  let exactE := asE3 exact
  have hExactNorm : ‖exactE‖ = 1 := hExactUnit
  have hDistance' : ‖actualE - exactE‖ ≤ 1 / 128 := by
    norm_num [actualE, exactE, orientationDistanceCeiling] at hDistance ⊢
    exact hDistance
  have hActualNormUpper : ‖actualE‖ ≤ 129 / 128 := by
    calc
      ‖actualE‖ =
          ‖(actualE - exactE) + exactE‖ := by
        congr 1
        abel
      _ ≤ ‖actualE - exactE‖ + ‖exactE‖ := norm_add_le _ _
      _ ≤ 1 / 128 + 1 := by rw [hExactNorm]; gcongr
      _ = 129 / 128 := by norm_num
  have hActualNormLower : 127 / 128 ≤ ‖actualE‖ := by
    calc
      127 / 128 = 1 - 1 / 128 := by norm_num
      _ ≤ ‖exactE‖ - ‖actualE - exactE‖ := by
        rw [hExactNorm]
        linarith
      _ ≤ ‖actualE‖ := by
        have hTriangle :
            ‖exactE‖ ≤ ‖actualE - exactE‖ + ‖actualE‖ := by
          calc
            ‖exactE‖ =
                ‖-(actualE - exactE) + actualE‖ := by
              congr 1
              abel
            _ ≤ ‖-(actualE - exactE)‖ + ‖actualE‖ := norm_add_le _ _
            _ = ‖actualE - exactE‖ + ‖actualE‖ := by rw [norm_neg]
        linarith
  have hActualNormPos : 0 < ‖actualE‖ := by
    linarith
  have hActualNonzero : IsNonzero actual := by
    simpa [IsNonzero, actualE] using (norm_pos_iff.mp hActualNormPos)
  have hInnerError :
      |⟪actualE - exactE, exactE⟫| ≤ 1 / 128 := by
    calc
      |⟪actualE - exactE, exactE⟫|
          ≤ ‖actualE - exactE‖ * ‖exactE‖ :=
        abs_real_inner_le_norm _ _
      _ ≤ (1 / 128) * 1 := by
        rw [hExactNorm]
        gcongr
      _ = 1 / 128 := by norm_num
  have hInnerLower :
      127 / 128 ≤ ⟪actualE, exactE⟫ := by
    have hExactInner :
        ⟪exactE, exactE⟫ = 1 := by
      rw [real_inner_self_eq_norm_sq, hExactNorm]
      norm_num
    have hExpand :
        ⟪actualE, exactE⟫ =
          ⟪exactE, exactE⟫ +
            ⟪actualE - exactE, exactE⟫ := by
      rw [inner_sub_left]
      ring
    rw [hExpand, hExactInner]
    have := (abs_le.mp hInnerError).1
    linarith
  have hAlignment :
      (127 / 129 : ℝ) ≤ directionCosine actual exact := by
    rw [directionCosine]
    change
      (127 / 129 : ℝ) ≤
        ⟪actualE, exactE⟫ / (‖actualE‖ * ‖exactE‖)
    rw [hExactNorm, mul_one]
    apply (le_div_iff₀ hActualNormPos).2
    calc
      (127 / 129 : ℝ) * ‖actualE‖
          ≤ (127 / 129 : ℝ) * (129 / 128) := by
        gcongr
      _ = 127 / 128 := by norm_num
      _ ≤ ⟪actualE, exactE⟫ := hInnerLower
  refine ⟨hActualNonzero, ?_⟩
  have hCos :
      Real.cos (1 / 4 : ℝ) ≤ directionCosine actual exact :=
    cos_quarter_le_alignment.trans hAlignment
  calc
    angularError actual exact =
        Real.arccos (directionCosine actual exact) := rfl
    _ ≤ Real.arccos (Real.cos (1 / 4 : ℝ)) :=
      Real.arccos_le_arccos hCos
    _ = 1 / 4 := by
      apply Real.arccos_cos
      · norm_num
      · have := Real.pi_gt_three
        linarith
    _ = orientationAngularErrorCeiling := by
      rfl

theorem applyVector_unit
    (transform : Transform)
    (vector : Vec3)
    (hCoefficient : ‖coefficient transform‖ = 1)
    (hUnit : IsUnit vector) :
    IsUnit (applyVector transform vector) := by
  have hCoefficientSq :
      transform.c ^ 2 + transform.s ^ 2 = 1 := by
    have hNormSq := congrArg (fun value : ℝ => value ^ 2) hCoefficient
    simp [coefficient, Complex.sq_norm] at hNormSq
    nlinarith
  have hInputSq :
      vector.x ^ 2 + vector.y ^ 2 + vector.z ^ 2 = 1 := by
    have hNormSq := congrArg (fun value : ℝ => value ^ 2) hUnit
    simp [asE3, EuclideanSpace.real_norm_sq_eq,
      Fin.sum_univ_succ] at hNormSq
    nlinarith
  have hOutputSq :
      (applyVector transform vector).x ^ 2 +
          (applyVector transform vector).y ^ 2 +
          (applyVector transform vector).z ^ 2 = 1 := by
    simp only [applyVector]
    nlinarith [hCoefficientSq, hInputSq]
  have hNonneg : 0 ≤ ‖asE3 (applyVector transform vector)‖ :=
    norm_nonneg _
  have hNormSq :
      ‖asE3 (applyVector transform vector)‖ ^ 2 = 1 := by
    simp [asE3, EuclideanSpace.real_norm_sq_eq,
      Fin.sum_univ_succ]
    nlinarith
  unfold IsUnit
  nlinarith

theorem binary64Tree_applyVector_angular_error
    (tree : TransformTree)
    (roundContract : RoundContract)
    (libmContract : Dry.Numeric.Trig.LibmContract)
    (vector : Vec3)
    (hProfiled : tree.Profiled)
    (hTreeRange : tree.GraphInRange roundContract libmContract)
    (hCount : tree.composeCount ≤ compositionCountLimit)
    (hVectorX : |vector.x| ≤ orientationComponentLimit)
    (hVectorY : |vector.y| ≤ orientationComponentLimit)
    (hVectorUnit : IsUnit vector)
    (hApplicationRange :
      VectorGraphInRange
        roundContract
        (tree.binary64Eval roundContract libmContract)
        vector) :
    let actual :=
      binary64ApplyVector
        roundContract
        (tree.binary64Eval roundContract libmContract)
        vector
    let exact := applyVector tree.exactEval vector
    IsNonzero actual ∧
      angularError actual exact ≤ orientationAngularErrorCeiling := by
  dsimp
  have hError :=
    binary64Tree_applyVector_error
      tree roundContract libmContract vector
      hProfiled hTreeRange hCount hVectorX hVectorY hApplicationRange
  have hDistance :
      ‖asE3
          (binary64ApplyVector
            roundContract
            (tree.binary64Eval roundContract libmContract)
            vector) -
        asE3 (applyVector tree.exactEval vector)‖ ≤
        orientationDistanceCeiling := by
    have := VectorError.distance hError
    norm_num [treeOrientationXYErrorCeiling,
      orientationDistanceCeiling] at this ⊢
    exact this
  have hExactUnit :
      IsUnit (applyVector tree.exactEval vector) :=
    applyVector_unit
      tree.exactEval
      vector
      tree.exact_coefficient_norm
      hVectorUnit
  exact angularError_le_quarter hExactUnit hDistance

end

end Dry.Numeric.Orientation

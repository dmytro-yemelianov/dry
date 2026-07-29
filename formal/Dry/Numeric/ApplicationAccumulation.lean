import Dry.Numeric.CompositionTree
import Mathlib.Analysis.Complex.Norm
import Mathlib.Tactic

/-!
# Bounded application after arbitrary planar composition trees

This module composes the parenthesization-preserving transform-tree bounds with the checked local
binary64 point/vector operation graphs. It bounds the final transformed point, Arc centre or
orientation against application of the exact-real transform tree to the same represented input.

The result remains conditional on the provisional profile, every transform-composition graph and the
final application graph satisfying their exact-operation range predicates, and the named binary64
rounding and imported `libm` contracts.
-/

namespace Dry.Numeric.ApplicationAccumulation

open Dry.Geometry.PlanarTransform
open Dry.Numeric.RoundModel
open Dry.Numeric.Binary64
open Dry.Numeric.Accumulation
open Dry.Numeric.CompositionTree

noncomputable section

def xy (vector : Vec3) : ℂ :=
  ⟨vector.x, vector.y⟩

def localCoordinateComponentLimit : ℝ :=
  2 ^ 20

def orientationComponentLimit : ℝ :=
  1

def treePointXYErrorCeiling : ℝ :=
  2 ^ 31

def treePointZErrorCeiling : ℝ :=
  1 / 2 ^ 12

def treeOrientationXYErrorCeiling : ℝ :=
  1 / 2 ^ 8

private theorem approx_mono
    {actual exact first second : ℝ}
    (hApprox : Approx actual exact first)
    (hError : first ≤ second) :
    Approx actual exact second :=
  le_trans hApprox hError

theorem xy_applyVector
    (transform : Transform)
    (vector : Vec3) :
    xy (applyVector transform vector) =
      coefficient transform * xy vector := by
  apply Complex.ext
  · simp [xy, coefficient, applyVector]
  · simp [xy, coefficient, applyVector]
    ring

theorem xy_applyPoint
    (transform : Transform)
    (point : Vec3) :
    xy (applyPoint transform point) =
      coefficient transform * xy point + translationXY transform := by
  apply Complex.ext
  · simp [xy, coefficient, translationXY, applyPoint, applyVector]
  · simp [xy, coefficient, translationXY, applyPoint, applyVector]
    ring

theorem VectorError.xy_norm
    {actual exact : Vec3}
    {xyError zError : ℝ}
    (hError : VectorError actual exact xyError zError) :
    ‖xy actual - xy exact‖ ≤ 2 * xyError := by
  calc
    ‖xy actual - xy exact‖
        ≤
      |(xy actual - xy exact).re| +
        |(xy actual - xy exact).im| :=
      Complex.norm_le_abs_re_add_abs_im _
    _ ≤ xyError + xyError :=
      add_le_add hError.1 hError.2.1
    _ = 2 * xyError := by ring

theorem xy_norm_le_two_mul
    (vector : Vec3)
    (limit : ℝ)
    (hx : |vector.x| ≤ limit)
    (hy : |vector.y| ≤ limit) :
    ‖xy vector‖ ≤ 2 * limit := by
  calc
    ‖xy vector‖ ≤ |(xy vector).re| + |(xy vector).im| :=
      Complex.norm_le_abs_re_add_abs_im _
    _ ≤ limit + limit := add_le_add hx hy
    _ = 2 * limit := by ring

theorem applyVector_xy_sensitivity
    (actual exact : Transform)
    (vector : Vec3) :
    ‖xy (applyVector actual vector) -
        xy (applyVector exact vector)‖ ≤
      ‖coefficient actual - coefficient exact‖ * ‖xy vector‖ := by
  rw [xy_applyVector, xy_applyVector]
  rw [show
    coefficient actual * xy vector -
        coefficient exact * xy vector =
      (coefficient actual - coefficient exact) * xy vector by ring]
  rw [norm_mul]

theorem applyPoint_xy_sensitivity
    (actual exact : Transform)
    (point : Vec3) :
    ‖xy (applyPoint actual point) -
        xy (applyPoint exact point)‖ ≤
      ‖coefficient actual - coefficient exact‖ * ‖xy point‖ +
        ‖translationXY actual - translationXY exact‖ := by
  rw [xy_applyPoint, xy_applyPoint]
  rw [show
    (coefficient actual * xy point + translationXY actual) -
        (coefficient exact * xy point + translationXY exact) =
      (coefficient actual - coefficient exact) * xy point +
        (translationXY actual - translationXY exact) by ring]
  exact (norm_add_le _ _).trans_eq (by rw [norm_mul])

private theorem vector_error_of_xy_norm
    {actual exact : Vec3}
    {xyError zError : ℝ}
    (hXY : ‖xy actual - xy exact‖ ≤ xyError)
    (hZ : Approx actual.z exact.z zError) :
    VectorError actual exact xyError zError := by
  refine ⟨?_, ?_, hZ⟩
  · simpa [Approx, xy] using
      (Complex.abs_re_le_norm (xy actual - xy exact)).trans hXY
  · simpa [Approx, xy] using
      (Complex.abs_im_le_norm (xy actual - xy exact)).trans hXY

theorem binary64Tree_applyPoint_error
    (tree : TransformTree)
    (roundContract : RoundContract)
    (libmContract : Dry.Numeric.Trig.LibmContract)
    (point : Vec3)
    (hProfiled : tree.Profiled)
    (hTreeRange : tree.GraphInRange roundContract libmContract)
    (hCount : tree.composeCount ≤ compositionCountLimit)
    (hPointX : |point.x| ≤ localCoordinateComponentLimit)
    (hPointY : |point.y| ≤ localCoordinateComponentLimit)
    (hApplicationRange :
      PointGraphInRange
        roundContract
        (tree.binary64Eval roundContract libmContract)
        point) :
    VectorError
      (binary64ApplyPoint
        roundContract
        (tree.binary64Eval roundContract libmContract)
        point)
      (applyPoint tree.exactEval point)
      treePointXYErrorCeiling
      treePointZErrorCeiling := by
  let actualTransform :=
    tree.binary64Eval roundContract libmContract
  let exactTransform := tree.exactEval
  let actualPoint :=
    binary64ApplyPoint roundContract actualTransform point
  let intermediatePoint := applyPoint actualTransform point
  let exactPoint := applyPoint exactTransform point
  have hTree :=
    binary64Tree_error
      tree roundContract libmContract hProfiled hTreeRange hCount
  have hLocal :
      VectorError
        actualPoint
        intermediatePoint
        pointXYErrorCeiling
        addSubErrorCeiling := by
    simpa [actualPoint, intermediatePoint, actualTransform] using
      binary64ApplyPoint_error
        roundContract
        actualTransform
        point
        (by simpa [actualTransform] using hApplicationRange)
  have hLocalXY :
      ‖xy actualPoint - xy intermediatePoint‖ ≤
        2 * pointXYErrorCeiling :=
    VectorError.xy_norm hLocal
  have hPointNorm :
      ‖xy point‖ ≤ 2 * localCoordinateComponentLimit :=
    xy_norm_le_two_mul
      point localCoordinateComponentLimit hPointX hPointY
  have hCoefficientPoint :
      ‖coefficient actualTransform - coefficient exactTransform‖ *
          ‖xy point‖ ≤
        repeatCoefficientErrorCeiling *
          (2 * localCoordinateComponentLimit) :=
    mul_le_mul
      hTree.1
      hPointNorm
      (norm_nonneg _)
      (by norm_num [repeatCoefficientErrorCeiling])
  have hSensitivity :
      ‖xy intermediatePoint - xy exactPoint‖ ≤
        repeatCoefficientErrorCeiling *
            (2 * localCoordinateComponentLimit) +
          treeTranslationXYErrorCeiling := by
    exact
      (applyPoint_xy_sensitivity actualTransform exactTransform point).trans
        (add_le_add hCoefficientPoint hTree.2.1)
  have hXY :
      ‖xy actualPoint - xy exactPoint‖ ≤
        2 * pointXYErrorCeiling +
          (repeatCoefficientErrorCeiling *
              (2 * localCoordinateComponentLimit) +
            treeTranslationXYErrorCeiling) := by
    calc
      ‖xy actualPoint - xy exactPoint‖ =
          ‖(xy actualPoint - xy intermediatePoint) +
            (xy intermediatePoint - xy exactPoint)‖ := by
        congr 1
        ring
      _ ≤
          ‖xy actualPoint - xy intermediatePoint‖ +
            ‖xy intermediatePoint - xy exactPoint‖ :=
        norm_add_le _ _
      _ ≤ _ := add_le_add hLocalXY hSensitivity
  have hXYCeiling :
      2 * pointXYErrorCeiling +
            (repeatCoefficientErrorCeiling *
                (2 * localCoordinateComponentLimit) +
              treeTranslationXYErrorCeiling) ≤
        treePointXYErrorCeiling := by
    norm_num [
      pointXYErrorCeiling,
      repeatCoefficientErrorCeiling,
      localCoordinateComponentLimit,
      treeTranslationXYErrorCeiling,
      treePointXYErrorCeiling
    ]
  have hZSensitivity :
      Approx
        intermediatePoint.z
        exactPoint.z
        repeatTranslationZErrorCeiling := by
    have hAdded :=
      Approx.addExactRight
        (right := point.z)
        hTree.2.2
    simpa [intermediatePoint, exactPoint, actualTransform, exactTransform,
      applyPoint, applyVector, add_comm] using hAdded
  have hZ :=
    hLocal.2.2.trans hZSensitivity
  have hZCeiling :
      addSubErrorCeiling + repeatTranslationZErrorCeiling ≤
        treePointZErrorCeiling := by
    norm_num [
      addSubErrorCeiling,
      repeatTranslationZErrorCeiling,
      treePointZErrorCeiling
    ]
  exact
    vector_error_of_xy_norm
      (hXY.trans hXYCeiling)
      (approx_mono hZ hZCeiling)

theorem binary64Tree_applyVector_error
    (tree : TransformTree)
    (roundContract : RoundContract)
    (libmContract : Dry.Numeric.Trig.LibmContract)
    (vector : Vec3)
    (hProfiled : tree.Profiled)
    (hTreeRange : tree.GraphInRange roundContract libmContract)
    (hCount : tree.composeCount ≤ compositionCountLimit)
    (hVectorX : |vector.x| ≤ orientationComponentLimit)
    (hVectorY : |vector.y| ≤ orientationComponentLimit)
    (hApplicationRange :
      VectorGraphInRange
        roundContract
        (tree.binary64Eval roundContract libmContract)
        vector) :
    VectorError
      (binary64ApplyVector
        roundContract
        (tree.binary64Eval roundContract libmContract)
        vector)
      (applyVector tree.exactEval vector)
      treeOrientationXYErrorCeiling
      0 := by
  let actualTransform :=
    tree.binary64Eval roundContract libmContract
  let exactTransform := tree.exactEval
  let actualVector :=
    binary64ApplyVector roundContract actualTransform vector
  let intermediateVector := applyVector actualTransform vector
  let exactVector := applyVector exactTransform vector
  have hTree :=
    binary64Tree_error
      tree roundContract libmContract hProfiled hTreeRange hCount
  have hLocal :
      VectorError
        actualVector
        intermediateVector
        vectorXYErrorCeiling
        0 := by
    simpa [actualVector, intermediateVector, actualTransform] using
      binary64ApplyVector_error
        roundContract
        actualTransform
        vector
        (by simpa [actualTransform] using hApplicationRange)
  have hLocalXY :
      ‖xy actualVector - xy intermediateVector‖ ≤
        2 * vectorXYErrorCeiling :=
    VectorError.xy_norm hLocal
  have hVectorNorm :
      ‖xy vector‖ ≤ 2 * orientationComponentLimit :=
    xy_norm_le_two_mul
      vector orientationComponentLimit hVectorX hVectorY
  have hSensitivity :
      ‖xy intermediateVector - xy exactVector‖ ≤
        repeatCoefficientErrorCeiling *
          (2 * orientationComponentLimit) := by
    exact
      (applyVector_xy_sensitivity actualTransform exactTransform vector).trans
        (mul_le_mul
          hTree.1
          hVectorNorm
          (norm_nonneg _)
          (by norm_num [repeatCoefficientErrorCeiling]))
  have hXY :
      ‖xy actualVector - xy exactVector‖ ≤
        2 * vectorXYErrorCeiling +
          repeatCoefficientErrorCeiling *
            (2 * orientationComponentLimit) := by
    calc
      ‖xy actualVector - xy exactVector‖ =
          ‖(xy actualVector - xy intermediateVector) +
            (xy intermediateVector - xy exactVector)‖ := by
        congr 1
        ring
      _ ≤
          ‖xy actualVector - xy intermediateVector‖ +
            ‖xy intermediateVector - xy exactVector‖ :=
        norm_add_le _ _
      _ ≤ _ := add_le_add hLocalXY hSensitivity
  have hXYCeiling :
      2 * vectorXYErrorCeiling +
            repeatCoefficientErrorCeiling *
              (2 * orientationComponentLimit) ≤
        treeOrientationXYErrorCeiling := by
    norm_num [
      vectorXYErrorCeiling,
      repeatCoefficientErrorCeiling,
      orientationComponentLimit,
      treeOrientationXYErrorCeiling
    ]
  have hZ :
      Approx actualVector.z exactVector.z 0 := by
    simp [Approx, actualVector, exactVector, actualTransform, exactTransform,
      binary64ApplyVector, applyVector]
  exact
    vector_error_of_xy_norm
      (hXY.trans hXYCeiling)
      hZ

end

end Dry.Numeric.ApplicationAccumulation

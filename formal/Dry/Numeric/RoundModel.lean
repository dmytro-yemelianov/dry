import Dry.Geometry.PlanarTransform
import Mathlib.Tactic

/-!
# Parametric rounding-error composition

This module models the operation graph of `features::Transform` with abstract rounded addition,
subtraction and multiplication. It proves how local operation bounds compose for planar vector/point
application and transform composition.

The model is deliberately parametric: it does not assert that Rust binary64 or `libm` satisfies a
particular local bound. Instantiating `Ops` for the pinned implementation and adding coefficient/input
representation error are separate FM1.4 obligations.
-/

namespace Dry.Numeric.RoundModel

open Dry.Geometry.PlanarTransform

def Approx (actual exact error : ℝ) : Prop :=
  |actual - exact| ≤ error

theorem Approx.trans {actual intermediate exact first second : ℝ}
    (hFirst : Approx actual intermediate first)
    (hSecond : Approx intermediate exact second) :
    Approx actual exact (first + second) := by
  calc
    |actual - exact| =
        |(actual - intermediate) + (intermediate - exact)| := by ring_nf
    _ ≤ |actual - intermediate| + |intermediate - exact| := abs_add_le _ _
    _ ≤ first + second := add_le_add hFirst hSecond

theorem Approx.add {leftActual leftExact leftError rightActual rightExact rightError : ℝ}
    (hLeft : Approx leftActual leftExact leftError)
    (hRight : Approx rightActual rightExact rightError) :
    Approx (leftActual + rightActual) (leftExact + rightExact) (leftError + rightError) := by
  calc
    |(leftActual + rightActual) - (leftExact + rightExact)| =
        |(leftActual - leftExact) + (rightActual - rightExact)| := by ring_nf
    _ ≤ |leftActual - leftExact| + |rightActual - rightExact| := abs_add_le _ _
    _ ≤ leftError + rightError := add_le_add hLeft hRight

theorem Approx.sub {leftActual leftExact leftError rightActual rightExact rightError : ℝ}
    (hLeft : Approx leftActual leftExact leftError)
    (hRight : Approx rightActual rightExact rightError) :
    Approx (leftActual - rightActual) (leftExact - rightExact) (leftError + rightError) := by
  calc
    |(leftActual - rightActual) - (leftExact - rightExact)| =
        |(leftActual - leftExact) - (rightActual - rightExact)| := by ring_nf
    _ ≤ |leftActual - leftExact| + |rightActual - rightExact| := abs_sub _ _
    _ ≤ leftError + rightError := add_le_add hLeft hRight

theorem Approx.addExactRight {actual exact error right : ℝ}
    (h : Approx actual exact error) :
    Approx (actual + right) (exact + right) error := by
  simpa [Approx] using h

structure Ops where
  add : ℝ → ℝ → ℝ
  sub : ℝ → ℝ → ℝ
  mul : ℝ → ℝ → ℝ
  addError : ℝ
  mulError : ℝ
  addError_nonneg : 0 ≤ addError
  mulError_nonneg : 0 ≤ mulError
  add_spec : ∀ left right, Approx (add left right) (left + right) addError
  sub_spec : ∀ left right, Approx (sub left right) (left - right) addError
  mul_spec : ∀ left right, Approx (mul left right) (left * right) mulError

def vectorXYError (ops : Ops) : ℝ :=
  ops.addError + 2 * ops.mulError

def pointXYError (ops : Ops) : ℝ :=
  2 * ops.addError + 2 * ops.mulError

def roundedApplyVector (ops : Ops) (transform : Transform) (vector : Vec3) : Vec3 :=
  {
    x := ops.sub (ops.mul transform.c vector.x) (ops.mul transform.s vector.y)
    y := ops.add (ops.mul transform.s vector.x) (ops.mul transform.c vector.y)
    z := vector.z
  }

def roundedApplyPoint (ops : Ops) (transform : Transform) (point : Vec3) : Vec3 :=
  let rotated := roundedApplyVector ops transform point
  {
    x := ops.add rotated.x transform.translation.x
    y := ops.add rotated.y transform.translation.y
    z := ops.add rotated.z transform.translation.z
  }

def roundedCompose (ops : Ops) (outer inner : Transform) : Transform :=
  let coefficient := roundedApplyVector ops outer ⟨inner.c, inner.s, 0⟩
  {
    c := coefficient.x
    s := coefficient.y
    translation := roundedApplyPoint ops outer inner.translation
  }

def VectorError (actual exact : Vec3) (xyError zError : ℝ) : Prop :=
  Approx actual.x exact.x xyError ∧
    Approx actual.y exact.y xyError ∧
      Approx actual.z exact.z zError

def TransformError
    (actual exact : Transform)
    (coefficientError translationXYError translationZError : ℝ) : Prop :=
  Approx actual.c exact.c coefficientError ∧
    Approx actual.s exact.s coefficientError ∧
      VectorError actual.translation exact.translation translationXYError translationZError

private theorem x_product_error (ops : Ops) (transform : Transform) (vector : Vec3) :
    Approx
      (ops.sub (ops.mul transform.c vector.x) (ops.mul transform.s vector.y))
      (transform.c * vector.x - transform.s * vector.y)
      (vectorXYError ops) := by
  have hProducts := Approx.sub
    (ops.mul_spec transform.c vector.x)
    (ops.mul_spec transform.s vector.y)
  have hLocal := ops.sub_spec
    (ops.mul transform.c vector.x)
    (ops.mul transform.s vector.y)
  have h := hLocal.trans hProducts
  simpa [vectorXYError, two_mul] using h

private theorem y_product_error (ops : Ops) (transform : Transform) (vector : Vec3) :
    Approx
      (ops.add (ops.mul transform.s vector.x) (ops.mul transform.c vector.y))
      (transform.s * vector.x + transform.c * vector.y)
      (vectorXYError ops) := by
  have hProducts := Approx.add
    (ops.mul_spec transform.s vector.x)
    (ops.mul_spec transform.c vector.y)
  have hLocal := ops.add_spec
    (ops.mul transform.s vector.x)
    (ops.mul transform.c vector.y)
  have h := hLocal.trans hProducts
  simpa [vectorXYError, two_mul] using h

theorem applyVector_error (ops : Ops) (transform : Transform) (vector : Vec3) :
    VectorError
      (roundedApplyVector ops transform vector)
      (applyVector transform vector)
      (vectorXYError ops)
      0 := by
  refine ⟨?_, ?_, ?_⟩
  · simpa [roundedApplyVector, applyVector] using x_product_error ops transform vector
  · simpa [roundedApplyVector, applyVector] using y_product_error ops transform vector
  · simp [Approx, roundedApplyVector, applyVector]

private theorem point_component_error
    (ops : Ops)
    {rounded exact translation : ℝ}
    (hRounded : Approx rounded exact (vectorXYError ops)) :
    Approx
      (ops.add rounded translation)
      (exact + translation)
      (pointXYError ops) := by
  have hLocal := ops.add_spec rounded translation
  have hInput := hRounded.addExactRight (right := translation)
  have h := hLocal.trans hInput
  have hError :
      ops.addError + vectorXYError ops = pointXYError ops := by
    simp [pointXYError, vectorXYError]
    ring
  rw [← hError]
  exact h

theorem applyPoint_error (ops : Ops) (transform : Transform) (point : Vec3) :
    VectorError
      (roundedApplyPoint ops transform point)
      (applyPoint transform point)
      (pointXYError ops)
      ops.addError := by
  have hVector := applyVector_error ops transform point
  refine ⟨?_, ?_, ?_⟩
  · exact point_component_error ops hVector.1
  · exact point_component_error ops hVector.2.1
  · simpa [roundedApplyPoint, roundedApplyVector, applyPoint, applyVector] using
      ops.add_spec point.z transform.translation.z

theorem compose_error (ops : Ops) (outer inner : Transform) :
    TransformError
      (roundedCompose ops outer inner)
      (compose outer inner)
      (vectorXYError ops)
      (pointXYError ops)
      ops.addError := by
  have hCoefficient := applyVector_error ops outer ⟨inner.c, inner.s, 0⟩
  have hTranslation := applyPoint_error ops outer inner.translation
  refine ⟨?_, ?_, ?_⟩
  · simpa [roundedCompose, compose, applyVector] using hCoefficient.1
  · simpa [roundedCompose, compose, applyVector] using hCoefficient.2.1
  · simpa [roundedCompose, compose, applyPoint] using hTranslation

end Dry.Numeric.RoundModel

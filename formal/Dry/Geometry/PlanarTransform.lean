import Mathlib.Data.Real.Basic
import Mathlib.Tactic

/-!
# Planar feature transforms

This module models the exact-real algebra behind `features::Transform`. It does not claim that the
binary64 Rust implementation or the `libm` trigonometric inputs are exact; those are separate numeric
and refinement obligations in the FM1 claim registry.
-/

namespace Dry.Geometry.PlanarTransform

@[ext]
structure Vec3 where
  x : ℝ
  y : ℝ
  z : ℝ

structure Transform where
  c : ℝ
  s : ℝ
  translation : Vec3

def identity : Transform :=
  {
    c := 1
    s := 0
    translation := ⟨0, 0, 0⟩
  }

def applyVector (transform : Transform) (vector : Vec3) : Vec3 :=
  {
    x := transform.c * vector.x - transform.s * vector.y
    y := transform.s * vector.x + transform.c * vector.y
    z := vector.z
  }

def applyPoint (transform : Transform) (point : Vec3) : Vec3 :=
  let rotated := applyVector transform point
  {
    x := rotated.x + transform.translation.x
    y := rotated.y + transform.translation.y
    z := rotated.z + transform.translation.z
  }

def compose (outer inner : Transform) : Transform :=
  let translated := applyVector outer inner.translation
  {
    c := outer.c * inner.c - outer.s * inner.s
    s := outer.s * inner.c + outer.c * inner.s
    translation :=
      {
        x := translated.x + outer.translation.x
        y := translated.y + outer.translation.y
        z := translated.z + outer.translation.z
      }
  }

@[simp]
theorem apply_identity (point : Vec3) : applyPoint identity point = point := by
  ext <;> simp [applyPoint, applyVector, identity]

theorem apply_compose (outer inner : Transform) (point : Vec3) :
    applyPoint (compose outer inner) point =
      applyPoint outer (applyPoint inner point) := by
  ext <;> simp [applyPoint, applyVector, compose] <;> ring

end Dry.Geometry.PlanarTransform

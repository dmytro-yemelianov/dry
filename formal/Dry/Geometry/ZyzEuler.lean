import Mathlib.Data.Real.Basic
import Mathlib.Analysis.SpecialFunctions.Trigonometric.Basic
import Mathlib.Analysis.SpecialFunctions.Trigonometric.Arctan
import Mathlib.Analysis.SpecialFunctions.Trigonometric.Inverse
import Mathlib.Analysis.SpecialFunctions.Complex.Arg
import Mathlib.Tactic

/-!
# ZYZ Euler Angle Conversions and Direction Roundtrip (Abstract Layer A)

This module formalizes the exact-real geometry of the tool orientation mapping between
a unit tool direction vector and ZYZ Euler angles per §5.3 of `docs/28-apt-irbcam-dialects.md`.

It covers Layer A (abstract mathematical properties):
- Theorem I6 (`direction_spin_independent`): the tool direction is independent of the free roll spin `rz2`.
- Theorem I7 (`direction_euler_roundtrip`): converting a unit direction vector to Euler angles and
  evaluating the forward direction recovers the exact unit direction vector.

It does NOT model:
- Binary64 floating-point rounding or IEEE-754 approximations in libm `acos`/`atan2`.
- Concrete IRBCAM target file serialization or degree-to-radian conversions.
- Robot wrist singular configurations or kinematic joint angle limits.
-/

namespace Dry.Geometry.ZyzEuler

noncomputable section

set_option linter.unusedVariables false

/-- Two-argument arctangent defined via the complex argument function. -/
def Real.arctan2 (y x : ℝ) : ℝ := Complex.arg ⟨x, y⟩

@[ext]
structure Vec3 where
  x : ℝ
  y : ℝ
  z : ℝ

instance : Repr Vec3 where
  reprPrec _ _ := "⟨Vec3⟩"

/-- A vector is a unit vector if the sum of squares of its components equals 1. -/
def IsUnit (v : Vec3) : Prop := v.x^2 + v.y^2 + v.z^2 = 1

/--
Forward direction vector resulting from applying ZYZ Euler rotations `Rz(rz1) * Ry(ry) * Rz(rz2)`
to the tool unit Z axis `(0, 0, 1)`.
-/
def direction (rz1 ry rz2 : ℝ) : Vec3 :=
  ⟨Real.cos rz1 * Real.sin ry, Real.sin rz1 * Real.sin ry, Real.cos ry⟩

/--
Extracts ZYZ Euler angles `(rz1, ry, rz2)` from a direction vector and specified spin angle.
-/
def euler (d : Vec3) (spin : ℝ) : ℝ × ℝ × ℝ :=
  (if d.x^2 + d.y^2 ≠ 0 then Real.arctan2 d.y d.x else 0, Real.arccos d.z, spin)

/-- Theorem I6: The tool direction vector is strictly independent of the spin angle `s`. -/
theorem direction_spin_independent (rz1 ry s : ℝ) : direction rz1 ry s = direction rz1 ry 0 := by
  rfl

/--
Theorem I7: For any unit direction vector `d`, round-tripping through Euler decomposition
recovers the exact original direction vector `d`.
-/
theorem direction_euler_roundtrip (d : Vec3) (hUnit : IsUnit d) (spin : ℝ) :
    direction (euler d spin).1 (euler d spin).2.1 (euler d spin).2.2 = d := by
  dsimp [direction, euler]
  have hunit_eq : d.x^2 + d.y^2 + d.z^2 = 1 := hUnit
  have hz_sq_le_one : d.z^2 ≤ 1 := by
    have hx2 : 0 ≤ d.x^2 := sq_nonneg d.x
    have hy2 : 0 ≤ d.y^2 := sq_nonneg d.y
    linarith
  have hz_le_one : d.z ≤ 1 := by
    nlinarith
  have hneg_one_le_z : -1 ≤ d.z := by
    nlinarith
  have hcos_z : Real.cos (Real.arccos d.z) = d.z := Real.cos_arccos hneg_one_le_z hz_le_one
  have hsin_z : Real.sin (Real.arccos d.z) = Real.sqrt (d.x^2 + d.y^2) := by
    rw [Real.sin_arccos]
    congr 1
    linarith
  have hnorm : ‖(⟨d.x, d.y⟩ : ℂ)‖ = Real.sqrt (d.x^2 + d.y^2) := by
    rw [Complex.norm_def]
    congr 1
    simp [Complex.normSq]
    ring
  by_cases hxy : d.x^2 + d.y^2 = 0
  · have hx0 : d.x = 0 := by
      have : d.x^2 ≤ 0 := by linarith [sq_nonneg d.y]
      nlinarith
    have hy0 : d.y = 0 := by
      have : d.y^2 ≤ 0 := by linarith [sq_nonneg d.x]
      nlinarith
    have hsin_zero : Real.sin (Real.arccos d.z) = 0 := by
      rw [hsin_z, hxy, Real.sqrt_zero]
    rw [if_neg (by simp [hxy])]
    ext
    · simp [hsin_zero, hx0]
    · simp [hsin_zero, hy0]
    · exact hcos_z
  · rw [if_pos hxy]
    have hcos_arg := Complex.norm_mul_cos_arg ⟨d.x, d.y⟩
    have hsin_arg := Complex.norm_mul_sin_arg ⟨d.x, d.y⟩
    dsimp [Real.arctan2]
    ext
    · calc Real.cos (Complex.arg ⟨d.x, d.y⟩) * Real.sin (Real.arccos d.z)
        _ = Real.cos (Complex.arg ⟨d.x, d.y⟩) * ‖(⟨d.x, d.y⟩ : ℂ)‖ := by rw [hsin_z, hnorm]
        _ = ‖(⟨d.x, d.y⟩ : ℂ)‖ * Real.cos (Complex.arg ⟨d.x, d.y⟩) := mul_comm _ _
        _ = d.x := by rw [hcos_arg]
    · calc Real.sin (Complex.arg ⟨d.x, d.y⟩) * Real.sin (Real.arccos d.z)
        _ = Real.sin (Complex.arg ⟨d.x, d.y⟩) * ‖(⟨d.x, d.y⟩ : ℂ)‖ := by rw [hsin_z, hnorm]
        _ = ‖(⟨d.x, d.y⟩ : ℂ)‖ * Real.sin (Complex.arg ⟨d.x, d.y⟩) := mul_comm _ _
        _ = d.y := by rw [hsin_arg]
    · exact hcos_z

end

end Dry.Geometry.ZyzEuler

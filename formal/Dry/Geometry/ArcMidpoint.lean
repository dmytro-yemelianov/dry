import Mathlib.Data.Real.Basic
import Mathlib.Analysis.SpecialFunctions.Trigonometric.Basic
import Mathlib.Tactic

/-!
# Circular Arc Midpoint Geometry (Abstract Layer A)

This module formalizes the exact-real geometry of circular arc midpoints per §5.4 of
`docs/28-apt-irbcam-dialects.md`.

In IRBCAM (and robot controllers like KRL/RAPID), circular moves are commanded via three
points: start, midpoint, and end. This module proves:
- Theorem I8 (`midpoint_on_circle`): the computed midpoint lies exactly on the circle of radius `ρ`.
- `startpoint_on_circle`, `endpoint_on_circle`: start and end points lie on the circle.
- `midpoint_angle_bisects`: the midpoint angular position bisects the arc sweep `Δ`.
- `midpoint_ne_startpoint`, `midpoint_ne_endpoint`: for non-degenerate radii (`ρ > 0`) and valid
  non-zero sweeps (`0 < |Δ| < 2π`), the midpoint is distinct from both the start and endpoints.

It does NOT model:
- Binary64 floating-point rounding and libm trigonometric tolerances.
- Chording tolerance and linear approximation algorithms.
- Helical arcs with non-zero Z pitch.
-/

namespace Dry.Geometry.ArcMidpoint

noncomputable section

@[ext]
structure Vec2 where
  x : ℝ
  y : ℝ

instance : Repr Vec2 where
  reprPrec _ _ := "⟨Vec2⟩"

/-- Squared Euclidean distance between two 2D points. -/
def distSq (p q : Vec2) : ℝ := (p.x - q.x)^2 + (p.y - q.y)^2

/--
Arc midpoint at half the signed sweep angle `Δ / 2` relative to start angle `φ`.
-/
def midpoint (c : Vec2) (ρ φ Δ : ℝ) : Vec2 :=
  ⟨c.x + ρ * Real.cos (φ + Δ / 2), c.y + ρ * Real.sin (φ + Δ / 2)⟩

/--
Arc endpoint at the full signed sweep angle `Δ` relative to start angle `φ`.
-/
def endpoint (c : Vec2) (ρ φ Δ : ℝ) : Vec2 :=
  ⟨c.x + ρ * Real.cos (φ + Δ), c.y + ρ * Real.sin (φ + Δ)⟩

/--
Arc startpoint at the initial angle `φ`.
-/
def startpoint (c : Vec2) (ρ φ : ℝ) : Vec2 :=
  ⟨c.x + ρ * Real.cos φ, c.y + ρ * Real.sin φ⟩

/-- Theorem I8: The arc midpoint lies on the circle of radius `ρ` centered at `c`. -/
theorem midpoint_on_circle (c : Vec2) (ρ φ Δ : ℝ) (_hρ : 0 ≤ ρ) :
    distSq (midpoint c ρ φ Δ) c = ρ^2 := by
  dsimp [distSq, midpoint]
  calc
    (c.x + ρ * Real.cos (φ + Δ / 2) - c.x)^2 + (c.y + ρ * Real.sin (φ + Δ / 2) - c.y)^2
      = ρ^2 * (Real.cos (φ + Δ / 2)^2 + Real.sin (φ + Δ / 2)^2) := by ring
    _ = ρ^2 * 1 := by rw [Real.cos_sq_add_sin_sq]
    _ = ρ^2 := mul_one _

/-- The arc startpoint lies on the circle of radius `ρ` centered at `c`. -/
theorem startpoint_on_circle (c : Vec2) (ρ φ : ℝ) :
    distSq (startpoint c ρ φ) c = ρ^2 := by
  dsimp [distSq, startpoint]
  calc
    (c.x + ρ * Real.cos φ - c.x)^2 + (c.y + ρ * Real.sin φ - c.y)^2
      = ρ^2 * (Real.cos φ^2 + Real.sin φ^2) := by ring
    _ = ρ^2 * 1 := by rw [Real.cos_sq_add_sin_sq]
    _ = ρ^2 := mul_one _

/-- The arc endpoint lies on the circle of radius `ρ` centered at `c`. -/
theorem endpoint_on_circle (c : Vec2) (ρ φ Δ : ℝ) :
    distSq (endpoint c ρ φ Δ) c = ρ^2 := by
  dsimp [distSq, endpoint]
  calc
    (c.x + ρ * Real.cos (φ + Δ) - c.x)^2 + (c.y + ρ * Real.sin (φ + Δ) - c.y)^2
      = ρ^2 * (Real.cos (φ + Δ)^2 + Real.sin (φ + Δ)^2) := by ring
    _ = ρ^2 * 1 := by rw [Real.cos_sq_add_sin_sq]
    _ = ρ^2 := mul_one _

/-- Sweep preservation / bisector theorem: midpoint angle bisects the sweep. -/
theorem midpoint_angle_bisects (φ Δ : ℝ) :
    (φ + Δ / 2) - φ = Δ / 2 ∧ (φ + Δ) - (φ + Δ / 2) = Δ / 2 := by
  constructor <;> ring

/-- Midpoint is distinct from startpoint when radius is positive and sweep is non-zero within (-2π, 2π). -/
theorem midpoint_ne_startpoint (c : Vec2) (ρ φ Δ : ℝ)
    (hρ : 0 < ρ) (hΔ_nonzero : Δ ≠ 0) (hΔ_bound : |Δ| < 2 * Real.pi) :
    midpoint c ρ φ Δ ≠ startpoint c ρ φ := by
  intro hEq
  have hx : (midpoint c ρ φ Δ).x = (startpoint c ρ φ).x := by rw [hEq]
  have hy : (midpoint c ρ φ Δ).y = (startpoint c ρ φ).y := by rw [hEq]
  dsimp [midpoint, startpoint] at hx hy
  have hcos : Real.cos (φ + Δ / 2) = Real.cos φ := by
    linarith [mul_left_cancel₀ hρ.ne' (by linarith : ρ * Real.cos (φ + Δ / 2) = ρ * Real.cos φ)]
  have hsin : Real.sin (φ + Δ / 2) = Real.sin φ := by
    linarith [mul_left_cancel₀ hρ.ne' (by linarith : ρ * Real.sin (φ + Δ / 2) = ρ * Real.sin φ)]
  have hcos_sub : Real.cos ((φ + Δ / 2) - φ) = 1 := by
    rw [Real.cos_sub, hcos, hsin]
    have h1 := Real.cos_sq_add_sin_sq φ
    linear_combination h1
  have h_ang : (φ + Δ / 2) - φ = Δ / 2 := by ring
  rw [h_ang] at hcos_sub
  have h_bound1 : -(2 * Real.pi) < Δ / 2 := by
    have : 0 < Real.pi := Real.pi_pos
    have h_abs : |Δ / 2| < Real.pi := by
      rw [abs_div, Nat.abs_ofNat]
      linarith [abs_nonneg Δ]
    have := abs_lt.mp h_abs
    linarith
  have h_bound2 : Δ / 2 < 2 * Real.pi := by
    have : 0 < Real.pi := Real.pi_pos
    have h_abs : |Δ / 2| < Real.pi := by
      rw [abs_div, Nat.abs_ofNat]
      linarith [abs_nonneg Δ]
    have := abs_lt.mp h_abs
    linarith
  have h_zero : Δ / 2 = 0 := (Real.cos_eq_one_iff_of_lt_of_lt h_bound1 h_bound2).mp hcos_sub
  have : Δ = 0 := by linarith
  exact hΔ_nonzero this

/-- Midpoint is distinct from endpoint when radius is positive and sweep is non-zero within (-2π, 2π). -/
theorem midpoint_ne_endpoint (c : Vec2) (ρ φ Δ : ℝ)
    (hρ : 0 < ρ) (hΔ_nonzero : Δ ≠ 0) (hΔ_bound : |Δ| < 2 * Real.pi) :
    midpoint c ρ φ Δ ≠ endpoint c ρ φ Δ := by
  intro hEq
  have hx : (midpoint c ρ φ Δ).x = (endpoint c ρ φ Δ).x := by rw [hEq]
  have hy : (midpoint c ρ φ Δ).y = (endpoint c ρ φ Δ).y := by rw [hEq]
  dsimp [midpoint, endpoint] at hx hy
  have hcos : Real.cos (φ + Δ / 2) = Real.cos (φ + Δ) := by
    linarith [mul_left_cancel₀ hρ.ne' (by linarith : ρ * Real.cos (φ + Δ / 2) = ρ * Real.cos (φ + Δ))]
  have hsin : Real.sin (φ + Δ / 2) = Real.sin (φ + Δ) := by
    linarith [mul_left_cancel₀ hρ.ne' (by linarith : ρ * Real.sin (φ + Δ / 2) = ρ * Real.sin (φ + Δ))]
  have hcos_sub : Real.cos ((φ + Δ) - (φ + Δ / 2)) = 1 := by
    rw [Real.cos_sub, ← hcos, ← hsin]
    have h1 := Real.cos_sq_add_sin_sq (φ + Δ / 2)
    linear_combination h1
  have h_ang : (φ + Δ) - (φ + Δ / 2) = Δ / 2 := by ring
  rw [h_ang] at hcos_sub
  have h_bound1 : -(2 * Real.pi) < Δ / 2 := by
    have : 0 < Real.pi := Real.pi_pos
    have h_abs : |Δ / 2| < Real.pi := by
      rw [abs_div, Nat.abs_ofNat]
      linarith [abs_nonneg Δ]
    have := abs_lt.mp h_abs
    linarith
  have h_bound2 : Δ / 2 < 2 * Real.pi := by
    have : 0 < Real.pi := Real.pi_pos
    have h_abs : |Δ / 2| < Real.pi := by
      rw [abs_div, Nat.abs_ofNat]
      linarith [abs_nonneg Δ]
    have := abs_lt.mp h_abs
    linarith
  have h_zero : Δ / 2 = 0 := (Real.cos_eq_one_iff_of_lt_of_lt h_bound1 h_bound2).mp hcos_sub
  have : Δ = 0 := by linarith
  exact hΔ_nonzero this

/-- Combined distinctness: midpoint is distinct from both start and endpoints. -/
theorem midpoint_distinct (c : Vec2) (ρ φ Δ : ℝ)
    (hρ : 0 < ρ) (hΔ_nonzero : Δ ≠ 0) (hΔ_bound : |Δ| < 2 * Real.pi) :
    midpoint c ρ φ Δ ≠ startpoint c ρ φ ∧ midpoint c ρ φ Δ ≠ endpoint c ρ φ Δ :=
  ⟨midpoint_ne_startpoint c ρ φ Δ hρ hΔ_nonzero hΔ_bound,
   midpoint_ne_endpoint c ρ φ Δ hρ hΔ_nonzero hΔ_bound⟩

end

end Dry.Geometry.ArcMidpoint

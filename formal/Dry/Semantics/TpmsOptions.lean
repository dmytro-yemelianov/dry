import Mathlib.Data.Rat.Defs
import Mathlib.Tactic

/-!
# TPMS Option Acceptance & Domain Predicate (FM1.GENERATE.TPMS.OPTION_ACCEPTANCE)

This module formalizes the TPMS option acceptance predicate:
- `TpmsOptions`: bounding box (min/max X, Y, Z), cell size (X, Y, Z), layer height, resolution;
- `validateTpmsOptions`: checks that bounds are strictly positive in extent, cell size is strictly positive, layer height is positive, and resolution is ≥ 2;
- Proves fail-closed property and soundness theorem `validate_tpms_options_sound`.
-/

namespace Dry.Semantics.TpmsOptions

structure Point3D where
  x : ℚ
  y : ℚ
  z : ℚ
deriving DecidableEq, Repr

structure TpmsOptions where
  bounds_min : Point3D
  bounds_max : Point3D
  cell_size : Point3D
  layer_height : ℚ
  resolution : ℕ
deriving DecidableEq, Repr

def validateTpmsOptions (opts : TpmsOptions) : Bool :=
  decide (
    opts.bounds_min.x < opts.bounds_max.x ∧
    opts.bounds_min.y < opts.bounds_max.y ∧
    opts.bounds_min.z < opts.bounds_max.z ∧
    0 < opts.cell_size.x ∧
    0 < opts.cell_size.y ∧
    0 < opts.cell_size.z ∧
    0 < opts.layer_height ∧
    2 ≤ opts.resolution
  )

theorem validate_tpms_options_sound (opts : TpmsOptions)
    (h : validateTpmsOptions opts = true) :
    opts.bounds_min.x < opts.bounds_max.x ∧
    opts.bounds_min.y < opts.bounds_max.y ∧
    opts.bounds_min.z < opts.bounds_max.z ∧
    0 < opts.cell_size.x ∧
    0 < opts.cell_size.y ∧
    0 < opts.cell_size.z ∧
    0 < opts.layer_height ∧
    2 ≤ opts.resolution := by
  exact decide_eq_true_iff.mp h

theorem validate_tpms_options_fail_closed_degenerate_bounds (opts : TpmsOptions)
    (h : opts.bounds_max.x ≤ opts.bounds_min.x ∨
         opts.bounds_max.y ≤ opts.bounds_min.y ∨
         opts.bounds_max.z ≤ opts.bounds_min.z) :
    validateTpmsOptions opts = false := by
  dsimp [validateTpmsOptions]
  apply decide_eq_false
  intro accepted
  rcases h with hx | hy | hz
  · exact (not_lt_of_ge hx) accepted.1
  · exact (not_lt_of_ge hy) accepted.2.1
  · exact (not_lt_of_ge hz) accepted.2.2.1

theorem validate_tpms_options_fail_closed_non_positive_cell (opts : TpmsOptions)
    (h : opts.cell_size.x ≤ 0 ∨ opts.cell_size.y ≤ 0 ∨ opts.cell_size.z ≤ 0) :
    validateTpmsOptions opts = false := by
  dsimp [validateTpmsOptions]
  apply decide_eq_false
  intro accepted
  rcases h with hx | hy | hz
  · exact (not_lt_of_ge hx) accepted.2.2.2.1
  · exact (not_lt_of_ge hy) accepted.2.2.2.2.1
  · exact (not_lt_of_ge hz) accepted.2.2.2.2.2.1

theorem validate_tpms_options_fail_closed_invalid_layer_height (opts : TpmsOptions)
    (h : opts.layer_height ≤ 0) :
    validateTpmsOptions opts = false := by
  dsimp [validateTpmsOptions]
  apply decide_eq_false
  intro accepted
  exact (not_lt_of_ge h) accepted.2.2.2.2.2.2.1

theorem validate_tpms_options_fail_closed_low_resolution (opts : TpmsOptions)
    (h : opts.resolution < 2) :
    validateTpmsOptions opts = false := by
  dsimp [validateTpmsOptions]
  apply decide_eq_false
  intro accepted
  exact (not_le_of_gt h) accepted.2.2.2.2.2.2.2

end Dry.Semantics.TpmsOptions

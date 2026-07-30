import Dry.Language.Common
import Mathlib.Data.Rat.Defs
import Mathlib.Tactic

/-!
# Deposition and Extrusion Math Semantics (FM1.5c)

This module formalizes the physical deposition and filament consumption laws over exact rationals:
- Extrusion volume equation: $V = \text{length} \times \text{width} \times \text{height} \times \text{flow}$;
- Travel segments (`travel = true`) produce zero extrusion volume ($V = 0$) and zero filament consumption ($F = 0$);
- Non-negative length, width, height, and flow imply non-negative volume;
- Linear scaling of length or flow ratio yields proportional volume scaling.
-/

namespace Dry.Semantics.Deposition

/-- Computes segment volume given travel flag, length, width, height, and optional flow ratio. -/
def computeVolume (travel : Bool) (length width height : ℚ) (flow : Option ℚ) : ℚ :=
  if travel then 0
  else
    let f := flow.getD 1
    length * width * height * f

/-- Computes filament consumption given volume and nozzle cross-sectional area. -/
def computeFilament (volume nozzleArea : ℚ) : ℚ :=
  if nozzleArea = 0 then 0
  else volume / nozzleArea

theorem travel_has_zero_volume (length width height : ℚ) (flow : Option ℚ) :
    computeVolume true length width height flow = 0 := by
  rfl

theorem travel_has_zero_filament (length width height nozzleArea : ℚ) (flow : Option ℚ) :
    computeFilament (computeVolume true length width height flow) nozzleArea = 0 := by
  dsimp [computeVolume, computeFilament]
  split_ifs with h
  · rfl
  · exact zero_div nozzleArea

theorem non_negative_volume_of_non_negative_inputs
    (length width height flowVal : ℚ)
    (hL : 0 ≤ length)
    (hW : 0 ≤ width)
    (hH : 0 ≤ height)
    (hF : 0 ≤ flowVal) :
    0 ≤ computeVolume false length width height (some flowVal) := by
  dsimp [computeVolume, Option.getD]
  positivity

theorem length_scaling_scales_volume
    (length width height flowVal scale : ℚ) :
    computeVolume false (length * scale) width height (some flowVal) =
      computeVolume false length width height (some flowVal) * scale := by
  dsimp [computeVolume, Option.getD]
  ring

end Dry.Semantics.Deposition

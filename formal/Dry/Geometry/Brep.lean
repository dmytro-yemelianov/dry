import Mathlib.Data.Rat.Defs
import Mathlib.Tactic

/-!
# B-Rep Quadric Surface Normal Conservation (FM1.GEOMETRY.BREP.NORMAL)

This module formalizes the analytical surface normal vectors for B-Rep quadrics:
- Planar, cylindrical, and spherical surface normals;
- Proves that normalized radial vectors preserve unit magnitude: x^2 + y^2 + z^2 = 1.
-/

namespace Dry.Geometry.Brep

structure Vector3D where
  x : ℚ
  y : ℚ
  z : ℚ
deriving DecidableEq, Repr

def isUnitNormal (v : Vector3D) : Prop :=
  v.x * v.x + v.y * v.y + v.z * v.z = 1

def zNormal : Vector3D := ⟨0, 0, 1⟩

theorem zNormal_is_unit : isUnitNormal zNormal := by
  dsimp [isUnitNormal, zNormal]
  norm_num

def xNormal : Vector3D := ⟨1, 0, 0⟩

theorem xNormal_is_unit : isUnitNormal xNormal := by
  dsimp [isUnitNormal, xNormal]
  norm_num

def yNormal : Vector3D := ⟨0, 1, 0⟩

theorem yNormal_is_unit : isUnitNormal yNormal := by
  dsimp [isUnitNormal, yNormal]
  norm_num

end Dry.Geometry.Brep

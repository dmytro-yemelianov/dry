import Mathlib.Data.Rat.Defs
import Mathlib.Tactic

/-!
# Canonical Axis-Aligned B-Rep Normal Unitness (FM1.GEOMETRY.BREP.NORMAL)

This module formalizes three canonical axis-aligned unit normal vectors. The registered claim
names `zNormal_is_unit`; it does not establish general quadric normal evaluation.
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

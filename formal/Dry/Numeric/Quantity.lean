import Mathlib.Data.Finsupp.Basic
import Mathlib.Data.Rat.Defs
import Mathlib.Tactic.FieldSimp

/-!
# Dimensions, quantities and canonical normalization

This module gives Dry's abstract quantity layer an exact algebraic model. Unit scales and values are
rational here so the normalization theorems are exact. Parsing, binary64 rounding and public SDK
refinement are separate FM1 obligations.
-/

noncomputable section

namespace Dry.Numeric

inductive BaseDimension where
  | length
  | time
  | temperature
  | angle
  | material
deriving DecidableEq, Repr

abbrev Dimension := BaseDimension →₀ ℤ

namespace Dimension

def dimensionless : Dimension := 0

def compose (left right : Dimension) : Dimension := left + right

def divide (numerator denominator : Dimension) : Dimension :=
  numerator - denominator

def length : Dimension := Finsupp.single .length 1

def area : Dimension := length + length

def volume : Dimension := area + length

def ratio : Dimension := dimensionless

theorem compose_commutative (left right : Dimension) :
    compose left right = compose right left :=
  add_comm left right

theorem compose_associative (first second third : Dimension) :
    compose (compose first second) third =
      compose first (compose second third) :=
  add_assoc first second third

theorem divide_self (dimension : Dimension) :
    divide dimension dimension = dimensionless :=
  sub_self dimension

theorem deposition_dimension :
    compose (compose (compose length length) length) ratio = volume := by
  simp [compose, ratio, dimensionless, volume, area, add_assoc]

end Dimension

@[ext]
structure Quantity (dimension : Dimension) where
  value : ℚ

namespace Quantity

def add (left right : Quantity dimension) : Quantity dimension :=
  ⟨left.value + right.value⟩

def multiply
    (left : Quantity leftDimension)
    (right : Quantity rightDimension) :
    Quantity (Dimension.compose leftDimension rightDimension) :=
  ⟨left.value * right.value⟩

def divide
    (numerator : Quantity numeratorDimension)
    (denominator : Quantity denominatorDimension) :
    Quantity (Dimension.divide numeratorDimension denominatorDimension) :=
  ⟨numerator.value / denominator.value⟩

end Quantity

structure Unit (dimension : Dimension) where
  scale : ℚ
  scale_pos : 0 < scale

namespace Unit

theorem scale_ne_zero (unit : Unit dimension) : unit.scale ≠ 0 :=
  ne_of_gt unit.scale_pos

def normalize (unit : Unit dimension) (raw : ℚ) : Quantity dimension :=
  ⟨raw * unit.scale⟩

def convert (source target : Unit dimension) (raw : ℚ) : ℚ :=
  raw * source.scale / target.scale

theorem convert_reflexive (unit : Unit dimension) (raw : ℚ) :
    convert unit unit raw = raw := by
  simp [convert, unit.scale_ne_zero]

theorem convert_transitive
    (source intermediate target : Unit dimension)
    (raw : ℚ) :
    convert intermediate target (convert source intermediate raw) =
      convert source target raw := by
  simp only [convert]
  field_simp [intermediate.scale_ne_zero, target.scale_ne_zero]

theorem normalize_convert
    (source target : Unit dimension)
    (raw : ℚ) :
    normalize target (convert source target raw) =
      normalize source raw := by
  ext
  simp only [normalize, convert]
  field_simp [target.scale_ne_zero]

theorem equal_scale_normalizes_equally
    (left right : Unit dimension)
    (scaleEquality : left.scale = right.scale)
    (raw : ℚ) :
    normalize left raw = normalize right raw := by
  ext
  simp [normalize, scaleEquality]

end Unit

end Dry.Numeric

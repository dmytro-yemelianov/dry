import Mathlib.Data.Rat.Defs

/-!
# Common logical language values

The public Dry IR v0 wire formats carry binary64 values, including bit patterns that may denote
non-finite numbers before validation. The abstract language model uses exact rationals for finite
values and an explicit rejected token for all non-finite inputs. Relating that token and the rational
values to concrete binary64 behavior is a separate numeric-refinement obligation.
-/

namespace Dry.Language

inductive Number where
  | finite (value : ℚ)
  | nonFinite
deriving DecidableEq, Repr

namespace Number

def IsFinite : Number → Prop
  | .finite _ => True
  | .nonFinite => False

def IsPositive : Number → Prop
  | .finite value => 0 < value
  | .nonFinite => False

def IsNonNegative : Number → Prop
  | .finite value => 0 ≤ value
  | .nonFinite => False

def IsUnitInterval : Number → Prop
  | .finite value => 0 ≤ value ∧ value ≤ 1
  | .nonFinite => False

instance (number : Number) : Decidable number.IsFinite := by
  cases number <;> unfold IsFinite <;> infer_instance

instance (number : Number) : Decidable number.IsPositive := by
  cases number <;> unfold IsPositive <;> infer_instance

instance (number : Number) : Decidable number.IsNonNegative := by
  cases number <;> unfold IsNonNegative <;> infer_instance

instance (number : Number) : Decidable number.IsUnitInterval := by
  cases number <;> unfold IsUnitInterval <;> infer_instance

end Number

namespace Optional

def All {α : Type u} (predicate : α → Prop) : Option α → Prop
  | none => True
  | some value => predicate value

instance {α : Type u} (predicate : α → Prop)
    [∀ value, Decidable (predicate value)] (value : Option α) :
    Decidable (All predicate value) := by
  cases value <;> unfold All <;> infer_instance

end Optional

namespace LogicalList

def All {α : Type u} (predicate : α → Prop) : List α → Prop
  | [] => True
  | value :: rest => predicate value ∧ All predicate rest

def decideAll {α : Type u} (predicate : α → Prop)
    [∀ value, Decidable (predicate value)] :
    (values : List α) → Decidable (All predicate values)
  | [] => isTrue trivial
  | value :: rest =>
      match (inferInstance : Decidable (predicate value)), decideAll predicate rest with
      | isTrue valueValid, isTrue restValid => isTrue ⟨valueValid, restValid⟩
      | isFalse valueInvalid, _ => isFalse (fun validity => valueInvalid validity.1)
      | _, isFalse restInvalid => isFalse (fun validity => restInvalid validity.2)

instance {α : Type u} (predicate : α → Prop)
    [∀ value, Decidable (predicate value)] (values : List α) :
    Decidable (All predicate values) :=
  decideAll predicate values

end LogicalList

structure PartialVec3 where
  x : Option Number
  y : Option Number
  z : Option Number
deriving DecidableEq, Repr

namespace PartialVec3

def AllFinite (vector : PartialVec3) : Prop :=
  Optional.All Number.IsFinite vector.x ∧
    Optional.All Number.IsFinite vector.y ∧
    Optional.All Number.IsFinite vector.z

instance (vector : PartialVec3) : Decidable vector.AllFinite :=
  by
    unfold AllFinite
    infer_instance

end PartialVec3

structure Vec2 where
  x : Number
  y : Number
deriving DecidableEq, Repr

namespace Vec2

def AllFinite (vector : Vec2) : Prop :=
  vector.x.IsFinite ∧ vector.y.IsFinite

instance (vector : Vec2) : Decidable vector.AllFinite :=
  by
    unfold AllFinite
    infer_instance

end Vec2

structure Vec3 where
  x : Number
  y : Number
  z : Number
deriving DecidableEq, Repr

namespace Vec3

def AllFinite (vector : Vec3) : Prop :=
  vector.x.IsFinite ∧ vector.y.IsFinite ∧ vector.z.IsFinite

def IsUnit : Vec3 → Prop
  | ⟨.finite x, .finite y, .finite z⟩ => x * x + y * y + z * z = 1
  | _ => False

instance (vector : Vec3) : Decidable vector.AllFinite :=
  by
    unfold AllFinite
    infer_instance

instance (vector : Vec3) : Decidable vector.IsUnit := by
  cases vector with
  | mk x y z =>
      cases x <;> cases y <;> cases z <;> unfold IsUnit <;> infer_instance

end Vec3

end Dry.Language

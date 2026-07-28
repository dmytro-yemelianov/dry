import Dry.Language.WellFormed

/-!
# Serializer-neutral L2 logical equality

The logical L2 syntax has already applied wire defaults and converted field omission into explicit
options. Equality at this boundary therefore compares the normalized schema version, metadata and
ordered segment stream, independently of JSON key order, omission spelling or binary layout.

Binary64 bit equality is deliberately not defined here. It belongs to the codec and numeric-refinement
layers.
-/

namespace Dry.Language.L2.LogicalEquality

def Equivalent (left right : Toolpath) : Prop :=
  left.version = right.version ∧
    left.metadata = right.metadata ∧
    left.segments = right.segments

theorem equivalent_iff_eq (left right : Toolpath) :
    Equivalent left right ↔ left = right := by
  cases left
  cases right
  simp [Equivalent]

theorem reflexive (toolpath : Toolpath) :
    Equivalent toolpath toolpath := by
  simp [Equivalent]

theorem symmetric
    (equivalent : Equivalent left right) :
    Equivalent right left := by
  rw [equivalent_iff_eq] at equivalent ⊢
  exact equivalent.symm

theorem transitive
    (leftMiddle : Equivalent left middle)
    (middleRight : Equivalent middle right) :
    Equivalent left right := by
  rw [equivalent_iff_eq] at leftMiddle middleRight ⊢
  exact leftMiddle.trans middleRight

theorem wellFormed_congr
    (equivalent : Equivalent left right) :
    left.WellFormed ↔ right.WellFormed := by
  rw [equivalent_iff_eq] at equivalent
  subst right
  rfl

end Dry.Language.L2.LogicalEquality

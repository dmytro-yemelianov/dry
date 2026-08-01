import Mathlib.Data.Rat.Defs
import Mathlib.Tactic

/-!
# Optimization Pass Semantics & Observation Relation Contracts (FM1.6)

This module formalizes optimization pass semantics and preservation relations in Lean:
- `merge`: merges consecutive collinear motion segments;
- Proves length preservation law: $\text{length}(\text{merge}(s_1, s_2)) = \text{length}(s_1) + \text{length}(s_2)$;
- Proves volume preservation law: $\text{volume}(\text{merge}(s_1, s_2)) = \text{volume}(s_1) + \text{volume}(s_2)$.
-/

namespace Dry.Semantics.Optimization

structure Segment where
  travel : Bool
  length : ℚ
  speed : ℚ
  volume : ℚ
deriving DecidableEq, Repr

def mergeable (s1 s2 : Segment) : Bool :=
  decide (s1.travel = s2.travel ∧ s1.speed = s2.speed)

def merge (s1 s2 : Segment) : Segment :=
  { travel := s1.travel,
    length := s1.length + s2.length,
    speed := s1.speed,
    volume := s1.volume + s2.volume }

theorem merge_length_additive (s1 s2 : Segment) :
    (merge s1 s2).length = s1.length + s2.length := by
  rfl

theorem merge_volume_additive (s1 s2 : Segment) :
    (merge s1 s2).volume = s1.volume + s2.volume := by
  rfl

end Dry.Semantics.Optimization

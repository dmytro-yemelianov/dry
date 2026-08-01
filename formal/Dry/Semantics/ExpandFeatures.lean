import Dry.Geometry.PlanarTransform
import Mathlib.Data.List.Range
import Mathlib.Tactic

/-!
# Bounded planar feature expansion

This module models the structural semantics of the current `Feature`, ordered `Group` and `Repeat`
nodes. The structural laws are generic over a transform algebra and operation action. A separate
geometric instantiation connects the action to Dry's exact-real planar transform model.

Names, source locations, partial-axis inheritance, manual-code rejection, binary64 trigonometry and
the Rust implementation remain separate refinement obligations.
-/

namespace Dry.Semantics.ExpandFeatures

universe u v

structure Algebra (Transform : Type u) (Op : Type v) where
  identity : Transform
  compose : Transform → Transform → Transform
  apply : Transform → Op → Op

inductive Node (Transform : Type u) (Op : Type v) where
  | feature (pose : Transform) (ops : List Op)
  | group (children : List (Node Transform Op))
  | repeat (count : Nat) (step : Transform) (child : Node Transform Op)

structure Program (Transform : Type u) (Op : Type v) where
  features : List (Node Transform Op)

def power (algebra : Algebra Transform Op) (step : Transform) : Nat → Transform
  | 0 => algebra.identity
  | count + 1 => algebra.compose (power algebra step count) step

mutual

  def expandNode
      (algebra : Algebra Transform Op)
      (parent : Transform) :
      Node Transform Op → List Op
    | .feature pose ops =>
        ops.map (algebra.apply (algebra.compose parent pose))
    | .group children =>
        expandNodes algebra parent children
    | .repeat count step child =>
        (List.range count).flatMap fun index =>
          expandNode algebra
            (algebra.compose parent (power algebra step index))
            child

  def expandNodes
      (algebra : Algebra Transform Op)
      (parent : Transform) :
      List (Node Transform Op) → List Op
    | [] => []
    | node :: rest =>
        expandNode algebra parent node ++
          expandNodes algebra parent rest

end

def expand
    (algebra : Algebra Transform Op)
    (program : Program Transform Op) : List Op :=
  expandNodes algebra algebra.identity program.features

mutual

  def opCount : Node Transform Op → Nat
    | .feature _ ops => ops.length
    | .group children => nodesOpCount children
    | .repeat count _ child => count * opCount child

  def nodesOpCount : List (Node Transform Op) → Nat
    | [] => 0
    | node :: rest => opCount node + nodesOpCount rest

end

mutual

  def nodeCount : Node Transform Op → Nat
    | .feature _ _ => 1
    | .group children => 1 + nodesNodeCount children
    | .repeat count _ child => 1 + count * nodeCount child

  def nodesNodeCount : List (Node Transform Op) → Nat
    | [] => 0
    | node :: rest => nodeCount node + nodesNodeCount rest

end

mutual

  def treeSize : Node Transform Op → Nat
    | .feature _ _ => 1
    | .group children => 1 + nodesTreeSize children
    | .repeat _ _ child => 1 + treeSize child

  def nodesTreeSize : List (Node Transform Op) → Nat
    | [] => 1
    | node :: rest => 1 + treeSize node + nodesTreeSize rest

end

def maxDepth : Node Transform Op → Nat
  | .feature _ _ => 0
  | .group [] => 0
  | .group children => 1 + (children.map maxDepth).foldl Nat.max 0
  | .repeat 0 _ _ => 0
  | .repeat (_ + 1) _ child => 1 + maxDepth child

structure Limits where
  maxOps : Nat
  maxNodes : Nat
  maxDepth : Nat

def WithinLimits
    (limits : Limits)
    (node : Node Transform Op) : Prop :=
  opCount node ≤ limits.maxOps ∧
    nodeCount node ≤ limits.maxNodes ∧
    maxDepth node ≤ limits.maxDepth

theorem expandNodes_append
    (algebra : Algebra Transform Op)
    (parent : Transform)
    (left right : List (Node Transform Op)) :
    expandNodes algebra parent (left ++ right) =
      expandNodes algebra parent left ++
        expandNodes algebra parent right := by
  induction left with
  | nil => rfl
  | cons node rest inductionHypothesis =>
      simp [expandNodes, inductionHypothesis, List.append_assoc]

theorem expand_group_append
    (algebra : Algebra Transform Op)
    (parent : Transform)
    (left right : List (Node Transform Op)) :
    expandNode algebra parent (.group (left ++ right)) =
      expandNode algebra parent (.group left) ++
        expandNode algebra parent (.group right) := by
  exact expandNodes_append algebra parent left right

theorem expand_repeat_zero
    (algebra : Algebra Transform Op)
    (parent step : Transform)
    (child : Node Transform Op) :
    expandNode algebra parent (.repeat 0 step child) = [] := by
  rfl

theorem expand_repeat_succ
    (algebra : Algebra Transform Op)
    (parent step : Transform)
    (child : Node Transform Op)
    (count : Nat) :
    expandNode algebra parent (.repeat (count + 1) step child) =
      expandNode algebra parent (.repeat count step child) ++
        expandNode algebra
          (algebra.compose parent (power algebra step count))
          child := by
  simp [expandNode, List.range_succ, List.flatMap_append]

mutual

  theorem length_expandNode
      (algebra : Algebra Transform Op)
      (parent : Transform)
      (node : Node Transform Op) :
      (expandNode algebra parent node).length = opCount node := by
    cases node with
    | feature pose ops =>
        simp [expandNode, opCount]
    | group children =>
        exact length_expandNodes algebra parent children
    | «repeat» count step child =>
        simp only [expandNode, opCount, List.length_flatMap]
        have childLength (index : Nat) :
            (expandNode algebra
              (algebra.compose parent (power algebra step index))
              child).length =
              opCount child :=
          length_expandNode algebra
            (algebra.compose parent (power algebra step index))
            child
        simp_rw [childLength]
        simp
  termination_by treeSize node
  decreasing_by
    all_goals simp_all [treeSize]

  theorem length_expandNodes
      (algebra : Algebra Transform Op)
      (parent : Transform)
      (nodes : List (Node Transform Op)) :
      (expandNodes algebra parent nodes).length = nodesOpCount nodes := by
    cases nodes with
    | nil => rfl
    | cons node rest =>
        simp only [expandNodes, nodesOpCount, List.length_append]
        rw [length_expandNode algebra parent node]
        rw [length_expandNodes algebra parent rest]
  termination_by nodesTreeSize nodes
  decreasing_by
    all_goals simp_all [nodesTreeSize]
    all_goals omega

end

theorem expansion_respects_op_budget
    (algebra : Algebra Transform Op)
    (parent : Transform)
    (node : Node Transform Op)
    (limits : Limits)
    (within : WithinLimits limits node) :
    (expandNode algebra parent node).length ≤ limits.maxOps := by
  rw [length_expandNode]
  exact within.1

theorem repeat_op_count
    (algebra : Algebra Transform Op)
    (parent step : Transform)
    (count : Nat)
    (child : Node Transform Op) :
    (expandNode algebra parent (.repeat count step child)).length =
      count * opCount child := by
  rw [length_expandNode]
  rfl

theorem expansion_respects_budgets
    (algebra : Algebra Transform Op)
    (parent : Transform)
    (node : Node Transform Op)
    (limits : Limits)
    (within : WithinLimits limits node) :
    (expandNode algebra parent node).length ≤ limits.maxOps ∧
      nodeCount node ≤ limits.maxNodes ∧
      maxDepth node ≤ limits.maxDepth := by
  rw [length_expandNode]
  exact within

inductive Evaluates
    (algebra : Algebra Transform Op) :
    Program Transform Op → List Op → Prop where
  | result (program : Program Transform Op) :
      Evaluates algebra program (expand algebra program)

namespace Evaluates

theorem deterministic
    (left : Evaluates algebra program first)
    (right : Evaluates algebra program second) :
    first = second := by
  cases left
  cases right
  rfl

end Evaluates

namespace Geometry

open Dry.Geometry.PlanarTransform

inductive Op where
  | point (tag : Nat) (value : Vec3)
  | vector (tag : Nat) (value : Vec3)
  | invariant (tag : Nat)

def applyOp (transform : Transform) : Op → Op
  | .point tag point => .point tag (applyPoint transform point)
  | .vector tag vector => .vector tag (applyVector transform vector)
  | .invariant tag => .invariant tag

def algebra : Algebra Transform Op :=
  {
    identity
    compose
    apply := applyOp
  }

theorem applyVector_compose
    (outer inner : Transform)
    (vector : Vec3) :
    applyVector (compose outer inner) vector =
      applyVector outer (applyVector inner vector) := by
  ext <;> simp [applyVector, compose] <;> ring

theorem applyOp_compose
    (outer inner : Transform)
    (op : Op) :
    applyOp (compose outer inner) op =
      applyOp outer (applyOp inner op) := by
  cases op with
  | point tag point =>
      simp [applyOp, apply_compose]
  | vector tag vector =>
      simp [applyOp, applyVector_compose]
  | invariant tag =>
      rfl

theorem feature_composition_action
    (outer localTransform : Transform)
    (ops : List Op) :
    expandNode algebra outer (.feature localTransform ops) =
      (ops.map (applyOp localTransform)).map (applyOp outer) := by
  simp [expandNode, algebra, applyOp_compose, List.map_map]

end Geometry

end Dry.Semantics.ExpandFeatures

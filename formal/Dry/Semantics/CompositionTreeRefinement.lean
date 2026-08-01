import Dry.Numeric.CompositionTree
import Dry.Semantics.ExpandFeatures
import Mathlib.Tactic

/-!
# Feature-path refinement to parenthesized composition trees

This module records the transform-expression shape used while Rust descends through nested
`Feature` and `Repeat` nodes. A group leaves the current parent unchanged; a feature appends its pose
on the right; repeat instance `i` appends the left-associated `i`-step accumulator on the right.

The construction deliberately preserves every binary composition node. It does not identify
different parenthesizations and therefore does not assume binary64 associativity. The final theorem
only transports the conditional numeric result from `Dry.Numeric.CompositionTree`; native Rust
rounding, libm and range-premise refinement remain separate obligations.
-/

namespace Dry.Semantics.CompositionTreeRefinement

open Dry.Geometry.PlanarTransform
open Dry.Numeric.RoundModel
open Dry.Numeric.Binary64
open Dry.Numeric.Trig
open Dry.Numeric.Accumulation
open Dry.Numeric.CompositionTree
open Dry.Semantics.ExpandFeatures

noncomputable section

structure Pose where
  degrees : ℝ
  translation : Vec3

def Pose.tree (pose : Pose) : TransformTree :=
  .pose pose.degrees pose.translation

def Pose.exact (pose : Pose) : Transform :=
  exactPose pose.degrees pose.translation

def Pose.binary64
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (pose : Pose) : Transform :=
  binary64Pose roundContract libmContract pose.degrees pose.translation

/-- The exact syntax tree of Rust's `instance = instance.compose(step)` recurrence. -/
def repeatInstanceTree (step : Pose) : ℕ → TransformTree
  | 0 => .identity
  | index + 1 => .compose (repeatInstanceTree step index) step.tree

inductive PathStep where
  /-- Rust's `parent.compose(local)` at a terminal feature. -/
  | feature (pose : Pose)
  /-- Rust's `parent.compose(instance)` for a selected repeat instance. -/
  | repeatInstance (step : Pose) (index : ℕ)

def PathStep.extend (parent : TransformTree) : PathStep → TransformTree
  | .feature pose => .compose parent pose.tree
  | .repeatInstance step index =>
      .compose parent (repeatInstanceTree step index)

/--
The transform tree at one selected feature occurrence. Ordered groups contribute no path step,
matching the fact that Rust passes their parent transform through unchanged.
-/
def pathTree (steps : List PathStep) : TransformTree :=
  steps.foldl PathStep.extend .identity

/--
An executable instantiation of the existing abstract feature expander that records the accumulated
transform tree at each marker operation.
-/
def treeAlgebra : Algebra TransformTree TransformTree where
  identity := .identity
  compose := .compose
  apply := fun transform _ => transform

@[simp]
theorem repeatInstanceTree_zero (step : Pose) :
    repeatInstanceTree step 0 = .identity :=
  rfl

@[simp]
theorem repeatInstanceTree_succ (step : Pose) (index : ℕ) :
    repeatInstanceTree step (index + 1) =
      .compose (repeatInstanceTree step index) step.tree :=
  rfl

@[simp]
theorem repeatInstanceTree_composeCount (step : Pose) (index : ℕ) :
    (repeatInstanceTree step index).composeCount = index := by
  induction index with
  | zero => rfl
  | succ index ih =>
      simp [repeatInstanceTree, Pose.tree, TransformTree.composeCount, ih]

@[simp]
theorem repeatInstanceTree_poseCount (step : Pose) (index : ℕ) :
    (repeatInstanceTree step index).poseCount = index := by
  induction index with
  | zero => rfl
  | succ index ih =>
      simp [repeatInstanceTree, Pose.tree, TransformTree.poseCount, ih]

theorem repeatInstanceTree_exactEval (step : Pose) (index : ℕ) :
    (repeatInstanceTree step index).exactEval =
      exactRepeat step.exact index := by
  induction index with
  | zero => rfl
  | succ index ih =>
      simp only [repeatInstanceTree, TransformTree.exactEval, exactRepeat]
      rw [ih]
      rfl

theorem repeatInstanceTree_binary64Eval
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (step : Pose)
    (index : ℕ) :
    (repeatInstanceTree step index).binary64Eval roundContract libmContract =
      binary64Repeat roundContract (step.binary64 roundContract libmContract) index := by
  induction index with
  | zero => rfl
  | succ index ih =>
      simp only [repeatInstanceTree, TransformTree.binary64Eval, binary64Repeat]
      rw [ih]
      rfl

theorem power_treeAlgebra_pose (step : Pose) (index : ℕ) :
    power treeAlgebra step.tree index =
      repeatInstanceTree step index := by
  induction index with
  | zero => rfl
  | succ index ih =>
      simpa [power, repeatInstanceTree, treeAlgebra] using
        congrArg (fun prior => TransformTree.compose prior step.tree) ih

@[simp]
theorem pathTree_nil :
    pathTree [] = .identity :=
  rfl

theorem pathTree_append_feature (steps : List PathStep) (pose : Pose) :
    pathTree (steps ++ [.feature pose]) =
      .compose (pathTree steps) pose.tree := by
  simp [pathTree, PathStep.extend]

theorem pathTree_append_repeat
    (steps : List PathStep)
    (step : Pose)
    (index : ℕ) :
    pathTree (steps ++ [.repeatInstance step index]) =
      .compose (pathTree steps) (repeatInstanceTree step index) := by
  simp [pathTree, PathStep.extend]

/--
The two transform-changing Rust descent cases append their local expression on the right of the
already accumulated parent; this is the parenthesization checked by the structural fixtures.
-/
theorem selected_path_parenthesization
    (steps : List PathStep)
    (featurePose repeatPose : Pose)
    (index : ℕ) :
    pathTree (steps ++ [.feature featurePose]) =
        .compose (pathTree steps) featurePose.tree ∧
      pathTree (steps ++ [.repeatInstance repeatPose index]) =
        .compose (pathTree steps) (repeatInstanceTree repeatPose index) :=
  ⟨pathTree_append_feature steps featurePose,
    pathTree_append_repeat steps repeatPose index⟩

/--
The actual generic feature-expansion semantics uses the same parent-first feature composition and
the same right-appended repeat-instance tree.
-/
theorem expandNode_tree_parenthesization
    (parent : TransformTree)
    (featurePose repeatPose : Pose)
    (count : ℕ)
    (child : Node TransformTree TransformTree) :
    expandNode treeAlgebra parent
          (.feature featurePose.tree [.identity]) =
        [.compose parent featurePose.tree] ∧
      expandNode treeAlgebra parent
          (.repeat count repeatPose.tree child) =
        (List.range count).flatMap fun index =>
          expandNode treeAlgebra
            (.compose parent (repeatInstanceTree repeatPose index))
            child := by
  constructor
  · rfl
  · simp only [expandNode]
    congr 1
    funext index
    rw [power_treeAlgebra_pose]
    rfl

/-- Entering an ordered group passes the parent tree to every child unchanged. -/
theorem expandNode_tree_group
    (parent : TransformTree)
    (children : List (Node TransformTree TransformTree)) :
    expandNode treeAlgebra parent (.group children) =
      expandNodes treeAlgebra parent children :=
  rfl

/--
Any selected nested Feature/Repeat path inherits the arbitrary-tree numeric theorem with exactly
the parenthesization constructed above.
-/
theorem pathTree_binary64_error
    (steps : List PathStep)
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (hProfiled : (pathTree steps).Profiled)
    (hRange : (pathTree steps).GraphInRange roundContract libmContract)
    (hCount : (pathTree steps).composeCount ≤ compositionCountLimit) :
    RepeatError
      ((pathTree steps).binary64Eval roundContract libmContract)
      (pathTree steps).exactEval
      repeatCoefficientErrorCeiling
      treeTranslationXYErrorCeiling
      repeatTranslationZErrorCeiling :=
  binary64Tree_error
    (pathTree steps)
    roundContract
    libmContract
    hProfiled
    hRange
    hCount

end

end Dry.Semantics.CompositionTreeRefinement

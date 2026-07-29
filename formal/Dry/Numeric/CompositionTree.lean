import Dry.Numeric.Accumulation
import Mathlib.Analysis.Complex.Exponential
import Mathlib.Tactic

/-!
# Bounded arbitrary planar composition trees

This module preserves the parenthesization of the transform expressions evaluated by nested
`Feature` and `Repeat` nodes. Unlike the same-step recurrence in `Dry.Numeric.Accumulation`, both
operands of a composition may already contain binary64 error.

The theorem is conditional on the existing profile limits and on every concrete composition graph
satisfying the checked exact-operation range predicate. It does not assert associativity of
binary64 composition.
-/

namespace Dry.Numeric.CompositionTree

open Dry.Geometry.PlanarTransform
open Dry.Numeric.RoundModel
open Dry.Numeric.Binary64
open Dry.Numeric.Angle
open Dry.Numeric.Trig
open Dry.Numeric.Accumulation

noncomputable section

inductive TransformTree where
  | identity
  | pose (degrees : ℝ) (translation : Vec3)
  | compose (outer inner : TransformTree)

def TransformTree.composeCount : TransformTree → ℕ
  | .identity => 0
  | .pose _ _ => 0
  | .compose outer inner =>
      outer.composeCount + inner.composeCount + 1

def TransformTree.poseCount : TransformTree → ℕ
  | .identity => 0
  | .pose _ _ => 1
  | .compose outer inner =>
      outer.poseCount + inner.poseCount

def TransformTree.cost (tree : TransformTree) : ℕ :=
  tree.poseCount + tree.composeCount

def TransformTree.Profiled : TransformTree → Prop
  | .identity => True
  | .pose degrees translation =>
      |degrees| ≤ degreeLimit ∧
        |translation.x| ≤ poseTranslationComponentLimit ∧
          |translation.y| ≤ poseTranslationComponentLimit
  | .compose outer inner =>
      outer.Profiled ∧ inner.Profiled

def TransformTree.exactEval : TransformTree → Transform
  | .identity => Dry.Geometry.PlanarTransform.identity
  | .pose degrees translation => exactPose degrees translation
  | .compose outer inner =>
      Dry.Geometry.PlanarTransform.compose outer.exactEval inner.exactEval

def TransformTree.binary64Eval
    (roundContract : RoundContract)
    (libmContract : LibmContract) : TransformTree → Transform
  | .identity => Dry.Geometry.PlanarTransform.identity
  | .pose degrees translation =>
      binary64Pose roundContract libmContract degrees translation
  | .compose outer inner =>
      binary64Compose
        roundContract
        (outer.binary64Eval roundContract libmContract)
        (inner.binary64Eval roundContract libmContract)

def TransformTree.GraphInRange
    (roundContract : RoundContract)
    (libmContract : LibmContract) : TransformTree → Prop
  | .identity => True
  | .pose _ _ => True
  | .compose outer inner =>
      outer.GraphInRange roundContract libmContract ∧
        inner.GraphInRange roundContract libmContract ∧
          ComposeGraphInRange
            roundContract
            (outer.binary64Eval roundContract libmContract)
            (inner.binary64Eval roundContract libmContract)

def treeCoefficientBase : ℝ :=
  1 + compositionCoefficientNormError

def treeCoefficientPotential (cost : ℕ) : ℝ :=
  treeCoefficientBase ^ cost - 1

def treeTranslationQuadratic : ℝ :=
  1 / 2 ^ 6

def treeTranslationXYError (cost : ℕ) : ℝ :=
  treeTranslationQuadratic * (cost : ℝ) ^ 2

def treeTranslationXYErrorCeiling : ℝ :=
  2 ^ 30

def treeCostLimit : ℕ :=
  2 * compositionCountLimit + 1

theorem TransformTree.poseCount_le_composeCount_add_one
    (tree : TransformTree) :
    tree.poseCount ≤ tree.composeCount + 1 := by
  induction tree with
  | identity => simp [TransformTree.poseCount, TransformTree.composeCount]
  | pose => simp [TransformTree.poseCount, TransformTree.composeCount]
  | compose outer inner outerIH innerIH =>
      simp only [TransformTree.poseCount, TransformTree.composeCount]
      omega

theorem TransformTree.cost_le_treeCostLimit
    (tree : TransformTree)
    (hCount : tree.composeCount ≤ compositionCountLimit) :
    tree.cost ≤ treeCostLimit := by
  have hPose := tree.poseCount_le_composeCount_add_one
  simp only [TransformTree.cost, treeCostLimit]
  omega

private theorem treeCoefficientBase_one_le :
    1 ≤ treeCoefficientBase := by
  norm_num [treeCoefficientBase, compositionCoefficientNormError]

private theorem treeCoefficientPotential_nonneg
    (cost : ℕ) :
    0 ≤ treeCoefficientPotential cost := by
  exact sub_nonneg.mpr (one_le_pow₀ treeCoefficientBase_one_le)

private theorem treeCoefficientPotential_compose
    (outerCost innerCost : ℕ) :
    compositionCoefficientNormError +
        (treeCoefficientPotential outerCost *
            (1 + treeCoefficientPotential innerCost) +
          treeCoefficientPotential innerCost) ≤
      treeCoefficientPotential (outerCost + innerCost + 1) := by
  have hPower :
      1 ≤ treeCoefficientBase ^ (outerCost + innerCost) :=
    one_le_pow₀ treeCoefficientBase_one_le
  have hLocalNonneg :
      0 ≤ compositionCoefficientNormError := by
    norm_num [compositionCoefficientNormError]
  have hLocal :
      compositionCoefficientNormError ≤
        treeCoefficientBase ^ (outerCost + innerCost) *
          compositionCoefficientNormError :=
    calc
      compositionCoefficientNormError =
          1 * compositionCoefficientNormError := by ring
      _ ≤
          treeCoefficientBase ^ (outerCost + innerCost) *
            compositionCoefficientNormError :=
        mul_le_mul_of_nonneg_right hPower hLocalNonneg
  calc
    compositionCoefficientNormError +
          (treeCoefficientPotential outerCost *
              (1 + treeCoefficientPotential innerCost) +
            treeCoefficientPotential innerCost)
        =
      compositionCoefficientNormError +
        treeCoefficientPotential (outerCost + innerCost) := by
      simp [treeCoefficientPotential, treeCoefficientBase, pow_add]
      ring
    _ ≤
      treeCoefficientBase ^ (outerCost + innerCost) *
          compositionCoefficientNormError +
        treeCoefficientPotential (outerCost + innerCost) :=
      add_le_add hLocal le_rfl
    _ = treeCoefficientPotential (outerCost + innerCost + 1) := by
      simp [treeCoefficientPotential, treeCoefficientBase, pow_succ]
      ring

theorem TransformTree.exact_coefficient_norm
    (tree : TransformTree) :
    ‖coefficient tree.exactEval‖ = 1 := by
  induction tree with
  | identity => simp [TransformTree.exactEval]
  | pose degrees translation =>
      exact exactPose_coefficient_norm degrees translation
  | compose outer inner outerIH innerIH =>
      rw [TransformTree.exactEval, coefficient_compose, norm_mul, outerIH,
        innerIH, one_mul]

theorem TransformTree.exact_translationXY_norm_le
    (tree : TransformTree)
    (hProfiled : tree.Profiled) :
    ‖translationXY tree.exactEval‖ ≤
      (tree.poseCount : ℝ) * poseTranslationXYNormLimit := by
  induction tree with
  | identity =>
      simp [TransformTree.exactEval, TransformTree.poseCount]
  | pose degrees translation =>
      simpa [TransformTree.exactEval, TransformTree.poseCount] using
        exactPose_translationXY_norm_le
          degrees translation hProfiled.2.1 hProfiled.2.2
  | compose outer inner outerIH innerIH =>
      have hOuter := outerIH hProfiled.1
      have hInner := innerIH hProfiled.2
      rw [TransformTree.exactEval, translationXY_compose]
      calc
        ‖coefficient outer.exactEval * translationXY inner.exactEval +
            translationXY outer.exactEval‖
            ≤
          ‖coefficient outer.exactEval * translationXY inner.exactEval‖ +
            ‖translationXY outer.exactEval‖ :=
          norm_add_le _ _
        _ =
          ‖translationXY inner.exactEval‖ +
            ‖translationXY outer.exactEval‖ := by
          rw [norm_mul, outer.exact_coefficient_norm, one_mul]
        _ ≤
          (inner.poseCount : ℝ) * poseTranslationXYNormLimit +
            (outer.poseCount : ℝ) * poseTranslationXYNormLimit :=
          add_le_add hInner hOuter
        _ =
          ((TransformTree.compose outer inner).poseCount : ℝ) *
            poseTranslationXYNormLimit := by
          simp [TransformTree.poseCount]
          ring

private theorem coefficient_compose_sensitivity
    {actualOuter exactOuter actualInner exactInner : ℂ}
    (hExactOuter : ‖exactOuter‖ = 1)
    (hExactInner : ‖exactInner‖ = 1) :
    ‖actualOuter * actualInner - exactOuter * exactInner‖ ≤
      ‖actualOuter - exactOuter‖ *
          (1 + ‖actualInner - exactInner‖) +
        ‖actualInner - exactInner‖ := by
  have hActualInner :
      ‖actualInner‖ ≤ 1 + ‖actualInner - exactInner‖ := by
    calc
      ‖actualInner‖ =
          ‖(actualInner - exactInner) + exactInner‖ := by ring_nf
      _ ≤ ‖actualInner - exactInner‖ + ‖exactInner‖ :=
        norm_add_le _ _
      _ = 1 + ‖actualInner - exactInner‖ := by
        rw [hExactInner]
        ring
  rw [show
    actualOuter * actualInner - exactOuter * exactInner =
      (actualOuter - exactOuter) * actualInner +
        exactOuter * (actualInner - exactInner) by ring]
  calc
    ‖(actualOuter - exactOuter) * actualInner +
        exactOuter * (actualInner - exactInner)‖
        ≤
      ‖(actualOuter - exactOuter) * actualInner‖ +
        ‖exactOuter * (actualInner - exactInner)‖ :=
      norm_add_le _ _
    _ =
      ‖actualOuter - exactOuter‖ * ‖actualInner‖ +
        ‖exactOuter‖ * ‖actualInner - exactInner‖ := by
      rw [norm_mul, norm_mul]
    _ ≤
      ‖actualOuter - exactOuter‖ *
          (1 + ‖actualInner - exactInner‖) +
        1 * ‖actualInner - exactInner‖ := by
      exact add_le_add
        (mul_le_mul_of_nonneg_left hActualInner (norm_nonneg _))
        (mul_le_mul_of_nonneg_right hExactOuter.le (norm_nonneg _))
    _ = _ := by ring

theorem TransformTree.coefficient_error
    (tree : TransformTree)
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (hProfiled : tree.Profiled)
    (hRange : tree.GraphInRange roundContract libmContract) :
    ‖coefficient (tree.binary64Eval roundContract libmContract) -
        coefficient tree.exactEval‖ ≤
      treeCoefficientPotential tree.cost := by
  induction tree with
  | identity =>
      simp [TransformTree.binary64Eval, TransformTree.exactEval,
        TransformTree.cost, TransformTree.poseCount,
        TransformTree.composeCount, treeCoefficientPotential]
  | pose degrees translation =>
      have h :=
        binary64Pose_coefficient_error
          roundContract
          libmContract
          degrees
          translation
          hProfiled.1
      exact h.trans (by
        norm_num [
          TransformTree.cost,
          TransformTree.poseCount,
          TransformTree.composeCount,
          treeCoefficientPotential,
          treeCoefficientBase,
          stepCoefficientNormError,
          compositionCoefficientNormError
        ])
  | compose outer inner outerIH innerIH =>
      have hOuter := outerIH hProfiled.1 hRange.1
      have hInner := innerIH hProfiled.2 hRange.2.1
      have hLocal :=
        binary64Compose_coefficient_local_error
          roundContract
          (outer.binary64Eval roundContract libmContract)
          (inner.binary64Eval roundContract libmContract)
          hRange.2.2
      have hSensitivity :=
        coefficient_compose_sensitivity
          (actualOuter :=
            coefficient (outer.binary64Eval roundContract libmContract))
          (exactOuter := coefficient outer.exactEval)
          (actualInner :=
            coefficient (inner.binary64Eval roundContract libmContract))
          (exactInner := coefficient inner.exactEval)
          outer.exact_coefficient_norm
          inner.exact_coefficient_norm
      calc
        ‖coefficient
              ((TransformTree.compose outer inner).binary64Eval
                roundContract libmContract) -
            coefficient (TransformTree.compose outer inner).exactEval‖
            ≤
          ‖coefficient
                (binary64Compose
                  roundContract
                  (outer.binary64Eval roundContract libmContract)
                  (inner.binary64Eval roundContract libmContract)) -
              coefficient
                (Dry.Geometry.PlanarTransform.compose
                  (outer.binary64Eval roundContract libmContract)
                  (inner.binary64Eval roundContract libmContract))‖ +
            ‖coefficient
                (Dry.Geometry.PlanarTransform.compose
                  (outer.binary64Eval roundContract libmContract)
                  (inner.binary64Eval roundContract libmContract)) -
              coefficient
                  (Dry.Geometry.PlanarTransform.compose
                    outer.exactEval inner.exactEval)‖ := by
          simp only [TransformTree.binary64Eval, TransformTree.exactEval]
          rw [show
            coefficient
                  (binary64Compose
                    roundContract
                    (outer.binary64Eval roundContract libmContract)
                    (inner.binary64Eval roundContract libmContract)) -
                coefficient
                  (Dry.Geometry.PlanarTransform.compose
                    outer.exactEval inner.exactEval) =
              (coefficient
                  (binary64Compose
                    roundContract
                    (outer.binary64Eval roundContract libmContract)
                    (inner.binary64Eval roundContract libmContract)) -
                coefficient
                  (Dry.Geometry.PlanarTransform.compose
                    (outer.binary64Eval roundContract libmContract)
                    (inner.binary64Eval roundContract libmContract))) +
              (coefficient
                  (Dry.Geometry.PlanarTransform.compose
                    (outer.binary64Eval roundContract libmContract)
                    (inner.binary64Eval roundContract libmContract)) -
                coefficient
                  (Dry.Geometry.PlanarTransform.compose
                    outer.exactEval inner.exactEval)) by
            ring]
          exact norm_add_le _ _
        _ ≤
          compositionCoefficientNormError +
            (treeCoefficientPotential outer.cost *
                (1 + treeCoefficientPotential inner.cost) +
              treeCoefficientPotential inner.cost) := by
          apply add_le_add hLocal
          rw [coefficient_compose, coefficient_compose]
          have hOneInner :
              1 +
                  ‖coefficient
                      (inner.binary64Eval roundContract libmContract) -
                    coefficient inner.exactEval‖ ≤
                1 + treeCoefficientPotential inner.cost :=
            add_le_add le_rfl hInner
          have hProduct :
              ‖coefficient
                    (outer.binary64Eval roundContract libmContract) -
                  coefficient outer.exactEval‖ *
                    (1 +
                      ‖coefficient
                          (inner.binary64Eval roundContract libmContract) -
                        coefficient inner.exactEval‖) ≤
                treeCoefficientPotential outer.cost *
                  (1 + treeCoefficientPotential inner.cost) :=
            mul_le_mul
              hOuter
              hOneInner
              (add_nonneg zero_le_one (norm_nonneg _))
              (treeCoefficientPotential_nonneg outer.cost)
          exact hSensitivity.trans (add_le_add hProduct hInner)
        _ ≤
          treeCoefficientPotential
            ((TransformTree.compose outer inner).cost) := by
          rw [show
            (TransformTree.compose outer inner).cost =
              outer.cost + inner.cost + 1 by
            simp [TransformTree.cost, TransformTree.poseCount,
              TransformTree.composeCount]
            omega]
          exact treeCoefficientPotential_compose outer.cost inner.cost

private theorem treeCoefficientPotential_le_exp
    (cost : ℕ) :
    treeCoefficientPotential cost ≤
      Real.exp ((cost : ℝ) * compositionCoefficientNormError) - 1 := by
  have hBase :
      treeCoefficientBase ≤
        Real.exp compositionCoefficientNormError := by
    simpa [treeCoefficientBase, add_comm] using
      Real.add_one_le_exp compositionCoefficientNormError
  have hPower :=
    pow_le_pow_left₀
      (by norm_num [treeCoefficientBase, compositionCoefficientNormError])
      hBase
      cost
  calc
    treeCoefficientPotential cost =
        treeCoefficientBase ^ cost - 1 := rfl
    _ ≤
        (Real.exp compositionCoefficientNormError) ^ cost - 1 :=
      sub_le_sub_right hPower 1
    _ =
        Real.exp ((cost : ℝ) * compositionCoefficientNormError) - 1 := by
      rw [Real.exp_nat_mul]

private theorem treeCoefficientPotential_le_linear
    {cost : ℕ}
    (hCost : cost ≤ treeCostLimit) :
    treeCoefficientPotential cost ≤
      2 * (cost : ℝ) * compositionCoefficientNormError := by
  let x : ℝ :=
    (cost : ℝ) * compositionCoefficientNormError
  have hCostReal : (cost : ℝ) ≤ treeCostLimit := by
    exact_mod_cast hCost
  have hx : 0 ≤ x := by
    dsimp [x]
    exact mul_nonneg (Nat.cast_nonneg _) (by
      norm_num [compositionCoefficientNormError])
  have hxHalf : x ≤ 1 / 2 := by
    dsimp [x]
    calc
      (cost : ℝ) * compositionCoefficientNormError
          ≤ treeCostLimit * compositionCoefficientNormError :=
        mul_le_mul_of_nonneg_right hCostReal (by
          norm_num [compositionCoefficientNormError])
      _ ≤ 1 / 2 := by
        norm_num [
          treeCostLimit,
          compositionCountLimit,
          compositionCoefficientNormError
        ]
  have hxOne : x < 1 := hxHalf.trans_lt (by norm_num)
  have hExp :=
    Real.exp_bound_div_one_sub_of_interval hx hxOne
  have hDen : 0 < 1 - x := sub_pos.mpr hxOne
  have hInv :
      1 / (1 - x) ≤ 1 + 2 * x := by
    apply (div_le_iff₀ hDen).2
    have hProduct :
        0 ≤ x * (1 - 2 * x) :=
      mul_nonneg hx (by linarith)
    nlinarith
  calc
    treeCoefficientPotential cost
        ≤ Real.exp x - 1 := by
      simpa [x] using treeCoefficientPotential_le_exp cost
    _ ≤ 1 / (1 - x) - 1 :=
      sub_le_sub_right hExp 1
    _ ≤ (1 + 2 * x) - 1 :=
      sub_le_sub_right hInv 1
    _ = 2 * (cost : ℝ) * compositionCoefficientNormError := by
      simp [x]
      ring

private theorem treeCoefficientPotential_le_ceiling
    {cost : ℕ}
    (hCost : cost ≤ treeCostLimit) :
    treeCoefficientPotential cost ≤ repeatCoefficientErrorCeiling := by
  let x : ℝ :=
    (cost : ℝ) * compositionCoefficientNormError
  have hCostReal : (cost : ℝ) ≤ treeCostLimit := by
    exact_mod_cast hCost
  have hx : 0 ≤ x := by
    dsimp [x]
    exact mul_nonneg (Nat.cast_nonneg _) (by
      norm_num [compositionCoefficientNormError])
  have hxMax :
      x ≤ treeCostLimit * compositionCoefficientNormError := by
    dsimp [x]
    exact mul_le_mul_of_nonneg_right hCostReal (by
      norm_num [compositionCoefficientNormError])
  have hxOne : x < 1 := by
    calc
      x ≤ treeCostLimit * compositionCoefficientNormError := hxMax
      _ < 1 := by
        norm_num [
          treeCostLimit,
          compositionCountLimit,
          compositionCoefficientNormError
        ]
  have hExp :=
    Real.exp_bound_div_one_sub_of_interval hx hxOne
  have hDen : 0 < 1 - x := sub_pos.mpr hxOne
  have hInv :
      1 / (1 - x) ≤ 1 + repeatCoefficientErrorCeiling := by
    apply (div_le_iff₀ hDen).2
    have hNumeric :
        treeCostLimit * compositionCoefficientNormError *
            (1 + repeatCoefficientErrorCeiling) ≤
          repeatCoefficientErrorCeiling := by
      norm_num [
        treeCostLimit,
        compositionCountLimit,
        compositionCoefficientNormError,
        repeatCoefficientErrorCeiling
      ]
    have hScaled :
        x * (1 + repeatCoefficientErrorCeiling) ≤
          repeatCoefficientErrorCeiling :=
      (mul_le_mul_of_nonneg_right hxMax (by
        norm_num [repeatCoefficientErrorCeiling])).trans hNumeric
    nlinarith
  calc
    treeCoefficientPotential cost
        ≤ Real.exp x - 1 := by
      simpa [x] using treeCoefficientPotential_le_exp cost
    _ ≤ 1 / (1 - x) - 1 :=
      sub_le_sub_right hExp 1
    _ ≤ repeatCoefficientErrorCeiling := by
      linarith

private theorem translation_compose_sensitivity
    {actualOuter exactOuter actualInner exactInner : Transform}
    (hExactOuter : ‖coefficient exactOuter‖ = 1) :
    ‖translationXY
          (Dry.Geometry.PlanarTransform.compose actualOuter actualInner) -
        translationXY
          (Dry.Geometry.PlanarTransform.compose exactOuter exactInner)‖ ≤
      ‖translationXY actualOuter - translationXY exactOuter‖ +
        ‖translationXY actualInner - translationXY exactInner‖ +
          ‖coefficient actualOuter - coefficient exactOuter‖ *
            (‖translationXY exactInner‖ +
              ‖translationXY actualInner - translationXY exactInner‖) := by
  have hActualInner :
      ‖translationXY actualInner‖ ≤
        ‖translationXY exactInner‖ +
          ‖translationXY actualInner - translationXY exactInner‖ := by
    calc
      ‖translationXY actualInner‖ =
          ‖translationXY exactInner +
            (translationXY actualInner - translationXY exactInner)‖ := by
        ring_nf
      _ ≤
          ‖translationXY exactInner‖ +
            ‖translationXY actualInner - translationXY exactInner‖ :=
        norm_add_le _ _
  rw [translationXY_compose, translationXY_compose]
  rw [show
    (coefficient actualOuter * translationXY actualInner +
          translationXY actualOuter) -
        (coefficient exactOuter * translationXY exactInner +
          translationXY exactOuter) =
      (translationXY actualOuter - translationXY exactOuter) +
        coefficient exactOuter *
          (translationXY actualInner - translationXY exactInner) +
        (coefficient actualOuter - coefficient exactOuter) *
          translationXY actualInner by ring]
  calc
    ‖(translationXY actualOuter - translationXY exactOuter) +
          coefficient exactOuter *
            (translationXY actualInner - translationXY exactInner) +
          (coefficient actualOuter - coefficient exactOuter) *
            translationXY actualInner‖
        ≤
      ‖translationXY actualOuter - translationXY exactOuter‖ +
          ‖coefficient exactOuter *
            (translationXY actualInner - translationXY exactInner)‖ +
        ‖(coefficient actualOuter - coefficient exactOuter) *
          translationXY actualInner‖ := by
      exact (norm_add_le _ _).trans
        (add_le_add (norm_add_le _ _) le_rfl)
    _ =
      ‖translationXY actualOuter - translationXY exactOuter‖ +
          ‖translationXY actualInner - translationXY exactInner‖ +
        ‖coefficient actualOuter - coefficient exactOuter‖ *
          ‖translationXY actualInner‖ := by
      rw [norm_mul, norm_mul, hExactOuter, one_mul]
    _ ≤ _ := by
      exact add_le_add le_rfl
        (mul_le_mul_of_nonneg_left hActualInner (norm_nonneg _))

private theorem treeTranslationError_compose
    {outerCost innerCost : ℕ}
    (hCost : outerCost + innerCost + 1 ≤ treeCostLimit) :
    treeTranslationXYError outerCost +
          treeTranslationXYError innerCost +
        (2 * (outerCost : ℝ) * compositionCoefficientNormError) *
          ((innerCost : ℝ) * poseTranslationXYNormLimit +
            treeTranslationXYError innerCost) +
        compositionTranslationXYNormError ≤
      treeTranslationXYError (outerCost + innerCost + 1) := by
  have hOuterNonneg : 0 ≤ (outerCost : ℝ) := Nat.cast_nonneg _
  have hInnerNonneg : 0 ≤ (innerCost : ℝ) := Nat.cast_nonneg _
  have hInnerCost : innerCost ≤ treeCostLimit := by omega
  have hInnerReal : (innerCost : ℝ) ≤ treeCostLimit := by
    exact_mod_cast hInnerCost
  have hInnerFactor :
      2 * compositionCoefficientNormError * (innerCost : ℝ) ≤ 1 := by
    calc
      2 * compositionCoefficientNormError * (innerCost : ℝ)
          ≤
        2 * compositionCoefficientNormError * treeCostLimit := by
        gcongr
        norm_num [compositionCoefficientNormError]
      _ ≤ 1 := by
        norm_num [
          treeCostLimit,
          compositionCountLimit,
          compositionCoefficientNormError
        ]
  have hCross :
      (2 * (outerCost : ℝ) * compositionCoefficientNormError) *
          treeTranslationXYError innerCost ≤
        treeTranslationQuadratic *
          (outerCost : ℝ) * (innerCost : ℝ) := by
    have hScale :
        0 ≤
          treeTranslationQuadratic *
            (outerCost : ℝ) * (innerCost : ℝ) := by
      exact mul_nonneg
        (mul_nonneg
          (by norm_num [treeTranslationQuadratic])
          hOuterNonneg)
        hInnerNonneg
    have h :=
      mul_le_mul_of_nonneg_left hInnerFactor hScale
    calc
      (2 * (outerCost : ℝ) * compositionCoefficientNormError) *
            treeTranslationXYError innerCost
          =
        (treeTranslationQuadratic *
            (outerCost : ℝ) * (innerCost : ℝ)) *
          (2 * compositionCoefficientNormError * (innerCost : ℝ)) := by
        simp [treeTranslationXYError, pow_two]
        ring
      _ ≤
        treeTranslationQuadratic *
          (outerCost : ℝ) * (innerCost : ℝ) := by
        simpa using h
  have hExactTerm :
      (2 * (outerCost : ℝ) * compositionCoefficientNormError) *
          ((innerCost : ℝ) * poseTranslationXYNormLimit) =
        treeTranslationQuadratic *
          (outerCost : ℝ) * (innerCost : ℝ) := by
    norm_num [
      compositionCoefficientNormError,
      poseTranslationXYNormLimit,
      treeTranslationQuadratic
    ]
    ring
  have hLocal :
      compositionTranslationXYNormError ≤
        treeTranslationQuadratic := by
    norm_num [
      compositionTranslationXYNormError,
      treeTranslationQuadratic
    ]
  calc
    treeTranslationXYError outerCost +
            treeTranslationXYError innerCost +
          (2 * (outerCost : ℝ) * compositionCoefficientNormError) *
            ((innerCost : ℝ) * poseTranslationXYNormLimit +
              treeTranslationXYError innerCost) +
          compositionTranslationXYNormError
        ≤
      treeTranslationXYError outerCost +
            treeTranslationXYError innerCost +
          (treeTranslationQuadratic *
              (outerCost : ℝ) * (innerCost : ℝ) +
            treeTranslationQuadratic *
              (outerCost : ℝ) * (innerCost : ℝ)) +
        treeTranslationQuadratic := by
      rw [mul_add, hExactTerm]
      exact add_le_add
        (add_le_add le_rfl (add_le_add le_rfl hCross))
        hLocal
    _ ≤ treeTranslationXYError (outerCost + innerCost + 1) := by
      simp [treeTranslationXYError]
      norm_num [treeTranslationQuadratic]
      nlinarith

theorem TransformTree.translationXY_error
    (tree : TransformTree)
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (hProfiled : tree.Profiled)
    (hRange : tree.GraphInRange roundContract libmContract)
    (hCost : tree.cost ≤ treeCostLimit) :
    ‖translationXY (tree.binary64Eval roundContract libmContract) -
        translationXY tree.exactEval‖ ≤
      treeTranslationXYError tree.cost := by
  induction tree with
  | identity =>
      simp [TransformTree.binary64Eval, TransformTree.exactEval,
        TransformTree.cost, TransformTree.poseCount,
        TransformTree.composeCount, treeTranslationXYError]
  | pose =>
      simp [TransformTree.binary64Eval, TransformTree.exactEval,
        binary64Pose, exactPose, TransformTree.cost,
        TransformTree.poseCount, TransformTree.composeCount,
        translationXY, treeTranslationXYError]
      norm_num [treeTranslationQuadratic]
  | compose outer inner outerIH innerIH =>
      have hCostEq :
          (TransformTree.compose outer inner).cost =
            outer.cost + inner.cost + 1 := by
        simp [TransformTree.cost, TransformTree.poseCount,
          TransformTree.composeCount]
        omega
      have hOuterCost : outer.cost ≤ treeCostLimit := by
        rw [hCostEq] at hCost
        omega
      have hInnerCost : inner.cost ≤ treeCostLimit := by
        rw [hCostEq] at hCost
        omega
      have hOuter :=
        outerIH hProfiled.1 hRange.1 hOuterCost
      have hInner :=
        innerIH hProfiled.2 hRange.2.1 hInnerCost
      have hCoefficient :=
        (outer.coefficient_error
          roundContract
          libmContract
          hProfiled.1
          hRange.1).trans
          (treeCoefficientPotential_le_linear hOuterCost)
      have hExactInner :=
        inner.exact_translationXY_norm_le hProfiled.2
      have hPoseCost : inner.poseCount ≤ inner.cost := by
        simp [TransformTree.cost]
      have hPoseCostReal :
          (inner.poseCount : ℝ) ≤ inner.cost := by
        exact_mod_cast hPoseCost
      have hExactInnerCost :
          ‖translationXY inner.exactEval‖ ≤
            (inner.cost : ℝ) * poseTranslationXYNormLimit :=
        hExactInner.trans
          (mul_le_mul_of_nonneg_right hPoseCostReal (by
            norm_num [poseTranslationXYNormLimit]))
      have hSensitivity :=
        translation_compose_sensitivity
          (actualOuter :=
            outer.binary64Eval roundContract libmContract)
          (exactOuter := outer.exactEval)
          (actualInner :=
            inner.binary64Eval roundContract libmContract)
          (exactInner := inner.exactEval)
          outer.exact_coefficient_norm
      have hLocal :=
        binary64Compose_translationXY_local_error
          roundContract
          (outer.binary64Eval roundContract libmContract)
          (inner.binary64Eval roundContract libmContract)
          hRange.2.2
      calc
        ‖translationXY
              ((TransformTree.compose outer inner).binary64Eval
                roundContract libmContract) -
            translationXY (TransformTree.compose outer inner).exactEval‖
            ≤
          ‖translationXY
                (binary64Compose
                  roundContract
                  (outer.binary64Eval roundContract libmContract)
                  (inner.binary64Eval roundContract libmContract)) -
              translationXY
                (Dry.Geometry.PlanarTransform.compose
                  (outer.binary64Eval roundContract libmContract)
                  (inner.binary64Eval roundContract libmContract))‖ +
            ‖translationXY
                (Dry.Geometry.PlanarTransform.compose
                  (outer.binary64Eval roundContract libmContract)
                  (inner.binary64Eval roundContract libmContract)) -
              translationXY
                (Dry.Geometry.PlanarTransform.compose
                  outer.exactEval inner.exactEval)‖ := by
          simp only [TransformTree.binary64Eval, TransformTree.exactEval]
          rw [show
            translationXY
                  (binary64Compose
                    roundContract
                    (outer.binary64Eval roundContract libmContract)
                    (inner.binary64Eval roundContract libmContract)) -
                translationXY
                  (Dry.Geometry.PlanarTransform.compose
                    outer.exactEval inner.exactEval) =
              (translationXY
                  (binary64Compose
                    roundContract
                    (outer.binary64Eval roundContract libmContract)
                    (inner.binary64Eval roundContract libmContract)) -
                translationXY
                  (Dry.Geometry.PlanarTransform.compose
                    (outer.binary64Eval roundContract libmContract)
                    (inner.binary64Eval roundContract libmContract))) +
              (translationXY
                  (Dry.Geometry.PlanarTransform.compose
                    (outer.binary64Eval roundContract libmContract)
                    (inner.binary64Eval roundContract libmContract)) -
                translationXY
                  (Dry.Geometry.PlanarTransform.compose
                    outer.exactEval inner.exactEval)) by
            ring]
          exact norm_add_le _ _
        _ ≤
          compositionTranslationXYNormError +
            (treeTranslationXYError outer.cost +
                treeTranslationXYError inner.cost +
              (2 * (outer.cost : ℝ) *
                  compositionCoefficientNormError) *
                ((inner.cost : ℝ) * poseTranslationXYNormLimit +
                  treeTranslationXYError inner.cost)) := by
          apply add_le_add hLocal
          exact hSensitivity.trans (by
            apply add_le_add
            · exact add_le_add hOuter hInner
            · exact mul_le_mul
                hCoefficient
                (add_le_add hExactInnerCost hInner)
                (add_nonneg (norm_nonneg _) (norm_nonneg _))
                (mul_nonneg
                  (mul_nonneg (by norm_num) (Nat.cast_nonneg _))
                  (by norm_num [compositionCoefficientNormError])))
        _ ≤
          treeTranslationXYError
            ((TransformTree.compose outer inner).cost) := by
          rw [hCostEq]
          have hCombinedCost :
              outer.cost + inner.cost + 1 ≤ treeCostLimit := by
            simpa [hCostEq] using hCost
          have hBudget :=
            treeTranslationError_compose
              (outerCost := outer.cost)
              (innerCost := inner.cost)
              hCombinedCost
          linarith

theorem TransformTree.translationZ_error
    (tree : TransformTree)
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (hRange : tree.GraphInRange roundContract libmContract) :
    Approx
      (tree.binary64Eval roundContract libmContract).translation.z
      tree.exactEval.translation.z
      ((tree.composeCount : ℝ) * addSubErrorCeiling) := by
  induction tree with
  | identity =>
      simp [Approx, TransformTree.binary64Eval, TransformTree.exactEval,
        TransformTree.composeCount]
  | pose =>
      simp [Approx, TransformTree.binary64Eval, TransformTree.exactEval,
        binary64Pose, exactPose, TransformTree.composeCount]
  | compose outer inner outerIH innerIH =>
      have hLocal :=
        (binary64Compose_error
          roundContract
          (outer.binary64Eval roundContract libmContract)
          (inner.binary64Eval roundContract libmContract)
          hRange.2.2).2.2.2.2
      have hIntermediate :
          Approx
            (Dry.Geometry.PlanarTransform.compose
              (outer.binary64Eval roundContract libmContract)
              (inner.binary64Eval roundContract libmContract)).translation.z
            (Dry.Geometry.PlanarTransform.compose
              outer.exactEval inner.exactEval).translation.z
            (((outer.composeCount : ℝ) +
                (inner.composeCount : ℝ)) * addSubErrorCeiling) := by
        have hAdded := (innerIH hRange.2.1).add (outerIH hRange.1)
        simpa [Dry.Geometry.PlanarTransform.compose, applyVector,
          add_mul, add_comm, add_left_comm, add_assoc] using hAdded
      have hCombined := hLocal.trans hIntermediate
      have hBound :
          addSubErrorCeiling +
              ((outer.composeCount : ℝ) +
                (inner.composeCount : ℝ)) * addSubErrorCeiling ≤
            (((outer.composeCount + inner.composeCount + 1 : ℕ) : ℝ) *
              addSubErrorCeiling) := by
        push_cast
        ring_nf
        exact le_rfl
      have hNext := le_trans hCombined hBound
      simpa [TransformTree.binary64Eval, TransformTree.exactEval,
        TransformTree.composeCount] using hNext

theorem binary64Tree_error
    (tree : TransformTree)
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (hProfiled : tree.Profiled)
    (hRange : tree.GraphInRange roundContract libmContract)
    (hCount : tree.composeCount ≤ compositionCountLimit) :
    RepeatError
      (tree.binary64Eval roundContract libmContract)
      tree.exactEval
      repeatCoefficientErrorCeiling
      treeTranslationXYErrorCeiling
      repeatTranslationZErrorCeiling := by
  have hCost := tree.cost_le_treeCostLimit hCount
  have hCoefficient :=
    (tree.coefficient_error
      roundContract libmContract hProfiled hRange).trans
      (treeCoefficientPotential_le_ceiling hCost)
  have hTranslation :=
    tree.translationXY_error
      roundContract libmContract hProfiled hRange hCost
  have hCostReal : (tree.cost : ℝ) ≤ treeCostLimit := by
    exact_mod_cast hCost
  have hTranslationCeiling :
      treeTranslationXYError tree.cost ≤
        treeTranslationXYErrorCeiling := by
    calc
      treeTranslationXYError tree.cost
          ≤
        treeTranslationQuadratic * (treeCostLimit : ℝ) ^ 2 := by
        exact mul_le_mul_of_nonneg_left
          ((sq_le_sq₀ (Nat.cast_nonneg _) (Nat.cast_nonneg _)).2
            hCostReal)
          (by norm_num [treeTranslationQuadratic])
      _ ≤ treeTranslationXYErrorCeiling := by
        norm_num [
          treeTranslationQuadratic,
          treeCostLimit,
          compositionCountLimit,
          treeTranslationXYErrorCeiling
        ]
  have hZ :=
    tree.translationZ_error roundContract libmContract hRange
  have hCountReal :
      (tree.composeCount : ℝ) ≤ compositionCountLimit := by
    exact_mod_cast hCount
  have hZCeiling :
      (tree.composeCount : ℝ) * addSubErrorCeiling ≤
        repeatTranslationZErrorCeiling := by
    calc
      (tree.composeCount : ℝ) * addSubErrorCeiling
          ≤ compositionCountLimit * addSubErrorCeiling :=
        mul_le_mul_of_nonneg_right hCountReal (by
          norm_num [addSubErrorCeiling])
      _ ≤ repeatTranslationZErrorCeiling := by
        norm_num [
          compositionCountLimit,
          addSubErrorCeiling,
          repeatTranslationZErrorCeiling
        ]
  exact
    ⟨hCoefficient,
      hTranslation.trans hTranslationCeiling,
      le_trans hZ hZCeiling⟩

end

end Dry.Numeric.CompositionTree

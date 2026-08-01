import Dry.Numeric.Trig
import Mathlib.Analysis.Complex.Norm
import Mathlib.Tactic

/-!
# Bounded repeat-transform accumulation

This module models the exact repeat-loop recurrence in `features::expand_node`:

`instance = instance.compose(step)`

It combines the profiled degree/libm coefficient error with the checked binary64 composition graph.
The theorem is conditional on every composition satisfying the existing exact-operation range
predicate and on the default profile's 100,000-composition ceiling.

The bounds are intentionally conservative. They cover the sequential repeat accumulator, not an
arbitrary nested composition tree.
-/

namespace Dry.Numeric.Accumulation

open Dry.Geometry.PlanarTransform
open Dry.Numeric.RoundModel
open Dry.Numeric.Binary64
open Dry.Numeric.Angle
open Dry.Numeric.Trig

noncomputable section

def compositionCountLimit : ℕ :=
  100000

def poseTranslationComponentLimit : ℝ :=
  2 ^ 20

def poseTranslationXYNormLimit : ℝ :=
  2 ^ 21

def stepCoefficientNormError : ℝ :=
  1 / 2 ^ 44

def compositionCoefficientNormError : ℝ :=
  1 / 2 ^ 28

def coefficientErrorIncrement : ℝ :=
  1 / 2 ^ 27

def repeatCoefficientErrorCeiling : ℝ :=
  1 / 2 ^ 10

def compositionTranslationXYNormError : ℝ :=
  1 / 2 ^ 27

def translationErrorIncrement : ℝ :=
  2 ^ 12

def repeatTranslationXYErrorCeiling : ℝ :=
  2 ^ 29

def repeatTranslationZErrorCeiling : ℝ :=
  1 / 2 ^ 13

def coefficient (transform : Transform) : ℂ :=
  ⟨transform.c, transform.s⟩

def translationXY (transform : Transform) : ℂ :=
  ⟨transform.translation.x, transform.translation.y⟩

def exactPose (degrees : ℝ) (translation : Vec3) : Transform :=
  {
    c := Real.cos (exactRadians degrees)
    s := Real.sin (exactRadians degrees)
    translation
  }

def binary64Pose
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (degrees : ℝ)
    (translation : Vec3) : Transform :=
  {
    c := binary64Cos roundContract libmContract degrees
    s := binary64Sin roundContract libmContract degrees
    translation
  }

def exactRepeat (step : Transform) : ℕ → Transform
  | 0 => identity
  | n + 1 => compose (exactRepeat step n) step

def binary64Repeat
    (contract : RoundContract)
    (step : Transform) : ℕ → Transform
  | 0 => identity
  | n + 1 => binary64Compose contract (binary64Repeat contract step n) step

def RepeatGraphInRange
    (contract : RoundContract)
    (step : Transform)
    (count : ℕ) : Prop :=
  ∀ index, index < count →
    ComposeGraphInRange contract (binary64Repeat contract step index) step

def RepeatError
    (actual exact : Transform)
    (coefficientError translationXYError translationZError : ℝ) : Prop :=
  ‖coefficient actual - coefficient exact‖ ≤ coefficientError ∧
    ‖translationXY actual - translationXY exact‖ ≤ translationXYError ∧
      |actual.translation.z - exact.translation.z| ≤ translationZError

@[simp]
theorem coefficient_identity :
    coefficient identity = 1 := by
  apply Complex.ext
  · simp [coefficient, identity]
  · simp [coefficient, identity]

@[simp]
theorem translationXY_identity :
    translationXY identity = 0 := by
  apply Complex.ext
  · simp [translationXY, identity]
  · simp [translationXY, identity]

theorem coefficient_compose (outer inner : Transform) :
    coefficient (compose outer inner) =
      coefficient outer * coefficient inner := by
  apply Complex.ext
  · simp [coefficient, compose]
  · simp [coefficient, compose]
    ring

theorem translationXY_compose (outer inner : Transform) :
    translationXY (compose outer inner) =
      coefficient outer * translationXY inner + translationXY outer := by
  apply Complex.ext
  · simp [coefficient, translationXY, compose, applyVector]
  · simp [coefficient, translationXY, compose, applyVector]
    ring

theorem exactPose_coefficient_norm
    (degrees : ℝ)
    (translation : Vec3) :
    ‖coefficient (exactPose degrees translation)‖ = 1 := by
  rw [Complex.norm_def]
  have h :=
    Real.cos_sq_add_sin_sq (exactRadians degrees)
  simpa [coefficient, exactPose, Complex.normSq_apply, pow_two] using h

theorem exactRepeat_coefficient_norm
    (step : Transform)
    (hStep : ‖coefficient step‖ = 1)
    (count : ℕ) :
    ‖coefficient (exactRepeat step count)‖ = 1 := by
  induction count with
  | zero => simp [exactRepeat]
  | succ count ih =>
      rw [exactRepeat, coefficient_compose, norm_mul, ih, hStep, one_mul]

private theorem approx_pair_norm
    {actualRe exactRe actualIm exactIm error : ℝ}
    (hRe : Approx actualRe exactRe error)
    (hIm : Approx actualIm exactIm error) :
    ‖(⟨actualRe, actualIm⟩ : ℂ) - ⟨exactRe, exactIm⟩‖ ≤ 2 * error := by
  calc
    ‖(⟨actualRe, actualIm⟩ : ℂ) - ⟨exactRe, exactIm⟩‖
        ≤
      |((⟨actualRe, actualIm⟩ : ℂ) - ⟨exactRe, exactIm⟩).re| +
        |((⟨actualRe, actualIm⟩ : ℂ) - ⟨exactRe, exactIm⟩).im| :=
      Complex.norm_le_abs_re_add_abs_im _
    _ ≤ error + error := add_le_add hRe hIm
    _ = 2 * error := by ring

theorem binary64Pose_coefficient_error
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (degrees : ℝ)
    (translation : Vec3)
    (hDegrees : |degrees| ≤ degreeLimit) :
    ‖coefficient (binary64Pose roundContract libmContract degrees translation) -
        coefficient (exactPose degrees translation)‖ ≤
      stepCoefficientNormError := by
  have h :=
    approx_pair_norm
      (binary64Cos_error roundContract libmContract degrees hDegrees)
      (binary64Sin_error roundContract libmContract degrees hDegrees)
  exact h.trans_eq (by norm_num [stepCoefficientNormError, coefficientErrorCeiling])

theorem exactPose_translationXY_norm_le
    (degrees : ℝ)
    (translation : Vec3)
    (hx : |translation.x| ≤ poseTranslationComponentLimit)
    (hy : |translation.y| ≤ poseTranslationComponentLimit) :
    ‖translationXY (exactPose degrees translation)‖ ≤
      poseTranslationXYNormLimit := by
  calc
    ‖translationXY (exactPose degrees translation)‖
        ≤
      |(translationXY (exactPose degrees translation)).re| +
        |(translationXY (exactPose degrees translation)).im| :=
      Complex.norm_le_abs_re_add_abs_im _
    _ ≤ poseTranslationComponentLimit + poseTranslationComponentLimit :=
      add_le_add hx hy
    _ = poseTranslationXYNormLimit := by
      norm_num [poseTranslationComponentLimit, poseTranslationXYNormLimit]

theorem binary64Compose_coefficient_local_error
    (contract : RoundContract)
    (outer inner : Transform)
    (hRange : ComposeGraphInRange contract outer inner) :
    ‖coefficient (binary64Compose contract outer inner) -
        coefficient (compose outer inner)‖ ≤
      compositionCoefficientNormError := by
  have h := binary64Compose_error contract outer inner hRange
  have hPair := approx_pair_norm h.1 h.2.1
  exact hPair.trans_eq (by
    norm_num [compositionCoefficientNormError, vectorXYErrorCeiling])

theorem binary64Compose_translationXY_local_error
    (contract : RoundContract)
    (outer inner : Transform)
    (hRange : ComposeGraphInRange contract outer inner) :
    ‖translationXY (binary64Compose contract outer inner) -
        translationXY (compose outer inner)‖ ≤
      compositionTranslationXYNormError := by
  have h := binary64Compose_error contract outer inner hRange
  have hPair := approx_pair_norm h.2.2.1 h.2.2.2.1
  exact hPair.trans_eq (by
    norm_num [compositionTranslationXYNormError, pointXYErrorCeiling])

private theorem mul_error
    {actualOuter exactOuter actualStep exactStep : ℂ}
    (hActualStep :
      ‖actualStep‖ ≤ 1 + stepCoefficientNormError)
    (hExactOuter : ‖exactOuter‖ = 1)
    (hStep :
      ‖actualStep - exactStep‖ ≤ stepCoefficientNormError) :
    ‖actualOuter * actualStep - exactOuter * exactStep‖ ≤
      ‖actualOuter - exactOuter‖ * (1 + stepCoefficientNormError) +
        stepCoefficientNormError := by
  have hExactOuterLe : ‖exactOuter‖ ≤ 1 := hExactOuter.le
  rw [show
    actualOuter * actualStep - exactOuter * exactStep =
      (actualOuter - exactOuter) * actualStep +
        exactOuter * (actualStep - exactStep) by ring]
  calc
    ‖(actualOuter - exactOuter) * actualStep +
        exactOuter * (actualStep - exactStep)‖
        ≤
      ‖(actualOuter - exactOuter) * actualStep‖ +
        ‖exactOuter * (actualStep - exactStep)‖ :=
      norm_add_le _ _
    _ =
      ‖actualOuter - exactOuter‖ * ‖actualStep‖ +
        ‖exactOuter‖ * ‖actualStep - exactStep‖ := by
      rw [norm_mul, norm_mul]
    _ ≤
      ‖actualOuter - exactOuter‖ * (1 + stepCoefficientNormError) +
        1 * stepCoefficientNormError := by
      gcongr
    _ = _ := by ring

private theorem translation_error
    {actualCoefficient exactCoefficient stepTranslation actualTranslation
      exactTranslation : ℂ}
    (hStepTranslation :
      ‖stepTranslation‖ ≤ poseTranslationXYNormLimit) :
    ‖(actualCoefficient * stepTranslation + actualTranslation) -
        (exactCoefficient * stepTranslation + exactTranslation)‖ ≤
      ‖actualTranslation - exactTranslation‖ +
        ‖actualCoefficient - exactCoefficient‖ *
          poseTranslationXYNormLimit := by
  rw [show
    (actualCoefficient * stepTranslation + actualTranslation) -
        (exactCoefficient * stepTranslation + exactTranslation) =
      (actualTranslation - exactTranslation) +
        (actualCoefficient - exactCoefficient) * stepTranslation by ring]
  calc
    ‖(actualTranslation - exactTranslation) +
        (actualCoefficient - exactCoefficient) * stepTranslation‖
        ≤
      ‖actualTranslation - exactTranslation‖ +
        ‖(actualCoefficient - exactCoefficient) * stepTranslation‖ :=
      norm_add_le _ _
    _ =
      ‖actualTranslation - exactTranslation‖ +
        ‖actualCoefficient - exactCoefficient‖ * ‖stepTranslation‖ := by
      rw [norm_mul]
    _ ≤
      ‖actualTranslation - exactTranslation‖ +
        ‖actualCoefficient - exactCoefficient‖ *
          poseTranslationXYNormLimit := by
      gcongr

private theorem coefficient_increment_absorbs_drift :
    repeatCoefficientErrorCeiling * stepCoefficientNormError +
        stepCoefficientNormError + compositionCoefficientNormError ≤
      coefficientErrorIncrement := by
  norm_num [
    repeatCoefficientErrorCeiling,
    stepCoefficientNormError,
    compositionCoefficientNormError,
    coefficientErrorIncrement
  ]

private theorem translation_increment_absorbs_drift :
    repeatCoefficientErrorCeiling * poseTranslationXYNormLimit +
        compositionTranslationXYNormError ≤
      translationErrorIncrement := by
  norm_num [
    repeatCoefficientErrorCeiling,
    poseTranslationXYNormLimit,
    compositionTranslationXYNormError,
    translationErrorIncrement
  ]

private theorem count_times_coefficient_increment_le_ceiling
    {count : ℕ}
    (hCount : count ≤ compositionCountLimit) :
    (count : ℝ) * coefficientErrorIncrement ≤
      repeatCoefficientErrorCeiling := by
  have hReal :
      (count : ℝ) ≤ (compositionCountLimit : ℕ) := by
    exact_mod_cast hCount
  calc
    (count : ℝ) * coefficientErrorIncrement
        ≤ (compositionCountLimit : ℕ) * coefficientErrorIncrement :=
      mul_le_mul_of_nonneg_right hReal (by
        norm_num [coefficientErrorIncrement])
    _ ≤ repeatCoefficientErrorCeiling := by
      norm_num [
        compositionCountLimit,
        coefficientErrorIncrement,
        repeatCoefficientErrorCeiling
      ]

private theorem count_times_translation_increment_le_ceiling
    {count : ℕ}
    (hCount : count ≤ compositionCountLimit) :
    (count : ℝ) * translationErrorIncrement ≤
      repeatTranslationXYErrorCeiling := by
  have hReal :
      (count : ℝ) ≤ (compositionCountLimit : ℕ) := by
    exact_mod_cast hCount
  calc
    (count : ℝ) * translationErrorIncrement
        ≤ (compositionCountLimit : ℕ) * translationErrorIncrement :=
      mul_le_mul_of_nonneg_right hReal (by
        norm_num [translationErrorIncrement])
    _ ≤ repeatTranslationXYErrorCeiling := by
      norm_num [
        compositionCountLimit,
        translationErrorIncrement,
        repeatTranslationXYErrorCeiling
      ]

private theorem count_times_z_increment_le_ceiling
    {count : ℕ}
    (hCount : count ≤ compositionCountLimit) :
    (count : ℝ) * addSubErrorCeiling ≤
      repeatTranslationZErrorCeiling := by
  have hReal :
      (count : ℝ) ≤ (compositionCountLimit : ℕ) := by
    exact_mod_cast hCount
  calc
    (count : ℝ) * addSubErrorCeiling
        ≤ (compositionCountLimit : ℕ) * addSubErrorCeiling :=
      mul_le_mul_of_nonneg_right hReal (by
        norm_num [addSubErrorCeiling])
    _ ≤ repeatTranslationZErrorCeiling := by
      norm_num [
        compositionCountLimit,
        addSubErrorCeiling,
        repeatTranslationZErrorCeiling
      ]

private theorem binary64Repeat_linear_error
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (degrees : ℝ)
    (translation : Vec3)
    (count : ℕ)
    (hDegrees : |degrees| ≤ degreeLimit)
    (hx : |translation.x| ≤ poseTranslationComponentLimit)
    (hy : |translation.y| ≤ poseTranslationComponentLimit)
    (hCount : count ≤ compositionCountLimit)
    (hRange :
      RepeatGraphInRange
        roundContract
        (binary64Pose roundContract libmContract degrees translation)
        count) :
    RepeatError
      (binary64Repeat
        roundContract
        (binary64Pose roundContract libmContract degrees translation)
        count)
      (exactRepeat (exactPose degrees translation) count)
      ((count : ℝ) * coefficientErrorIncrement)
      ((count : ℝ) * translationErrorIncrement)
      ((count : ℝ) * addSubErrorCeiling) := by
  let actualStep :=
    binary64Pose roundContract libmContract degrees translation
  let exactStep :=
    exactPose degrees translation
  have hStep :
      ‖coefficient actualStep - coefficient exactStep‖ ≤
        stepCoefficientNormError :=
    binary64Pose_coefficient_error
      roundContract libmContract degrees translation hDegrees
  have hExactStep : ‖coefficient exactStep‖ = 1 :=
    exactPose_coefficient_norm degrees translation
  have hActualStep :
      ‖coefficient actualStep‖ ≤ 1 + stepCoefficientNormError := by
    calc
      ‖coefficient actualStep‖ =
          ‖(coefficient actualStep - coefficient exactStep) +
            coefficient exactStep‖ := by ring_nf
      _ ≤
          ‖coefficient actualStep - coefficient exactStep‖ +
            ‖coefficient exactStep‖ :=
        norm_add_le _ _
      _ ≤ stepCoefficientNormError + 1 :=
        add_le_add hStep hExactStep.le
      _ = 1 + stepCoefficientNormError := add_comm _ _
  have hTranslation :
      ‖translationXY exactStep‖ ≤ poseTranslationXYNormLimit :=
    exactPose_translationXY_norm_le degrees translation hx hy
  change
    RepeatError
      (binary64Repeat roundContract actualStep count)
      (exactRepeat exactStep count)
      ((count : ℝ) * coefficientErrorIncrement)
      ((count : ℝ) * translationErrorIncrement)
      ((count : ℝ) * addSubErrorCeiling)
  induction count with
  | zero =>
      simp [RepeatError, binary64Repeat, exactRepeat]
  | succ count ih =>
      have hCountPrevious : count ≤ compositionCountLimit :=
        (Nat.le_succ count).trans hCount
      have hRangePrevious :
          RepeatGraphInRange roundContract actualStep count :=
        fun index hIndex => hRange index (hIndex.trans (Nat.lt_succ_self count))
      have hPrevious := ih hCountPrevious hRangePrevious
      have hCurrentRange :
          ComposeGraphInRange
            roundContract
            (binary64Repeat roundContract actualStep count)
            actualStep :=
        hRange count (Nat.lt_succ_self count)
      have hCoefficientUniform :
          ‖coefficient (binary64Repeat roundContract actualStep count) -
              coefficient (exactRepeat exactStep count)‖ ≤
            repeatCoefficientErrorCeiling :=
        hPrevious.1.trans
          (count_times_coefficient_increment_le_ceiling hCountPrevious)
      have hCoefficientSensitivity :=
        mul_error
          (actualOuter :=
            coefficient (binary64Repeat roundContract actualStep count))
          (exactOuter := coefficient (exactRepeat exactStep count))
          (actualStep := coefficient actualStep)
          (exactStep := coefficient exactStep)
          hActualStep
          (exactRepeat_coefficient_norm exactStep hExactStep count)
          hStep
      have hCoefficientLocal :=
        binary64Compose_coefficient_local_error
          roundContract
          (binary64Repeat roundContract actualStep count)
          actualStep
          hCurrentRange
      have hCoefficientDrift :
          ‖coefficient (binary64Repeat roundContract actualStep count) -
                coefficient (exactRepeat exactStep count)‖ *
                stepCoefficientNormError +
              stepCoefficientNormError +
              compositionCoefficientNormError ≤
            coefficientErrorIncrement := by
        calc
          ‖coefficient (binary64Repeat roundContract actualStep count) -
                coefficient (exactRepeat exactStep count)‖ *
                stepCoefficientNormError +
              stepCoefficientNormError +
              compositionCoefficientNormError
              ≤
            repeatCoefficientErrorCeiling * stepCoefficientNormError +
              stepCoefficientNormError +
              compositionCoefficientNormError := by
            exact add_le_add
              (add_le_add
                (mul_le_mul_of_nonneg_right hCoefficientUniform (by
                  norm_num [stepCoefficientNormError]))
                le_rfl)
              le_rfl
          _ ≤ coefficientErrorIncrement :=
            coefficient_increment_absorbs_drift
      have hCoefficient :
          ‖coefficient
                (binary64Compose
                  roundContract
                  (binary64Repeat roundContract actualStep count)
                  actualStep) -
              coefficient (compose (exactRepeat exactStep count) exactStep)‖ ≤
            ((count + 1 : ℕ) : ℝ) * coefficientErrorIncrement := by
        calc
          ‖coefficient
                (binary64Compose
                  roundContract
                  (binary64Repeat roundContract actualStep count)
                  actualStep) -
              coefficient (compose (exactRepeat exactStep count) exactStep)‖
              ≤
            ‖coefficient
                  (binary64Compose
                    roundContract
                    (binary64Repeat roundContract actualStep count)
                    actualStep) -
                coefficient
                  (compose
                    (binary64Repeat roundContract actualStep count)
                    actualStep)‖ +
              ‖coefficient
                  (compose
                    (binary64Repeat roundContract actualStep count)
                    actualStep) -
                coefficient (compose (exactRepeat exactStep count) exactStep)‖ :=
            by
              rw [show
                coefficient
                      (binary64Compose
                        roundContract
                        (binary64Repeat roundContract actualStep count)
                        actualStep) -
                    coefficient (compose (exactRepeat exactStep count) exactStep) =
                  (coefficient
                      (binary64Compose
                        roundContract
                        (binary64Repeat roundContract actualStep count)
                        actualStep) -
                    coefficient
                      (compose
                        (binary64Repeat roundContract actualStep count)
                        actualStep)) +
                  (coefficient
                      (compose
                        (binary64Repeat roundContract actualStep count)
                        actualStep) -
                    coefficient (compose (exactRepeat exactStep count) exactStep)) by
                ring]
              exact norm_add_le _ _
          _ ≤
            compositionCoefficientNormError +
              (‖coefficient (binary64Repeat roundContract actualStep count) -
                    coefficient (exactRepeat exactStep count)‖ *
                  (1 + stepCoefficientNormError) +
                stepCoefficientNormError) :=
            add_le_add hCoefficientLocal (by
              simpa only [coefficient_compose] using hCoefficientSensitivity)
          _ ≤
            ‖coefficient (binary64Repeat roundContract actualStep count) -
                coefficient (exactRepeat exactStep count)‖ +
              coefficientErrorIncrement := by
            calc
              compositionCoefficientNormError +
                    (‖coefficient
                          (binary64Repeat roundContract actualStep count) -
                        coefficient (exactRepeat exactStep count)‖ *
                        (1 + stepCoefficientNormError) +
                      stepCoefficientNormError)
                  =
                ‖coefficient
                    (binary64Repeat roundContract actualStep count) -
                  coefficient (exactRepeat exactStep count)‖ +
                  (‖coefficient
                      (binary64Repeat roundContract actualStep count) -
                    coefficient (exactRepeat exactStep count)‖ *
                      stepCoefficientNormError +
                    stepCoefficientNormError +
                    compositionCoefficientNormError) := by ring
              _ ≤
                ‖coefficient
                    (binary64Repeat roundContract actualStep count) -
                  coefficient (exactRepeat exactStep count)‖ +
                  coefficientErrorIncrement :=
                add_le_add le_rfl hCoefficientDrift
          _ ≤
            (count : ℝ) * coefficientErrorIncrement +
              coefficientErrorIncrement :=
            add_le_add hPrevious.1 le_rfl
          _ = ((count + 1 : ℕ) : ℝ) * coefficientErrorIncrement := by
            push_cast
            ring
      have hTranslationSensitivity :=
        translation_error
          (actualCoefficient :=
            coefficient (binary64Repeat roundContract actualStep count))
          (exactCoefficient := coefficient (exactRepeat exactStep count))
          (stepTranslation := translationXY exactStep)
          (actualTranslation :=
            translationXY (binary64Repeat roundContract actualStep count))
          (exactTranslation := translationXY (exactRepeat exactStep count))
          hTranslation
      have hTranslationLocal :=
        binary64Compose_translationXY_local_error
          roundContract
          (binary64Repeat roundContract actualStep count)
          actualStep
          hCurrentRange
      have hTranslationDrift :
          ‖coefficient (binary64Repeat roundContract actualStep count) -
                coefficient (exactRepeat exactStep count)‖ *
                poseTranslationXYNormLimit +
              compositionTranslationXYNormError ≤
            translationErrorIncrement := by
        calc
          ‖coefficient (binary64Repeat roundContract actualStep count) -
                coefficient (exactRepeat exactStep count)‖ *
                poseTranslationXYNormLimit +
              compositionTranslationXYNormError
              ≤
            repeatCoefficientErrorCeiling * poseTranslationXYNormLimit +
              compositionTranslationXYNormError := by
            exact add_le_add
              (mul_le_mul_of_nonneg_right hCoefficientUniform (by
                norm_num [poseTranslationXYNormLimit]))
              le_rfl
          _ ≤ translationErrorIncrement :=
            translation_increment_absorbs_drift
      have hSameTranslation :
          translationXY actualStep = translationXY exactStep := by
        rfl
      have hTranslationNext :
          ‖translationXY
                (binary64Compose
                  roundContract
                  (binary64Repeat roundContract actualStep count)
                  actualStep) -
              translationXY (compose (exactRepeat exactStep count) exactStep)‖ ≤
            ((count + 1 : ℕ) : ℝ) * translationErrorIncrement := by
        calc
          ‖translationXY
                (binary64Compose
                  roundContract
                  (binary64Repeat roundContract actualStep count)
                  actualStep) -
              translationXY (compose (exactRepeat exactStep count) exactStep)‖
              ≤
            ‖translationXY
                  (binary64Compose
                    roundContract
                    (binary64Repeat roundContract actualStep count)
                    actualStep) -
                translationXY
                  (compose
                    (binary64Repeat roundContract actualStep count)
                    actualStep)‖ +
              ‖translationXY
                  (compose
                    (binary64Repeat roundContract actualStep count)
                    actualStep) -
                translationXY (compose (exactRepeat exactStep count) exactStep)‖ :=
            by
              rw [show
                translationXY
                      (binary64Compose
                        roundContract
                        (binary64Repeat roundContract actualStep count)
                        actualStep) -
                    translationXY (compose (exactRepeat exactStep count) exactStep) =
                  (translationXY
                      (binary64Compose
                        roundContract
                        (binary64Repeat roundContract actualStep count)
                        actualStep) -
                    translationXY
                      (compose
                        (binary64Repeat roundContract actualStep count)
                        actualStep)) +
                  (translationXY
                      (compose
                        (binary64Repeat roundContract actualStep count)
                        actualStep) -
                    translationXY (compose (exactRepeat exactStep count) exactStep)) by
                ring]
              exact norm_add_le _ _
          _ ≤
            compositionTranslationXYNormError +
              (‖translationXY
                    (binary64Repeat roundContract actualStep count) -
                  translationXY (exactRepeat exactStep count)‖ +
                ‖coefficient
                    (binary64Repeat roundContract actualStep count) -
                  coefficient (exactRepeat exactStep count)‖ *
                    poseTranslationXYNormLimit) :=
            add_le_add hTranslationLocal (by
              rw [translationXY_compose, translationXY_compose, hSameTranslation]
              exact hTranslationSensitivity)
          _ ≤
            ‖translationXY (binary64Repeat roundContract actualStep count) -
                translationXY (exactRepeat exactStep count)‖ +
              translationErrorIncrement := by
            calc
              compositionTranslationXYNormError +
                    (‖translationXY
                          (binary64Repeat roundContract actualStep count) -
                        translationXY (exactRepeat exactStep count)‖ +
                      ‖coefficient
                          (binary64Repeat roundContract actualStep count) -
                        coefficient (exactRepeat exactStep count)‖ *
                        poseTranslationXYNormLimit)
                  =
                ‖translationXY
                    (binary64Repeat roundContract actualStep count) -
                  translationXY (exactRepeat exactStep count)‖ +
                  (‖coefficient
                      (binary64Repeat roundContract actualStep count) -
                    coefficient (exactRepeat exactStep count)‖ *
                      poseTranslationXYNormLimit +
                    compositionTranslationXYNormError) := by ring
              _ ≤
                ‖translationXY
                    (binary64Repeat roundContract actualStep count) -
                  translationXY (exactRepeat exactStep count)‖ +
                  translationErrorIncrement :=
                add_le_add le_rfl hTranslationDrift
          _ ≤
            (count : ℝ) * translationErrorIncrement +
              translationErrorIncrement :=
            add_le_add hPrevious.2.1 le_rfl
          _ = ((count + 1 : ℕ) : ℝ) * translationErrorIncrement := by
            push_cast
            ring
      have hZLocal :=
        (binary64Compose_error
          roundContract
          (binary64Repeat roundContract actualStep count)
          actualStep
          hCurrentRange).2.2.2.2
      have hZIntermediate :
          Approx
            (compose
              (binary64Repeat roundContract actualStep count)
              actualStep).translation.z
            (compose (exactRepeat exactStep count) exactStep).translation.z
            ((count : ℝ) * addSubErrorCeiling) := by
        simpa [Approx, compose, applyVector, actualStep, exactStep,
          binary64Pose, exactPose] using
          hPrevious.2.2
      have hZ :=
        hZLocal.trans hZIntermediate
      refine ⟨?_, ?_, ?_⟩
      · simpa [binary64Repeat, exactRepeat] using hCoefficient
      · simpa [binary64Repeat, exactRepeat] using hTranslationNext
      · have hBound :
            addSubErrorCeiling + (count : ℝ) * addSubErrorCeiling ≤
              ((count + 1 : ℕ) : ℝ) * addSubErrorCeiling := by
          push_cast
          ring_nf
          exact le_rfl
        have hZNext :
            Approx
              (binary64Compose
                roundContract
                (binary64Repeat roundContract actualStep count)
                actualStep).translation.z
              (compose (exactRepeat exactStep count) exactStep).translation.z
              (((count + 1 : ℕ) : ℝ) * addSubErrorCeiling) :=
          le_trans hZ hBound
        simpa [binary64Repeat, exactRepeat] using hZNext

theorem binary64Repeat_error
    (roundContract : RoundContract)
    (libmContract : LibmContract)
    (degrees : ℝ)
    (translation : Vec3)
    (count : ℕ)
    (hDegrees : |degrees| ≤ degreeLimit)
    (hx : |translation.x| ≤ poseTranslationComponentLimit)
    (hy : |translation.y| ≤ poseTranslationComponentLimit)
    (hCount : count ≤ compositionCountLimit)
    (hRange :
      RepeatGraphInRange
        roundContract
        (binary64Pose roundContract libmContract degrees translation)
        count) :
    RepeatError
      (binary64Repeat
        roundContract
        (binary64Pose roundContract libmContract degrees translation)
        count)
      (exactRepeat (exactPose degrees translation) count)
      repeatCoefficientErrorCeiling
      repeatTranslationXYErrorCeiling
      repeatTranslationZErrorCeiling := by
  have hLinear :=
    binary64Repeat_linear_error
      roundContract
      libmContract
      degrees
      translation
      count
      hDegrees
      hx
      hy
      hCount
      hRange
  exact
    ⟨hLinear.1.trans
        (count_times_coefficient_increment_le_ceiling hCount),
      hLinear.2.1.trans
        (count_times_translation_increment_le_ceiling hCount),
      hLinear.2.2.trans
        (count_times_z_increment_le_ceiling hCount)⟩

end

end Dry.Numeric.Accumulation

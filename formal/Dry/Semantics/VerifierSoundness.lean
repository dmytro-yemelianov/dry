import Mathlib.Data.Rat.Defs
import Mathlib.Tactic

/-!
# Verifier Rule Soundness Semantics (FM1.5e)

This module formalizes core verifier check predicates and proves their exact mathematical soundness:
- Speed range validation (`minSpeed ≤ speed ≤ maxSpeed`);
- Maximum volumetric flow validation (`flow ≤ maxFlow`);
- Retraction speed and distance limits (`speed ≤ maxSpeed`, `distance ≤ maxDistance`);
- Monotonic Z elevation validation (`zPrevious ≤ zCurrent`).
-/

namespace Dry.Semantics.VerifierSoundness

def validateSpeed (speed minSpeed maxSpeed : ℚ) : Bool :=
  decide (minSpeed ≤ speed ∧ speed ≤ maxSpeed)

def validateFlow (flow maxFlow : ℚ) : Bool :=
  decide (flow ≤ maxFlow)

def validateRetractionSpeed (speed maxRetractSpeed : ℚ) : Bool :=
  decide (speed ≤ maxRetractSpeed)

def validateRetractionDistance (dist maxRetractDist : ℚ) : Bool :=
  decide (dist ≤ maxRetractDist)

def validateMonotonicZ (zPrevious zCurrent : ℚ) : Bool :=
  decide (zPrevious ≤ zCurrent)

theorem validateSpeed_sound (speed minSpeed maxSpeed : ℚ)
    (h : validateSpeed speed minSpeed maxSpeed = true) :
    minSpeed ≤ speed ∧ speed ≤ maxSpeed := by
  dsimp [validateSpeed] at h
  exact of_decide_eq_true h

theorem validateFlow_sound (flow maxFlow : ℚ)
    (h : validateFlow flow maxFlow = true) :
    flow ≤ maxFlow := by
  dsimp [validateFlow] at h
  exact of_decide_eq_true h

theorem validateRetractionSpeed_sound (speed maxRetractSpeed : ℚ)
    (h : validateRetractionSpeed speed maxRetractSpeed = true) :
    speed ≤ maxRetractSpeed := by
  dsimp [validateRetractionSpeed] at h
  exact of_decide_eq_true h

theorem validateRetractionDistance_sound (dist maxRetractDist : ℚ)
    (h : validateRetractionDistance dist maxRetractDist = true) :
    dist ≤ maxRetractDist := by
  dsimp [validateRetractionDistance] at h
  exact of_decide_eq_true h

theorem validateMonotonicZ_sound (zPrevious zCurrent : ℚ)
    (h : validateMonotonicZ zPrevious zCurrent = true) :
    zPrevious ≤ zCurrent := by
  dsimp [validateMonotonicZ] at h
  exact of_decide_eq_true h

theorem coreValidators_sound
    (speed minSpeed maxSpeed flow maxFlow retractSpeed maxRetractSpeed
      retractDistance maxRetractDistance zPrevious zCurrent : ℚ)
    (hSpeed : validateSpeed speed minSpeed maxSpeed = true)
    (hFlow : validateFlow flow maxFlow = true)
    (hRetractSpeed :
      validateRetractionSpeed retractSpeed maxRetractSpeed = true)
    (hRetractDistance :
      validateRetractionDistance retractDistance maxRetractDistance = true)
    (hMonotonicZ : validateMonotonicZ zPrevious zCurrent = true) :
    (minSpeed ≤ speed ∧ speed ≤ maxSpeed) ∧
      flow ≤ maxFlow ∧
      retractSpeed ≤ maxRetractSpeed ∧
      retractDistance ≤ maxRetractDistance ∧
      zPrevious ≤ zCurrent := by
  exact ⟨validateSpeed_sound _ _ _ hSpeed,
    validateFlow_sound _ _ hFlow,
    validateRetractionSpeed_sound _ _ hRetractSpeed,
    validateRetractionDistance_sound _ _ hRetractDistance,
    validateMonotonicZ_sound _ _ hMonotonicZ⟩

end Dry.Semantics.VerifierSoundness

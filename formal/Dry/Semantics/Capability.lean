import Mathlib.Data.Rat.Defs
import Mathlib.Tactic

/-!
# Abstract Capability Predicate (FM1.8)

This module formalizes a small machine capability predicate:
- `MachineProfile`: feedrate range, volumetric flow cap, max temperature;
- `checkCapability`: evaluates toolpath requirements against machine capabilities;
- Proves fail-closed property: `checkCapability` returns false whenever any requirement exceeds capabilities.

It intentionally does not model target lowering, controller behavior, semantic
lifting, kinematics, source maps, opaque commands, or physical machines.
-/

namespace Dry.Semantics.Capability

structure MachineProfile where
  max_feedrate : ℚ
  max_flow : ℚ
  max_temp : ℚ
deriving DecidableEq, Repr

structure ToolpathRequirement where
  req_feedrate : ℚ
  req_flow : ℚ
  req_temp : ℚ
deriving DecidableEq, Repr

def checkCapability (p : MachineProfile) (r : ToolpathRequirement) : Bool :=
  decide (r.req_feedrate ≤ p.max_feedrate ∧
          r.req_flow ≤ p.max_flow ∧
          r.req_temp ≤ p.max_temp)

theorem checkCapability_sound (p : MachineProfile) (r : ToolpathRequirement)
    (h : checkCapability p r = true) :
    r.req_feedrate ≤ p.max_feedrate ∧ r.req_flow ≤ p.max_flow ∧ r.req_temp ≤ p.max_temp := by
  exact decide_eq_true_iff.mp h

theorem checkCapability_fail_closed_feedrate (p : MachineProfile) (r : ToolpathRequirement)
    (h : p.max_feedrate < r.req_feedrate) :
    checkCapability p r = false := by
  dsimp [checkCapability]
  exact decide_eq_false (by intro hCond; have h1 := hCond.1; linarith)

theorem checkCapability_fail_closed (p : MachineProfile) (r : ToolpathRequirement)
    (h : p.max_feedrate < r.req_feedrate ∨
      p.max_flow < r.req_flow ∨
      p.max_temp < r.req_temp) :
    checkCapability p r = false := by
  dsimp [checkCapability]
  apply decide_eq_false
  intro accepted
  rcases h with hFeedrate | hFlow | hTemperature
  · exact (not_le_of_gt hFeedrate) accepted.1
  · exact (not_le_of_gt hFlow) accepted.2.1
  · exact (not_le_of_gt hTemperature) accepted.2.2

end Dry.Semantics.Capability

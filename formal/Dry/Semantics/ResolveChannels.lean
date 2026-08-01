import Dry.Language.Common
import Dry.Language.L2
import Dry.Language.WellFormed
import Mathlib.Tactic

/-!
# Process channel resolution semantics (FM1.5b)

This module models process channel state propagation during lowering:
- Process channels: `temperature`, `fan`, `flow`, `tool`;
- Operations: `temperature`, `fan`, `flow`, `tool`, `dwell`, `move`;
- `flow` defaults to 1.0 (`Number.finite 1`), omitting explicit flow when ratio = 1.0;
- `temperature`, `fan`, and `tool` default to `none`;
- Channel settings propagate forward to all subsequent emitted motion and dwell segments;
- Channel changes do not themselves emit motion segments.

The Lean model proves:
1. `resolve` fold determinism and append law;
2. default moves carry default channel state (`flow = none`, `temp = none`, `fan = none`, `tool = none`);
3. explicit channel updates propagate to all subsequent emitted segments;
4. subsequent channel updates do not rewrite prior emitted segments.
-/

namespace Dry.Semantics.ResolveChannels

open Dry.Language
open Dry.Language.L2

inductive Op where
  | move (finish : PartialVec3) (speed : Number)
  | dwell (seconds : Number)
  | temperature (nozzle : Number)
  | fan (speed : Number)
  | flow (ratio : Number)
  | tool (index : Nat)
deriving DecidableEq, Repr

structure State where
  position : PartialVec3
  temperature : Option Number
  fan : Option Number
  flow : Number
  tool : Option Nat
deriving DecidableEq, Repr

def initialState (startPos : PartialVec3 := ⟨none, none, none⟩) : State :=
  { position := startPos,
    temperature := none,
    fan := none,
    flow := .finite 1,
    tool := none }

def flowField (ratio : Number) : Option Number :=
  if ratio = .finite 1 then none else some ratio

def step (state : State) (op : Op) : State × Option Segment :=
  match op with
  | .temperature t => ({ state with temperature := some t }, none)
  | .fan f => ({ state with fan := some f }, none)
  | .flow ratio => ({ state with flow := ratio }, none)
  | .tool index => ({ state with tool := some index }, none)
  | .dwell s =>
      let seg : Segment := {
        start := state.position,
        finish := state.position,
        travel := true,
        speed := .finite 0,
        length := .finite 0,
        volume := .finite 0,
        filament := .finite 0,
        width := none,
        height := none,
        kind := .dwell,
        centre := none,
        clockwise := false,
        temperature := state.temperature,
        fan := state.fan,
        flow := none,
        tool := state.tool,
        dwellSeconds := some s,
        manualGcode := none,
        orientation := none,
        controlPoints := none
      }
      (state, some seg)
  | .move p speed =>
      let seg : Segment := {
        start := state.position,
        finish := p,
        travel := true,
        speed := speed,
        length := .finite 0,
        volume := .finite 0,
        filament := .finite 0,
        width := none,
        height := none,
        kind := .line,
        centre := none,
        clockwise := false,
        temperature := state.temperature,
        fan := state.fan,
        flow := flowField state.flow,
        tool := state.tool,
        dwellSeconds := none,
        manualGcode := none,
        orientation := none,
        controlPoints := none
      }
      ({ state with position := p }, some seg)

def resolve (state : State) : List Op → State × List Segment
  | [] => (state, [])
  | op :: ops =>
      let (nextState, maybeSeg) := step state op
      let (finalState, restSegs) := resolve nextState ops
      match maybeSeg with
      | none => (finalState, restSegs)
      | some seg => (finalState, seg :: restSegs)

theorem resolve_append (state : State) (ops1 ops2 : List Op) :
    (resolve state (ops1 ++ ops2)).2 = (resolve state ops1).2 ++ (resolve (resolve state ops1).1 ops2).2 ∧
    (resolve state (ops1 ++ ops2)).1 = (resolve (resolve state ops1).1 ops2).1 := by
  induction ops1 generalizing state with
  | nil =>
      simp [resolve]
  | cons op ops1 ih =>
      simp [resolve]
      cases step state op with
      | mk nextState maybeSeg =>
          cases maybeSeg <;> simp <;> exact ih nextState

theorem default_moves_carry_default_channels
    (pos : PartialVec3)
    (moves : List (PartialVec3 × Number)) :
    ∀ seg ∈ (resolve (initialState pos) (moves.map (fun (p, s) => Op.move p s))).2,
      seg.temperature = none ∧ seg.fan = none ∧ seg.flow = none ∧ seg.tool = none := by
  induction moves generalizing pos with
  | nil =>
      intro seg h
      cases h
  | cons head tail ih =>
      intro seg h
      rcases head with ⟨p, s⟩
      dsimp [resolve, step, initialState, flowField] at h
      cases h with
      | head =>
          refine ⟨rfl, rfl, rfl, rfl⟩
      | tail _ hTail =>
          exact ih p seg hTail

theorem explicit_temperature_propagates
    (pos : PartialVec3)
    (tempVal : Number)
    (moves : List (PartialVec3 × Number)) :
    ∀ seg ∈ (resolve (initialState pos) (Op.temperature tempVal :: moves.map (fun (p, s) => Op.move p s))).2,
      seg.temperature = some tempVal := by
  induction moves generalizing pos with
  | nil =>
      intro seg h
      cases h
  | cons head tail ih =>
      intro seg h
      rcases head with ⟨p, s⟩
      dsimp [resolve, step, initialState] at h
      cases h with
      | head =>
          rfl
      | tail _ hTail =>
          exact ih p seg hTail

theorem explicit_flow_propagates
    (pos : PartialVec3)
    (flowRatio : Number)
    (hNotOne : flowRatio ≠ .finite 1)
    (moves : List (PartialVec3 × Number)) :
    ∀ seg ∈ (resolve (initialState pos) (Op.flow flowRatio :: moves.map (fun (p, s) => Op.move p s))).2,
      seg.flow = some flowRatio := by
  induction moves generalizing pos with
  | nil =>
      intro seg h
      cases h
  | cons head tail ih =>
      intro seg h
      rcases head with ⟨p, s⟩
      dsimp [resolve, step, initialState, flowField] at h
      rw [if_neg hNotOne] at h
      cases h with
      | head =>
          rfl
      | tail _ hTail =>
          exact ih p seg hTail

theorem later_channel_update_does_not_rewrite_earlier
    (pos : PartialVec3)
    (t1 t2 : Number)
    (p1 p2 : PartialVec3)
    (s1 s2 : Number) :
    let ops := [Op.temperature t1, Op.move p1 s1, Op.temperature t2, Op.move p2 s2]
    let (_, segs) := resolve (initialState pos) ops
    ∃ seg1 seg2, segs = [seg1, seg2] ∧ seg1.temperature = some t1 ∧ seg2.temperature = some t2 := by
  dsimp [resolve, step, initialState]
  refine ⟨{ start := pos, finish := p1, travel := true, speed := s1, length := .finite 0, volume := .finite 0, filament := .finite 0, width := none, height := none, kind := .line, centre := none, clockwise := false, temperature := some t1, fan := none, flow := none, tool := none, dwellSeconds := none, manualGcode := none, orientation := none, controlPoints := none },
          { start := p1, finish := p2, travel := true, speed := s2, length := .finite 0, volume := .finite 0, filament := .finite 0, width := none, height := none, kind := .line, centre := none, clockwise := false, temperature := some t2, fan := none, flow := none, tool := none, dwellSeconds := none, manualGcode := none, orientation := none, controlPoints := none },
          rfl, rfl, rfl⟩

end Dry.Semantics.ResolveChannels

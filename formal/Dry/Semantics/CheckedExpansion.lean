import Dry.Semantics.ExpandFeatures
import Mathlib.Tactic

/-!
# Checked planar feature expansion

This module gives a computable error-trace semantics for a bounded refinement subset of the current
Rust feature expander. It models ordered groups and repeats, dynamic depth/node/operation checks,
locally self-contained moves, invariant tool operations and transformed manual-code rejection.

Natural-number X translations keep the refinement fixtures exact and avoid claiming binary64 or
trigonometric refinement. Pose finiteness, empty names, arcs, splines, orientations, counter overflow
and epsilon-based transform identity remain separate obligations.
-/

namespace Dry.Semantics.CheckedExpansion

open Dry.Semantics.ExpandFeatures

structure PartialPoint where
  x : Option Nat
  y : Option Nat
  z : Option Nat
deriving BEq, Repr

structure Point where
  x : Nat
  y : Nat
  z : Nat
deriving BEq, Repr

inductive SourceOp where
  | tool (index : Nat)
  | move (point : PartialPoint)
  | manualGcode (text : String)
deriving BEq, Repr

inductive OutputOp where
  | tool (index : Nat)
  | move (point : Point)
  | manualGcode (text : String)
deriving BEq, Repr

inductive FailureCode where
  | maxDepth
  | maxNodes
  | maxOps
  | undefinedCoordinate
  | transformedManual
deriving BEq, Repr

structure Failure where
  code : FailureCode
  message : String
deriving BEq, Repr

structure State where
  ops : List OutputOp := []
  nodes : Nat := 0
deriving BEq, Repr

def structuralAlgebra : Dry.Semantics.ExpandFeatures.Algebra Nat SourceOp :=
  {
    identity := 0
    compose := Nat.add
    apply := fun _ op => op
  }

inductive Event where
  | enter (depth : Nat) (path : String)
  | feature (transform : Nat) (path : String) (ops : List SourceOp)
deriving BEq, Repr

mutual

  def eventsNode
      (parent depth : Nat)
      (path : String) :
      Node Nat SourceOp → List Event
    | .feature pose ops =>
        [
          .enter depth path,
          .feature (parent + pose) path ops
        ]
    | .group children =>
        .enter depth path ::
          eventsChildren parent (depth + 1) path 0 children
    | .repeat count step child =>
        .enter depth path ::
          (List.range count).flatMap fun index =>
            eventsNode
              (parent + power structuralAlgebra step index)
              (depth + 1)
              s!"{path}.instances[{index}]"
              child

  def eventsChildren
      (parent depth : Nat)
      (parentPath : String)
      (index : Nat) :
      List (Node Nat SourceOp) → List Event
    | [] => []
    | child :: rest =>
        eventsNode
            parent
            depth
            s!"{parentPath}.children[{index}]"
            child ++
          eventsChildren parent depth parentPath (index + 1) rest

end

def eventsProgram :
    List (Node Nat SourceOp) → Nat → List Event
  | [], _ => []
  | node :: rest, index =>
      eventsNode 0 0 s!"features[{index}]" node ++
        eventsProgram rest (index + 1)

def depthFailure (limits : Limits) (path : String) : Failure :=
  ⟨.maxDepth, s!"{path} exceeds max feature depth ({limits.maxDepth})"⟩

def nodeFailure (limits : Limits) (path : String) : Failure :=
  ⟨.maxNodes, s!"{path} exceeds max expanded nodes ({limits.maxNodes})"⟩

def opFailure (limits : Limits) (path : String) : Failure :=
  ⟨.maxOps, s!"{path} exceeds max expanded ops ({limits.maxOps})"⟩

def undefinedFailure (path axis : String) : Failure :=
  ⟨.undefinedCoordinate,
    s!"{path}.{axis} is undefined; features must be locally self-contained"⟩

def manualFailure (path : String) : Failure :=
  ⟨.transformedManual, s!"{path}.manual_gcode cannot be transformed safely"⟩

def inheritAxis
    (current previous : Option Nat)
    (path axis : String) : Except Failure Nat :=
  match current with
  | some value => .ok value
  | none =>
      match previous with
      | some value => .ok value
      | none => .error (undefinedFailure path axis)

def inheritPoint
    (point position : PartialPoint)
    (path : String) : Except Failure Point := do
  let x ← inheritAxis point.x position.x path "x"
  let y ← inheritAxis point.y position.y path "y"
  let z ← inheritAxis point.z position.z path "z"
  pure ⟨x, y, z⟩

def pushOp
    (limits : Limits)
    (path : String)
    (op : OutputOp)
    (state : State) : Except Failure State :=
  if state.ops.length ≥ limits.maxOps then
    .error (opFailure limits path)
  else
    .ok { state with ops := state.ops ++ [op] }

def runOps
    (limits : Limits)
    (transform : Nat)
    (featurePath : String) :
    List SourceOp → Nat → PartialPoint → State → Except Failure State
  | [], _, _, state => .ok state
  | op :: rest, index, position, state => do
      let opPath := s!"{featurePath}.ops[{index}]"
      match op with
      | .tool toolIndex =>
          let next ← pushOp limits opPath (.tool toolIndex) state
          runOps limits transform featurePath rest (index + 1) position next
      | .move partialPoint =>
          let localPoint ← inheritPoint partialPoint position opPath
          let transformed : Point :=
            { localPoint with x := transform + localPoint.x }
          let next ← pushOp limits opPath (.move transformed) state
          let localPosition : PartialPoint :=
            ⟨some localPoint.x, some localPoint.y, some localPoint.z⟩
          runOps limits transform featurePath rest (index + 1) localPosition next
      | .manualGcode text =>
          if transform != 0 then
            .error (manualFailure opPath)
          else
            let next ← pushOp limits opPath (.manualGcode text) state
            runOps limits transform featurePath rest (index + 1) position next

def runEvents
    (limits : Limits) :
    List Event → State → Except Failure State
  | [], state => .ok state
  | event :: rest, state =>
      match event with
      | .enter depth path =>
          if depth > limits.maxDepth then
            .error (depthFailure limits path)
          else
            let visited := state.nodes + 1
            if visited > limits.maxNodes then
              .error (nodeFailure limits path)
            else
              runEvents limits rest { state with nodes := visited }
      | .feature transform path ops => do
          let next ← runOps limits transform path ops 0 ⟨none, none, none⟩ state
          runEvents limits rest next

def evaluate
    (limits : Limits)
    (program : Program Nat SourceOp) : Except Failure (List OutputOp) := do
  let state ← runEvents limits (eventsProgram program.features 0) {}
  pure state.ops

theorem runEvents_depth_failure
    (limits : Limits)
    (depth : Nat)
    (path : String)
    (rest : List Event)
    (state : State)
    (exceeded : depth > limits.maxDepth) :
    runEvents limits (.enter depth path :: rest) state =
      .error (depthFailure limits path) := by
  simp [runEvents, exceeded]

theorem runEvents_node_failure
    (limits : Limits)
    (depth : Nat)
    (path : String)
    (rest : List Event)
    (state : State)
    (withinDepth : ¬ depth > limits.maxDepth)
    (exceeded : state.nodes + 1 > limits.maxNodes) :
    runEvents limits (.enter depth path :: rest) state =
      .error (nodeFailure limits path) := by
  simp [runEvents, withinDepth, exceeded]

theorem pushOp_budget_failure
    (limits : Limits)
    (path : String)
    (op : OutputOp)
    (state : State)
    (full : state.ops.length ≥ limits.maxOps) :
    pushOp limits path op state =
      .error (opFailure limits path) := by
  simp [pushOp, full]

theorem inheritPoint_missing_y
    (path : String)
    (x : Nat)
    (pointZ previousX previousZ : Option Nat) :
    inheritPoint
        ⟨some x, none, pointZ⟩
        ⟨previousX, none, previousZ⟩
        path =
      .error (undefinedFailure path "y") := by
  cases pointZ <;> cases previousZ <;> rfl

theorem runOps_transformed_manual_failure
    (limits : Limits)
    (transform : Nat)
    (featurePath text : String)
    (rest : List SourceOp)
    (index : Nat)
    (position : PartialPoint)
    (state : State)
    (transformed : transform != 0) :
    runOps limits transform featurePath
        (.manualGcode text :: rest) index position state =
      .error (manualFailure s!"{featurePath}.ops[{index}]") := by
  simp [runOps, transformed]

inductive CheckedEvaluates :
    Limits → Program Nat SourceOp → Except Failure (List OutputOp) → Prop where
  | result (limits : Limits) (program : Program Nat SourceOp) :
      CheckedEvaluates limits program (evaluate limits program)

namespace CheckedEvaluates

theorem deterministic
    (left : CheckedEvaluates limits program first)
    (right : CheckedEvaluates limits program second) :
    first = second := by
  cases left
  cases right
  rfl

end CheckedEvaluates

end Dry.Semantics.CheckedExpansion

/-!
# Deterministic ordered state folds

This module captures the structural reason that an ordered list of total deterministic operations has a
unique final state. Concrete geometry, channel and floating-point obligations belong to later FM1
modules.
-/

namespace Dry.Semantics.OrderedFold

structure State where
  position : Int
  channel : Int
deriving DecidableEq

structure Op where
  positionDelta : Int
  channelDelta : Int
deriving DecidableEq

def applyOp (state : State) (op : Op) : State :=
  {
    position := state.position + op.positionDelta
    channel := state.channel + op.channelDelta
  }

def run : State → List Op → State
  | state, [] => state
  | state, op :: ops => run (applyOp state op) ops

inductive Exec : State → List Op → State → Prop where
  | nil (state : State) : Exec state [] state
  | cons (tail : Exec (applyOp state op) ops final) :
      Exec state (op :: ops) final

theorem run_append (state : State) (prefixOps suffix : List Op) :
    run state (prefixOps ++ suffix) = run (run state prefixOps) suffix := by
  induction prefixOps generalizing state with
  | nil => rfl
  | cons op ops inductionHypothesis =>
      simpa [run] using inductionHypothesis (applyOp state op)

namespace Exec

theorem eq_run (execution : Exec initial ops final) :
    final = run initial ops := by
  induction execution with
  | nil => rfl
  | cons tail inductionHypothesis =>
      simpa [run] using inductionHypothesis

theorem deterministic
    (left : Exec initial ops first)
    (right : Exec initial ops second) :
    first = second := by
  rw [left.eq_run, right.eq_run]

end Exec

end Dry.Semantics.OrderedFold

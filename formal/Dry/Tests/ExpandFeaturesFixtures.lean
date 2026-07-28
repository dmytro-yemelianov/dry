import Dry.Semantics.ExpandFeatures

/-!
# Executable planar feature-expansion fixtures

These fixtures exercise the computable structural model with natural-number translations. They
snapshot ordering, repeat instances, exact operation counts and dynamic node/depth budgets without
claiming refinement to Rust `f64` transforms.
-/

namespace Dry.Tests.ExpandFeaturesFixtures

open Dry.Semantics.ExpandFeatures

def translationAlgebra : Dry.Semantics.ExpandFeatures.Algebra Nat Nat :=
  {
    identity := 0
    compose := Nat.add
    apply := Nat.add
  }

def leaf (pose value : Nat) : Node Nat Nat :=
  .feature pose [value]

def groupOrder : Node Nat Nat :=
  .group [leaf 0 10, leaf 0 20]

def repeatZero : Node Nat Nat :=
  .repeat 0 10 (leaf 1 0)

def repeatInstances : Node Nat Nat :=
  .repeat 3 10 (leaf 1 0)

def nestedOrder : Node Nat Nat :=
  .repeat 2 10 (.group [leaf 1 0, leaf 2 0])

structure Fixture where
  id : String
  node : Node Nat Nat
  expectedExpansion : List Nat
  expectedOpCount : Nat
  expectedNodeCount : Nat
  expectedMaxDepth : Nat

def fixtures : List Fixture :=
  [
    ⟨"group-source-order", groupOrder, [10, 20], 2, 3, 1⟩,
    ⟨"repeat-zero", repeatZero, [], 0, 1, 0⟩,
    ⟨"repeat-instances", repeatInstances, [1, 11, 21], 3, 4, 1⟩,
    ⟨"nested-repeat-order", nestedOrder, [1, 2, 11, 12], 4, 7, 2⟩
  ]

def renderValues (values : List Nat) : String :=
  String.intercalate "," (values.map toString)

def renderFixture (fixture : Fixture) : String :=
  let actualExpansion := expandNode translationAlgebra 0 fixture.node
  let actualOpCount := opCount fixture.node
  let actualNodeCount := nodeCount fixture.node
  let actualMaxDepth := maxDepth fixture.node
  let agrees :=
    actualExpansion == fixture.expectedExpansion &&
      actualOpCount == fixture.expectedOpCount &&
      actualNodeCount == fixture.expectedNodeCount &&
      actualMaxDepth == fixture.expectedMaxDepth
  let outcome := if agrees then "valid" else "fixture-error"
  s!"{fixture.id}\t{outcome}\t{renderValues actualExpansion}\t{actualOpCount}\t{actualNodeCount}\t{actualMaxDepth}"

def render : String :=
  String.intercalate "\n"
    ("id\toutcome\texpanded\top_count\tnode_count\tmax_depth" ::
      fixtures.map renderFixture)

end Dry.Tests.ExpandFeaturesFixtures

def main : IO Unit :=
  IO.println Dry.Tests.ExpandFeaturesFixtures.render

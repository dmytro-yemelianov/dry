# Dry formal model

This directory contains the machine-checked abstract semantics tracked by the FM1 claim registry. It
does not replace Rust conformance tests or imply that abstract real-number results already apply to the
binary64 implementation.

The Lean and Mathlib versions are pinned together at `v4.30.0`.

## Build

With `elan` installed:

```sh
cd formal
lake update
lake exe cache get
lake build --wfail
```

Validate the repository links and claim statuses from the repository root:

```sh
python3 -m pip install -r tools/requirements.txt
python3 tools/validate_proof_claims.py
```

The initial checked claims are:

- exact-real planar transform composition;
- uniqueness of the final state for an ordered deterministic operation fold;
- commutative dimension composition and the deposition-equation dimension;
- coherent canonical normalization across same-dimension rational unit conversions;
- Dry IR v0 L2 validation succeeds exactly for the selected serializer-neutral well-formed subset;
- normalized L2 logical equivalence is exactly structural equality;
- ordered `Feature`/`Group`/`Repeat` expansion, exact repeat operation counts, resource-budget
  preservation and deterministic traces;
- checked success/first-error determinism for a bounded operation-validation subset;
- exact-real planar feature-pose action on abstract points, vectors and invariant operations.

The executable positive/negative L2 boundary fixtures, natural-number feature-expansion fixtures and
Lean-generated Rust-refinement fixtures are checked against committed TSV and schema-validated JSON
snapshots:

```sh
python3 tools/check_proof_fixtures.py
```

The L2 JSON fixture encodes finite logical values as normalized numerator/denominator strings and uses
an explicit `non-finite` token. The feature fixture snapshots source ordering, zero and nonzero repeats,
dynamic operation/node counts and maximum expansion depth. The refinement JSON assigns 28 expected
success or exact first-error cases for ordered groups/repeats, feature names and poses,
local-coordinate inheritance, arcs, splines, orientations, manual code and resource limits. Its
fixture-only `NaN`/`inf`/`-inf` strings are converted to IEEE-754 values by an independent adapter
before the Rust expander is called; they are not production wire syntax.
`crates/core/tests/feature_refinement.rs` repeats every observation to check determinism and confirms
the exact result against the generated expectation. A second Lean-generated two-case corpus uses an
executable integer quarter-turn model to check parent-first Feature and nested Repeat composition
against Rust within `1e-12`; this tolerance is structural evidence, not a general binary64 theorem.
Four pose and three one-step composition cases additionally export real-π reference intervals and
exact power-of-two budgets. A native Rust test checks degree conversion and cardinal sine/cosine
results, then reconstructs the compose operation graph with arbitrary-precision exact dyadics from
the observed binary64 inputs before comparing each local result to its profiled interval.
`tools/check_feature_mutations.py` then compiles 27 pinned source changes to
`crates/core/src/features.rs` in isolated workspaces and requires each
named fixture to kill its assigned mutant. The source hash and replacements are reviewed in
`proofs/feature-refinement-mutations-v0.toml`; a non-compiling mutant does not count as killed.

Run that bounded mutation gate directly with:

```sh
python3 tools/check_feature_mutations.py
```

The checked refinement status is limited to those committed subsets. Universal native/wasm numeric
refinement, finite-width counter overflow, nonzero/unit orientation behavior, mutation coverage
outside the pinned feature-expansion changes, full frame graphs and wider Rust refinement remain
explicitly pending in
[`../proofs/claims.toml`](../proofs/claims.toml).

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
dynamic operation/node counts and maximum expansion depth. The refinement JSON assigns 11 expected
success or exact first-error cases for ordered groups/repeats, local-coordinate inheritance, manual
code and resource limits; `crates/core/tests/feature_refinement.rs` consumes it independently against
the Rust expander and repeats every observation to check determinism.

The checked refinement status is limited to that committed subset. Numeric error bounds, remaining
operation validation, full frame graphs and wider Rust refinement remain explicitly pending in
[`../proofs/claims.toml`](../proofs/claims.toml).

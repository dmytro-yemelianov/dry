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
- Dry IR v0 L2 validation succeeds exactly for the selected serializer-neutral well-formed subset.

The executable positive/negative L2 boundary fixtures are checked against their committed snapshot:

```sh
python3 tools/check_proof_fixtures.py
```

Numeric error bounds and Rust refinement for the transform remain explicitly pending in
[`../proofs/claims.toml`](../proofs/claims.toml).

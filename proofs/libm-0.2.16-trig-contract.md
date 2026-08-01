# Imported libm 0.2.16 trigonometric contract

Status: conditional assurance assumption
Scope: binary64 `sin` and `cos` for finite arguments in `[-7, 7]`

## Pinned implementation

- crate: `libm`
- version: `0.2.16`
- crates.io checksum:
  `b6d2cec3eae94f9f509c767b45932f1ada8350c4bdb85af2fcab4a3c14807981`
- upstream tag: [`libm-v0.2.16`](https://github.com/rust-lang/compiler-builtins/tree/libm-v0.2.16/libm)
- release commit: `dfd2203a4d6110820ad7bb65cafe1bf331a03a3d`

The files extracted by Cargo from that crate match the release commit:

| Source | SHA-256 |
|---|---|
| `libm/src/math/sin.rs` | `b48cebd120fc2ac93a6563059cabda28a72c8680304eb8335b92b412235cc232` |
| `libm/src/math/cos.rs` | `6e81752899c471bd42d04bdf08e0b22bc10a29f92fa2a343281b995fd342f035` |
| `libm/src/math/k_sin.rs` | `dfe6524cb2d530ec51eb1a2ab233aeb65b3f12177c1fb2f50f044ea41170f5d8` |
| `libm/src/math/k_cos.rs` | `d4867527877c846aa8e2a0b43cf06f03e2d94c321a548acaf8d5171c09ff4057` |
| `libm/src/math/rem_pio2.rs` | `5a040ab09dc356bca6d57712327151ed43003e2bac156553e3eb87d1cdefd2d1` |

## Imported accuracy premise

The release's [MPFR precision policy](https://github.com/rust-lang/compiler-builtins/blob/libm-v0.2.16/libm-test/src/precision.rs)
labels its configured allowance as the implementation's worst-case precision and assigns one ULP to
both `Sin` and `Cos`. The policy file has SHA-256
`85b067b3023ba4768348a3ac2a87f8cd9f33d70943e5bcae88d06b4d0b5ad4b7`.

`formal/Dry/Numeric/Trig.lean` imports that policy through `LibmContract` as two explicit premises for
each function:

1. the libm output differs from the correctly rounded MPFR reference by at most `2^-52`; and
2. the reference differs from exact real trigonometry by at most `2^-53`.

These absolute ceilings are conservative over trigonometric outputs in `[-1, 1]`. Lean proves a
same-input libm ceiling of `2^-51`, then combines it with the checked `2^-46` degree-conversion error
and the one-Lipschitz bounds for real sine and cosine. The resulting end-to-end coefficient ceiling is
`2^-45` for degree inputs in `[-360, 360]`.

## Deliberate limitation

The upstream case-list, random, edge and spaced MPFR tests are strong implementation evidence, but
they are not an exhaustive proof over every binary64 input. Therefore:

- the Lean result is a checked conditional theorem;
- the numeric budget is bounded under this named imported contract;
- native and wasm refinement remain `pending`; and
- replacing this assumption with exhaustive interval/Gappa/Flocq-style verification of the pinned
  source remains a follow-up obligation.

# Design: Product workflows & pilot guides (Slice F)

**Date:** 2026-06-29
**Status:** Approved — tracked in GitHub issues
**Branch:** `feat/product-workflows` (off main, after the v0.3.0 release)
**Source docs:** `docs/08-production-transition.md` (§WS5), `docs/09-customer-readiness.md` (task #6).

## Goal

Convert the shipped contracts/releases into workflows a **pilot user can follow**: a CLI cookbook, three
pilot guides, runnable examples, and one high-value error-message fix. Every recipe is **verified by
actually running it** against committed inputs, so the docs can't drift into aspiration.

## Acceptance (08·WS5)

- A new technical user can complete a validated **generate → verify → emit** workflow quickly (the
  authoring pilot guide).
- A post-slicer user can complete **review → trace → rewrite** without reading Rust docs (the review
  pilot guide).
- Errors point at user-actionable fixes.

## Artifacts

| Path | What | Verified by |
|---|---|---|
| `docs/15-cli-cookbook.md` | every CLI command with a runnable, copy-pasteable recipe | running each against committed inputs |
| `docs/pilots/authoring.md` | generate → verify → emit (Python + TS) | running the Python example |
| `docs/pilots/post-slicer-review.md` | review → trace → rewrite on a `.gcode` (no Rust) | running the CLI recipes |
| `docs/pilots/sdk-integration.md` | embed Dry, reproduce a conformance vector | running the reproduction |
| `examples/authoring.py`, `examples/authoring.ts` | a small authored design (line + arc + verify + emit) | `examples/authoring.py` runs against the built module |
| `examples/part.gcode` | a small realistic slicer-style g-code for the review guide | feeding it to review/trace/rewrite |

## Code: one actionable error (F4)

Running an **IR command on raw g-code** (`dry emit part.gcode`) currently fails with
`expected value at line 1 column 1`. The shared `load()`/`load_streaming()` path will detect g-code-shaped
input (first non-empty, non-`;` line begins with `G`/`M`/`T` + digit) and die with an actionable hint:
*"<file> looks like raw G-code — use `dry import-gcode` / `review-gcode` instead."* Covered by a CLI test.

## Inputs reused

- IR fixtures: `conformance/gcode/*.json` (emit/simulate/verify/optimize/pack/unpack).
- Real g-code: slice-A `conformance/vectors/*/expected.gcode` plus a new `examples/part.gcode`.
- Python module builds to `py/python/dry/` (already present); TS API in `sdk/ts/src/`.

## Wiring

Index `docs/15-cli-cookbook.md` + the pilot guides in README + docs/README; mark 08·WS5 and 09 #6
delivered; add a "Cookbook & pilot guides" pointer to the README quickstart.

## Work breakdown (issues)

- Epic: Slice F — Product workflows & pilot guides.
- F1 CLI cookbook; F2 three pilot guides; F3 runnable examples (`examples/`); F4 actionable
  raw-g-code-on-IR-command error; F5 docs wiring + WS5/#6 markers.

# Design: Typed contract input (Slice H, first cut)

**Date:** 2026-06-29
**Status:** Approved — tracked in GitHub issues
**Branch:** `feat/typed-contract-input` (off main)
**Source docs:** `docs/08-production-transition.md` (§WS6, "typed contract input object to replace
comma-string boundaries"). Fixes the wart surfaced in slice F.

## Problem

The SDK `verify()` takes machine limits as **comma-strings**: Python `verify(bounds="0,200,0,200,0,200")`,
TS `verify('generic', 0, 0, '0,200,0,200,0,200', false, '...')`. That is error-prone and un-ergonomic for
a programmatic API.

## Constraint

`maturin` is not available in this environment, so the **native** PyO3 `resolve_verify` cannot be rebuilt
or verified here. Therefore the native signatures (CSV-based) stay **unchanged**, and the typed input is
added at the **SDK wrapper layer** — which is exactly where users feel the wart. This keeps the change
back-compatible and fully verifiable (Python against the existing `_native` module; TS via `npm run
build`).

## Design

Accept a **structured** form *or* the legacy CSV string for `bounds` and `speed_range`, converting
structured → CSV before the existing native call.

- **`bounds`**: `[[x0, x1], [y0, y1], [z0, z1]]` (mm) → `"x0,x1,y0,y1,z0,z1"`.
- **`speed_range`**: `[min, max]` (mm/min) → `"min,max"`.
- A `str` is passed through unchanged (back-compat).

### Python (`py/python/dry/__init__.py`)

`verify(..., bounds=None, ..., speed_range=None)` gains list support:

```python
design.verify(bounds=[[0, 200], [0, 200], [0, 200]], speed_range=[300, 9000])
design.verify(bounds="0,200,0,200,0,200")  # still works
```

### TypeScript (`sdk/ts/src/design.ts`)

`bounds` / `speedRange` accept `string | number[][]` / `string | [number, number]`:

```ts
design.verify('generic', 0, 0, [[0, 200], [0, 200], [0, 200]], false, [300, 9000]);
design.verify('generic', 0, 0, '0,200,0,200,0,200');  // still works
```

## Tests

- Python: a test that `verify(bounds=[[...]])` and `verify(bounds="...")` agree (run against the existing
  `_native` module — no maturin needed).
- TS: a test in `sdk/ts/test/` that the structured and CSV forms agree (via `npm run build && npm test`).

## Docs / examples

Update `examples/authoring.py` / `authoring.ts` and the authoring pilot guide to the structured form
(noting CSV still works), and tighten the `docs/14` comma-string note ("the SDK now accepts structured
limits; CSV is still accepted").

## Out of scope (follow-up)

Changing the native PyO3/wasm signatures to accept a single typed `Contracts` object (needs maturin to
verify) — left for when the Python build toolchain is available. The CLI keeps CSV flags (idiomatic for a
CLI).

## Work breakdown (issues)

- Epic: Slice H (first cut) — typed contract input.
- H1 Python SDK structured bounds/speed_range + test; H2 TS SDK structured bounds/speedRange + test;
  H3 update examples + authoring guide + docs/14 note.

# Design: Dry IR v0 spec + public conformance vectors (Slice A)

**Date:** 2026-06-29
**Status:** Approved — implementation tracked in GitHub issues
**Source docs:** `docs/08-production-transition.md` (§WS1), `docs/09-customer-readiness.md` (integrator gate, immediate tasks #3/#4)

## Goal

Publish the **public contract** for Dry IR v0 so an independent implementation can read and write
Dry files without depending on `dry-core`. This is the first slice of the production-transition program
(08·WS1) and the gate that unblocks the *SDK integrator* segment (09).

Scope of "done" (chosen: **Standard**):

- a normative prose spec for Dry IR v0 (JSON wire form + `DRY0` + `DRY1` + units + segment kinds +
  `Meta` + version semantics + versioning/compatibility policy + conformance model);
- a machine-readable JSON Schema (draft 2020-12) for the JSON wire form;
- a curated, clean-room public vector set (each vector: input JSON + `DRY0`/`DRY1` bytes + metrics +
  g-code goldens);
- an **independent** pure-Python validator that round-trips JSON + both binary forms with **no
  `dry-core`**;
- a CI gate.

Explicitly **out of scope** for this slice: a from-scratch reference *engine* (re-deriving
`simulate`/`emit` outputs in a second language). That was the "Maximal" option we did not pick. Metrics
and g-code remain published **golden reference outputs**, generated and drift-gated by `dry-core`.

## Approach (chosen)

Hand-authored spec + engine-generated goldens + independent Python validator. The engine is the single
source of truth for expected outputs; a Rust "bless" test regenerates goldens and the normal CI run
asserts the committed goldens still match (drift gate). Rejected alternatives: generating the schema from
Rust types via `schemars` (can't express the binary layout or semver prose, and drags proc-macro deps
into an engine whose entire dependency set is `serde` + `miniz_oxide`); and a fully manual no-generator
flow (hand-encoding DEFLATE goldens is infeasible and drift is undetectable).

## Conformance model — the load-bearing decision

`DRY0`/`DRY1` bodies are DEFLATE (miniz_oxide level 8). Python's `zlib` will **not** produce
byte-identical bytes, and cross-language canonical-JSON float formatting can differ. So conformance is
defined **semantically**, not by cross-language byte-identity:

- **JSON conformance** — parse a vector's `input.json` to the IR value model and re-serialize to a
  *semantically equal* IR. Byte-identical canonical JSON (field order = struct declaration order;
  shortest-round-trip floats; unset/default channels omitted; no insignificant whitespace) is documented
  as a property the **reference** encoder guarantees, not the conformance bar.
- **Binary conformance** — (a) decode the reference `expected.dry0`/`expected.dry1` to a semantically
  equal IR, and (b) the implementation's own encode→decode round-trips losslessly. Cross-implementation
  byte-identity is **explicitly not promised**; within one implementation at fixed settings encoding is
  deterministic and byte-stable (that is exactly what the `dry-core` drift gate enforces).
- **Semantic equality** — exact f64 bit-equality for every quantity; structural equality of
  options / enums / arrays.

## Artifacts & layout

| Path | What | Source of truth |
|---|---|---|
| `docs/10-dry-ir-v0-spec.md` | Normative prose spec | hand-authored |
| `spec/dry-ir-v0.schema.json` | JSON Schema (draft 2020-12) for the JSON wire form | hand-authored |
| `conformance/vectors/<name>/` | one bundle per vector (see below) | `input.json` authored; rest generated |
| `conformance/vectors/MANIFEST.json` | index: vectors, feature-coverage matrix, sha256 of every artifact | generated |
| `conformance/vectors/_negative/` | malformed inputs that MUST be rejected | authored |
| `tools/validate_vectors.py`, `tools/requirements.txt` | independent stdlib codec + `jsonschema` (no `dry-core`) | authored |
| `crates/core/tests/spec_vectors.rs` | bless-generator + drift/round-trip gate | authored |

Per-vector bundle:

- `vector.json` — metadata: `name`, `description`, `feature_tags[]`, `emit_params` (or `null`),
  `ir_version`, `frozen` (bool).
- `input.json` — the canonical IR (also the JSON wire vector).
- `expected.dry0`, `expected.dry1` — binary goldens (dry-core, miniz_oxide level 8).
- `metrics.json` — `simulate()` golden (published reference output).
- `expected.gcode` — `emit()` golden, present only when `emit_params != null`.

## Vector set (~10 curated, clean-room)

Each vector targets specific spec features so the coverage matrix is auditable:

1. `minimal_line` — one extruding line, channels unset (byte-identity baseline).
2. `arc_g2_g3` — arcs with centre, both directions.
3. `spline` — `control_points`.
4. `dwell` — `dwell_s`.
5. `retract_unretract` — retract/unretract kinds.
6. `deposit` — deposit kind.
7. `manual_gcode` — verbatim passthrough (and the `enc_ver` legacy boundary).
8. `channels_full` — temperature/fan/flow/tool all set.
9. `five_axis` — `orientation` unit vector.
10. `meta_header` — `Meta` generator/units/source_hash/invariants.
11. `edge_empty_and_none_axes` — empty toolpath; `None` axes on first move.

Negative vectors under `_negative/` (satisfy WS1 "unknown enum/kind/version failures are documented"):
unknown segment kind, bad magic, truncated body, future `enc_ver`. The spec documents each failure and
both the Python validator and a Rust test assert rejection with the right error class.

## Scope boundary of the independent validator

The Python validator proves the **format/codec** contract — JSON + `DRY0` + `DRY1` decode / encode /
round-trip + JSON-Schema validation + sha256 of every artifact against the MANIFEST. It does **not**
re-derive `metrics.json` or `expected.gcode` (that is `simulate`/`emit` engine logic). Those goldens are
published reference outputs, generated and drift-gated by `dry-core`, so an external *engine* could check
itself — but re-computing them is not in this slice.

## Versioning & compatibility policy (spec section)

Three independently-named version axes:

- IR schema version — `Toolpath.version` (currently `0`).
- `DRY0` encoding version — `enc_ver` (currently `1`; `0` legacy, accepted without `manual_gcode`).
- `DRY1` encoding version — `enc_ver` (currently `2`; `1` legacy).

SemVer mapping for the IR/spec: additive optional field or new `SegmentKind` = **minor**;
remove / rename / retype a field or change a default = **major**. Reader rules are documented as they
behave today: unknown JSON object fields are ignored (forward-compat); unknown `SegmentKind` enum values
are rejected at deserialize/decode. The compatibility promise — *old valid v0 files keep decoding unless
a major bump migrates them* — is enforced by `frozen: true` vectors that a regression test must always
decode.

## Generation + drift gate (Rust)

`crates/core/tests/spec_vectors.rs`:

- reads each `conformance/vectors/*/input.json`;
- regenerates `expected.dry0`, `expected.dry1`, `metrics.json`, and (when `emit_params` present)
  `expected.gcode`;
- with `UPDATE_VECTORS=1` set, writes them and refreshes `MANIFEST.json` (the "bless" path);
- otherwise asserts the committed goldens are byte-identical to freshly generated ones and that every
  MANIFEST sha256 matches (the drift gate);
- asserts every `frozen: true` vector still decodes (back-compat regression);
- asserts each `_negative/` input is rejected with the expected error class.

Runs under the existing `cargo test --all`. No new crate, no new core dependency.

## Independent validator (Python)

`tools/validate_vectors.py` (stdlib `zlib` + `struct` + `json`, plus `jsonschema` from
`tools/requirements.txt` — independent of `dry-core`). For each vector:

1. validate `input.json` against `spec/dry-ir-v0.schema.json`;
2. parse `input.json` → IR dict;
3. independently decode `expected.dry0` → IR; assert semantically equal to (1);
4. independently decode `expected.dry1` → IR; assert semantically equal;
5. independently encode IR → `dry0'`/`dry1'`, decode back, assert lossless self round-trip;
6. verify every artifact's sha256 against `MANIFEST.json`.

For each `_negative/` input, assert the independent decoder rejects it with the documented error.
Exit non-zero on any mismatch.

## CI wiring

New job `spec-vectors (python)` in `.github/workflows/ci.yml`: checkout → setup Python →
`pip install -r tools/requirements.txt` → `python tools/validate_vectors.py conformance/vectors`. The
Rust drift gate rides the existing `core` job's `cargo test --all`.

## Acceptance → 08·WS1

- *"a new implementation can read and write at least one vector without dry-core"* → the Python validator
  does it for **all** vectors. ✓
- *"old valid v0 files keep decoding after new releases unless explicitly migrated"* → `frozen` vectors +
  regression test. ✓
- *"unknown enum/kind/version failures are documented"* → spec section + `_negative/` vectors asserted in
  both Rust and Python. ✓

## Work breakdown (GitHub issues)

- **Epic**: Slice A — Dry IR v0 spec + public conformance vectors.
- A1: Normative spec doc `docs/10-dry-ir-v0-spec.md`.
- A2: JSON Schema `spec/dry-ir-v0.schema.json`.
- A3: Curated vector bundles + bless-generator + drift gate (`spec_vectors.rs`, `MANIFEST.json`).
- A4: Negative vectors + documented failure modes.
- A5: Independent pure-Python validator.
- A6: CI job for the Python validator.
- A7: Docs index wiring (README + docs/README) + compatibility-policy cross-links.

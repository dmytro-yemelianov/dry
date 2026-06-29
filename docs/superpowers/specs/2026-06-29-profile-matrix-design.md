# Design: Golden firmware/printer profile matrix (Slice M)

**Date:** 2026-06-29
**Status:** Approved — tracked in GitHub issues
**Branch:** `feat/profile-matrix` (off main)
**Source:** `docs/08-production-transition.md` §WS3 remaining ("golden profiles for a small supported
firmware/printer matrix").

## Goal

A small, curated, **supported** matrix of machine/material/firmware profiles — each validated and
exercised through the review pipeline with a drift-gated golden — so pilots have ready-to-use profiles
and the profile contract is proven across the matrix.

## Matrix (6 entries — Marlin/Klipper/Duet × PLA/PETG/ABS)

| Entry | Firmware | Material | Envelope |
|---|---|---|---|
| `marlin-pla-i3` | marlin | PLA | 220×220×250 |
| `marlin-petg-i3` | marlin | PETG | 220×220×250 |
| `klipper-pla-corexy` | klipper | PLA | 350×350×250 |
| `klipper-abs-corexy` | klipper | ABS | 350×350×250 |
| `duet-petg-cartesian` | duet | PETG | 300×300×300 |
| `duet-abs-corexy` | duet | ABS | 256×256×256 |

Material defaults: PLA `min_temp 190`, PETG `220`, ABS `230`; per-material `max_volumetric_flow` and
retraction limits.

## Layout

```
conformance/profile-matrix/
  MANIFEST.json                      # the supported matrix (name, firmware, material, envelope)
  <entry>/profile.json               # the supported profile (authored clean-room)
  <entry>/review.json                # golden ReviewReport of examples/part.gcode under that profile
```

## Generation + gate

`crates/core/tests/profile_matrix.rs`:
- For each entry: load `profile.json` via `Profile::from_json` (so each is schema-valid by construction);
  import `examples/part.gcode` with `profile.gcode_import_params()`; `simulate`; `verify` with
  `profile.contracts()`; build a `ReviewReport`; write/drift-gate `review.json`.
- `UPDATE_PROFILE_MATRIX=1` blesses; the normal run asserts the committed goldens match and that the
  manifest covers every entry directory.

## Independent validation

Extend `tools/validate_reports.py` to validate every `conformance/profile-matrix/*/profile.json` against
`spec/dry-profile-v1.schema.json` and every `review.json` against `ReviewReport` — no `dry-core`. Already
runs in the `spec-vectors` CI job.

## Docs

A "Supported profile matrix" section in `docs/16-support-matrix.md` linking the entries; note these are
authored clean-room (provenance ledger, `docs/17`).

## Acceptance → 08·WS3

- ✅ golden profiles for a small supported firmware/printer matrix
- ✅ each profile validated and drift-gated through the review pipeline
- ✅ independently schema-validated; documented

## Work breakdown (issues)

- Epic: Slice M — golden firmware/printer profile matrix.
- M1 matrix profiles; M2 generator + drift gate (`profile_matrix.rs`) + MANIFEST; M3 Python validation +
  docs.

# Slicer corpus

Genuine, unmodified slicer output, sliced from Dry-authored parametric STL stress-case models.
Committed here for the post-slicer QA pilot writeup (`docs/25-slicer-corpus-baseline.md`); see the
design doc for the full rationale: `docs/superpowers/specs/2026-08-03-slicer-corpus-and-profiles-design.md`.

## Authority: descriptive evidence, not an oracle

**Nothing in this directory is a correctness reference.** `conformance/gallery/`, `conformance/roundtrip/`,
`conformance/gcode/`, `conformance/golden/`, and `conformance/profiles/` are all bootstrapped from the
FullControl fork and are treated as an oracle: Dry's engine is not considered correct until it reproduces
them (`conformance/README.md`). This directory is a deliberate, sibling exception to that rule.

OrcaSlicer and CuraEngine are not correctness references for Dry's IR, and nobody drift-gates
`review-gcode`'s findings against "what OrcaSlicer intended." A file here means exactly one thing:
**this is unmodified third-party slicer output, and here is what Dry's importer/verifier currently say
about it.** That is useful evidence for a pilot writeup — it is not a pass/fail gate, and no test in this
repo checks the *content* of these files, only that they still import without a hard parse error (a
regression check on the importer, not a correctness check on the g-code).

## What's here

9 possible slicer x profile x model combinations were designed (see the design doc §3); **2 combinations
proved out** in practice, both OrcaSlicer:

| Combination | Models | Status |
|---|---|---|
| OrcaSlicer 2.4.0-beta / Bambu Lab X1 Carbon 0.4 nozzle / `Bambu PLA Basic @BBL X1C` | all 6 | proven |
| OrcaSlicer 2.4.0-beta / Prusa MK4 0.4 nozzle / `Prusa Generic PLA @MK4` | `cube` only | proven |

7 files total, ~2.9 MB (`du -sh conformance/slicer-corpus/`), under the 5 MB budget.

Two further combinations were designed and attempted but did **not** ship — see `MANIFEST.json`'s
`not_shipped` array for the exact errors and root causes:

- **Voron 2.4 350 / Klipper / ABS** — the bundled Voron process chain in this OrcaSlicer beta never sets
  `compatible_printers` on a concrete machine, so every stock pairing fails
  `2652: process not compatible with printer` before slicing starts. Working around that compatibility
  bug surfaces a second, unrelated klipper-flavor validation error. Root-caused, not resolved.
- **CuraEngine 5.13.0** — 3 attempts, all failed on missing settings the bare CLI does not fill from its
  own JSON schema defaults (`roofing_layer_count` has no default, and Cura's application-level
  quality-profile stack — which normally supplies it — is not loaded by a raw `CuraEngine slice` call).

This corpus is therefore **2 slicer/firmware/profile combinations, not the 4 originally scoped**, and it
is a **7-file, 2-combination sample**, not the 10-50-job pilot corpus `docs/09-customer-readiness.md`
describes — it is the seed a real customer pilot's corpus would be built from. See
`docs/25-slicer-corpus-baseline.md` for the full baseline write-up and per-rule finding classification.

## Regeneration

```
python3 tools/slicer_corpus/gen_models.py   # 6 stdlib-only parametric STLs
tools/slicer_corpus/slice_matrix.sh          # slices the full matrix with local slicer binaries
```

`slice_matrix.sh` is **not run in CI** — no slicer binary is installed on any CI runner (GitHub-hosted or
the idle Hetzner box). It slices every model/combination it knows how to drive into a scratch directory,
re-imports every output through `dry review-gcode --json` (import-cleanly check, not a byte-identity
diff — slicer versions drift), and diffs the result against this directory's frozen files. A maintainer
re-freezes by copying the relevant scratch outputs here and updating `MANIFEST.json` after a slicer
upgrade or a matrix change.

## Slicer versions used

- OrcaSlicer 2.4.0-beta (`/Applications/OrcaSlicer.app`)
- CuraEngine 5.13.0 (`/Applications/UltiMaker Cura.app`) — attempted, not shipped (see above)

## Filenames

`<model>__<slicer>-<profile>.gcode`, flat (no per-slicer subdirectories) — 7 files does not need a
directory tree, and a flat `MANIFEST.json` keyed by filename is easier to diff than a nested one.

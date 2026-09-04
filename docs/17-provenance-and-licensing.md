# Provenance & licensing audit

The operational companion to [`CLEANROOM.md`](CLEANROOM.md) (the policy). This page is the **auditable
ledger**: where every committed corpus came from, and what licences ship in a release — so a release can
be reviewed for provenance and dependency licensing (08·WS7 acceptance).

*(Engineering record, not legal advice — confirm with counsel before external commercial distribution.)*

## 0. Functional Source License (FSL-1.1-MIT) Distribution Policy

Dry source, binaries, wheels, npm tarballs, browser/WASM engine, and executable gallery are licensed under the **Functional Source License, Version 1.1, MIT Future License (FSL-1.1-MIT)**.

- **Developer & Internal Freedom:** Free for developers, researchers, hobbyists, internal enterprise manufacturing, prototyping, and non-competing applications.
- **Competing Use Protection:** A commercial license is required only for providing a Competing Service (e.g. offering a hosted toolpath compilation/verification API or white-labeled CAM SaaS).
- **2-Year Automatic MIT Conversion:** Every release automatically converts to the standard permissive MIT License exactly two (2) years after release date.

Dry-authored packages declare the standard SPDX identifier `FSL-1.1-MIT` across Cargo, npm, and PyPI packaging manifests. All third-party runtime dependencies remain strictly under permissive licenses (MIT, Apache-2.0, BSD-3-Clause).

## 1. Corpus provenance ledger

| Path | Origin | Clean-room status | Regenerate |
|---|---|---|---|
| `conformance/gcode/` | FullControl oracle output | output-only (no code copied) | `conformance/export.py` |
| `conformance/gallery/` | FullControl oracle (28 fixtures: 27 registry designs + distinct Overhang Challenge Plus variant) | output-only | `conformance/export.py` |
| `conformance/golden/` | FullControl oracle golden g-code | output-only | `conformance/export.py` |
| `conformance/profiles/` | FullControl oracle device profiles | output-only | `conformance/export.py` |
| `conformance/roundtrip/` | FullControl oracle | output-only | `conformance/export.py` |
| `conformance/simulate/` | FullControl oracle metrics | output-only | `conformance/export.py` |
| `conformance/oracle/` | the FullControl oracle generator itself | **dev/CI only — excluded from releases** | n/a |
| `conformance/vectors/` | authored for the Dry IR v0 spec (slice A) | **authored clean-room** | `UPDATE_VECTORS=1 cargo test -p dry-core --test spec_vectors` |
| `conformance/reports/` | authored for the report contract (slice D) | **authored clean-room** | `UPDATE_REPORTS=1 cargo test -p dry-core --test report_goldens` |
| `conformance/reports/cnc/` | authored for the P5.3 CNC slice | **authored clean-room** | `UPDATE_GOLDEN=1 cargo test -p dry-core --test cnc_pocket_e2e` |
| `spec/examples/profiles/` | authored example profiles (slice D) | **authored clean-room** | hand-maintained |
| `examples/` | authored pilot examples (slice F) | **authored clean-room** | hand-maintained |
| `conformance/slicer-corpus/` | genuine OrcaSlicer output (2 combinations), sliced locally from Dry-authored parametric STLs | **third-party output, descriptive only** (no slicer/vendor code copied or shipped, but — unlike the FullControl rows above — not a functional-output match Dry's engine is judged against; see `conformance/slicer-corpus/README.md`) | `tools/slicer_corpus/slice_matrix.sh` |
| `spec/examples/profiles/{bambu-x1c-pla,prusa-mk4-pla}.json` | authored from public manufacturer spec sheets, conservative where unpublished; five limits across these two and `ender3-pla-marlin.json` re-sourced 2026-08-04 from the **vendor slicer's own stock profile**, which OrcaSlicer embeds verbatim as a trailing comment block in every file it writes (`docs/superpowers/specs/2026-08-03-slicer-corpus-and-profiles-design.md` §4 addendum) | **authored clean-room** — the read values are configuration numbers (`retraction_speed = 60`, `machine_max_jerk_x = 9,9`), not vendor code, and no slicer source or profile file is copied or shipped | hand-maintained |

"Output-only" means Dry matches the oracle's **functional output** (g-code, metrics) for interoperability
and regression — no oracle source is copied into Dry, and the oracle is never shipped. Reserved-name
files copied from the oracle are sanitized for cross-platform checkout (`export.py`). `slicer-corpus/`
is not this: Dry never generates or is judged against these files' content, they are simply unmodified
third-party output Dry's importer/verifier report on — the shared discipline with the output-only rows
is narrower, limited to "no vendor code copied or shipped."

## 2. Dependency-license audit (what ships in a release)

All runtime dependencies are permissive (MIT / Apache-2.0 / similar). **No GPL code is linked into or
shipped in any release artifact.**

| Crate / package | Used by | Licence (typical) |
|---|---|---|
| `serde`, `serde_json` | core, CLI, bindings | MIT / Apache-2.0 |
| `miniz_oxide` | core (DEFLATE) | MIT / Apache-2.0 / Zlib |
| `libm` | core (math) | MIT / Apache-2.0 |
| `clap` | CLI | MIT / Apache-2.0 |
| `pyo3` | Python binding | MIT / Apache-2.0 |
| `wasm-bindgen` | wasm binding | MIT / Apache-2.0 |
| `jsonschema` (Python) | `tools/validate_*.py` (dev only) | MIT |
| `sha2`, `criterion` | dev-only (tests/benches) | MIT / Apache-2.0 |

The **FullControl oracle** (GPLv3) is the only copyleft dependency; it lives under `conformance/oracle/`,
is used at dev/CI time only, and is excluded from every published package and release asset.

Verify the live tree yourself:

```sh
cargo tree -e normal --workspace        # runtime dependency graph
cargo install cargo-license && cargo license   # per-crate licences (optional)
```

## 3. Audit checklist (per release)

- [ ] No new runtime dependency introduces a copyleft licence (`cargo tree -e normal`).
- [ ] The oracle (`conformance/oracle/`) is absent from the release artifacts (CLI archives, wheels, npm).
- [ ] New corpora are recorded in the ledger above with their origin + regeneration command.
- [ ] `NOTICE` / proprietary `LICENSE` are present in every customer artifact.
- [ ] Third-party MIT / Apache-2.0 notices remain attached to the corresponding vendored components.
- [ ] The public product build serves the expected gallery and WASM files and contains no package
      archives or unplanned build artifacts.
- [ ] If the optional docs-only build is used, its boundary audit excludes `/pkg`, `/gallery`, wasm,
      package archives, and SDK implementation.
- [ ] Changelog notes any licence-relevant change.

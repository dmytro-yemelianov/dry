# Provenance & licensing audit

The operational companion to [`CLEANROOM.md`](CLEANROOM.md) (the policy). This page is the **auditable
ledger**: where every committed corpus came from, and what licences ship in a release — so a release can
be reviewed for provenance and dependency licensing (08·WS7 acceptance).

*(Engineering record, not legal advice — confirm with counsel before public commercial positioning.)*

## 1. Corpus provenance ledger

| Path | Origin | Clean-room status | Regenerate |
|---|---|---|---|
| `conformance/gcode/` | FullControl oracle output | output-only (no code copied) | `conformance/export.py` |
| `conformance/gallery/` | FullControl oracle (26 exported fixtures from a 27-design registry) | output-only | `conformance/export.py` |
| `conformance/golden/` | FullControl oracle golden g-code | output-only | `conformance/export.py` |
| `conformance/profiles/` | FullControl oracle device profiles | output-only | `conformance/export.py` |
| `conformance/roundtrip/` | FullControl oracle | output-only | `conformance/export.py` |
| `conformance/simulate/` | FullControl oracle metrics | output-only | `conformance/export.py` |
| `conformance/oracle/` | the FullControl oracle generator itself | **dev/CI only — excluded from releases** | n/a |
| `conformance/vectors/` | authored for the Dry IR v0 spec (slice A) | **authored clean-room** | `UPDATE_VECTORS=1 cargo test -p dry-core --test spec_vectors` |
| `conformance/reports/` | authored for the report contract (slice D) | **authored clean-room** | `UPDATE_REPORTS=1 cargo test -p dry-core --test report_goldens` |
| `spec/examples/profiles/` | authored example profiles (slice D) | **authored clean-room** | hand-maintained |
| `examples/` | authored pilot examples (slice F) | **authored clean-room** | hand-maintained |

"Output-only" means Dry matches the oracle's **functional output** (g-code, metrics) for interoperability
and regression — no oracle source is copied into Dry, and the oracle is never shipped. Reserved-name
files copied from the oracle are sanitized for cross-platform checkout (`export.py`).

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
- [ ] `NOTICE` / `LICENSE` are present in source archives.
- [ ] Changelog notes any licence-relevant change.

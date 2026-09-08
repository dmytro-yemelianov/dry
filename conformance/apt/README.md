# APT Conformance Corpus & Provenance Ledger

This directory contains reference APT-CL programs exported from CAM systems, conformance goldens, and validation manifests per [docs/28-apt-irbcam-dialects.md](../../docs/28-apt-irbcam-dialects.md) §10.

## Provenance Ledger

| File | CAM System / Post | Build / Version | Exporter / Source | Verification Status |
|---|---|---|---|---|
| `fusion360-apt-cps/pocket_drill_arc.apt` | Autodesk Fusion 360 (`apt.cps`) | 2.0.18720 | Stock Autodesk APT Post | Verified against ISO 4343:2000 |

## Goldens

Each `.apt` input has four associated golden files generated deterministically by `dry`:
- `<part>.ir.json`: Lifted Dry IR v0 toolpath (`dry import-apt`).
- `<part>.irbcam.json`: Lowered IRBCAM target list JSON format (`dry emit --format irbcam`).
- `<part>.irbcam.csv`: Lowered IRBCAM target list CSV format (`dry emit --format irbcam-csv`).
- `<part>.roundtrip.apt`: Normalized APT-CL emission (`dry emit --format apt`).

Goldens are drift-gated by `crates/core/tests/apt_import_goldens.rs` and can be regenerated via:
```bash
UPDATE_GOLDEN=1 cargo test -p dry-core --test apt_import_goldens
```

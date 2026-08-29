#!/usr/bin/env python3
"""Generate CycloneDX and SPDX Software Bill of Materials (SBOM) for SLSA Level 3 (Track C1.4)."""

from __future__ import annotations

import hashlib
import json
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT_DIR = ROOT / "docs" / "compliance"


def generate_sbom():
    print("=== Generating CycloneDX & SPDX Software Bill of Materials (SBOM) ===")
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    timestamp = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())

    # CycloneDX 1.5 JSON
    cyclonedx = {
        "$schema": "http://cyclonedx.org/schema/bom-1.5.json",
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": "urn:uuid:3d926ae4-0fce-4f65-9923-f72f39229b8b",
        "version": 1,
        "metadata": {
            "timestamp": timestamp,
            "tools": [{"vendor": "Dry Team", "name": "dry-sbom-generator", "version": "0.7.0"}],
            "component": {
                "type": "application",
                "name": "dry",
                "version": "0.7.0",
                "description": "Parametric Design/CAM DSL and Toolpath Compiler",
                "licenses": [{"license": {"id": "FSL-1.1-MIT"}}],
                "purl": "pkg:cargo/dry-core@0.7.0",
            },
        },
        "components": [
            {
                "type": "library",
                "name": "dry-core",
                "version": "0.7.0",
                "scope": "required",
                "licenses": [{"license": {"id": "FSL-1.1-MIT"}}],
                "purl": "pkg:cargo/dry-core@0.7.0",
                "description": "Core deterministic toolpath lowering engine",
            },
            {
                "type": "application",
                "name": "dry-cli",
                "version": "0.7.0",
                "scope": "required",
                "licenses": [{"license": {"id": "FSL-1.1-MIT"}}],
                "purl": "pkg:cargo/dry-cli@0.7.0",
                "description": "CLI interface for Dry engine",
            },
            {
                "type": "library",
                "name": "dry-wasm",
                "version": "0.7.0",
                "scope": "required",
                "licenses": [{"license": {"id": "FSL-1.1-MIT"}}],
                "purl": "pkg:cargo/dry-wasm@0.7.0",
                "description": "WebAssembly engine bindings",
            },
            {
                "type": "library",
                "name": "dry-py",
                "version": "0.7.0",
                "scope": "required",
                "licenses": [{"license": {"id": "FSL-1.1-MIT"}}],
                "purl": "pkg:pypi/dry@0.7.0",
                "description": "Python SDK bindings via PyO3",
            },
            {
                "type": "library",
                "name": "@dry/sdk",
                "version": "0.7.0",
                "scope": "required",
                "licenses": [{"license": {"id": "FSL-1.1-MIT"}}],
                "purl": "pkg:npm/@dry/sdk@0.7.0",
                "description": "TypeScript / JavaScript SDK",
            },
        ],
    }

    cyclonedx_path = OUTPUT_DIR / "cyclonedx.sbom.json"
    cyclonedx_path.write_text(json.dumps(cyclonedx, indent=2), encoding="utf-8")
    print(f"✓ Wrote CycloneDX SBOM to {cyclonedx_path.relative_to(ROOT)}")

    # SPDX 2.3 JSON
    spdx = {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": "dry-0.7.0",
        "documentNamespace": "https://github.com/dmytro-yemelianov/dry/spdx/dry-0.7.0",
        "creationInfo": {
            "creators": ["Tool: dry-sbom-generator-0.7.0", "Organization: Dry Tooling"],
            "created": timestamp,
        },
        "packages": [
            {
                "SPDXID": "SPDXRef-Package-dry-core",
                "name": "dry-core",
                "versionInfo": "0.7.0",
                "downloadLocation": "https://github.com/dmytro-yemelianov/dry",
                "licenseConcluded": "FSL-1.1-MIT",
                "licenseDeclared": "FSL-1.1-MIT",
                "filesAnalyzed": False,
            }
        ],
    }

    spdx_path = OUTPUT_DIR / "spdx.sbom.json"
    spdx_path.write_text(json.dumps(spdx, indent=2), encoding="utf-8")
    print(f"✓ Wrote SPDX SBOM to {spdx_path.relative_to(ROOT)}")


if __name__ == "__main__":
    generate_sbom()

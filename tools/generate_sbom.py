#!/usr/bin/env python3
"""Generate committed CycloneDX and SPDX release-component SBOMs."""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT_DIR = ROOT / "docs" / "compliance"

manifest = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
workspace_package = manifest.split("[workspace.package]", 1)[1].split("\n[", 1)[0]


def manifest_value(name):
    match = re.search(rf'^\s*{re.escape(name)}\s*=\s*"([^"]+)"', workspace_package, re.M)
    if not match:
        raise RuntimeError(f"[workspace.package] is missing {name}")
    return match.group(1)


VERSION = manifest_value("version")
LICENSE_ID = manifest_value("license")

changelog = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
release_header = re.search(
    rf"^## \[{re.escape(VERSION)}\] - (\d{{4}}-\d{{2}}-\d{{2}})$",
    changelog,
    re.M,
)
if not release_header:
    raise RuntimeError(f"CHANGELOG.md is missing a dated [{VERSION}] release header")
RELEASE_TIMESTAMP = f"{release_header.group(1)}T00:00:00Z"

COMPONENTS = (
    ("library", "dry-core", "pkg:cargo/dry-core", "Core deterministic toolpath lowering engine"),
    ("application", "dry-cli", "pkg:cargo/dry-cli", "Rust CLI: inspect / simulate / verify / emit"),
    ("library", "dry-license", "pkg:cargo/dry-license", "Offline Ed25519 license verification"),
    ("library", "dry-llm", "pkg:cargo/dry-llm", "Prompt-to-design planner"),
    ("library", "dry-moonraker", "pkg:cargo/dry-moonraker", "Moonraker fleet synchronisation client"),
    ("library", "dry-wasm", "pkg:cargo/dry-wasm", "WebAssembly engine bindings"),
    ("library", "dry-py", "pkg:pypi/dry", "Python SDK bindings via PyO3"),
    ("library", "@dry/sdk", "pkg:npm/@dry/sdk", "TypeScript / JavaScript SDK"),
    ("application", "@dry/mcp", "pkg:npm/@dry/mcp", "Model Context Protocol server"),
)


def license_block():
    return [{"license": {"id": LICENSE_ID}}]


def generate_sbom():
    print("=== Generating CycloneDX & SPDX release-component SBOMs ===")
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    # Committed release SBOMs must reproduce byte-for-byte. The release date is
    # already the normative version date checked by scripts/check-version.sh.
    timestamp = RELEASE_TIMESTAMP

    cyclonedx_components = [
        {
            "type": kind,
            "name": name,
            "version": VERSION,
            "scope": "required",
            "licenses": license_block(),
            "purl": f"{purl}@{VERSION}",
            "description": description,
        }
        for kind, name, purl, description in COMPONENTS
    ]
    cyclonedx = {
        "$schema": "http://cyclonedx.org/schema/bom-1.5.json",
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": "urn:uuid:60cf9074-a31c-4fd7-9020-fa72c6e90062",
        "version": 1,
        "metadata": {
            "timestamp": timestamp,
            "tools": [
                {
                    "vendor": "DryMachina",
                    "name": "dry-sbom-generator",
                    "version": VERSION,
                }
            ],
            "component": {
                "type": "application",
                "name": "DryMachina",
                "version": VERSION,
                "description": "Parametric Design/CAM DSL and Toolpath Compiler",
                "licenses": license_block(),
                "purl": f"pkg:cargo/dry-core@{VERSION}",
            },
        },
        "components": cyclonedx_components,
    }
    cyclonedx_path = OUTPUT_DIR / "cyclonedx.sbom.json"
    cyclonedx_path.write_text(
        json.dumps(cyclonedx, indent=2) + "\n", encoding="utf-8"
    )
    print(f"✓ Wrote {cyclonedx_path.relative_to(ROOT)}")

    spdx_packages = [
        {
            "SPDXID": f"SPDXRef-Package-{name.lstrip('@').replace('/', '-').replace('@', '-')}",
            "name": name,
            "versionInfo": VERSION,
            "downloadLocation": "https://github.com/dmytro-yemelianov/dry",
            "licenseConcluded": LICENSE_ID,
            "licenseDeclared": LICENSE_ID,
            "filesAnalyzed": False,
        }
        for _, name, _, _ in COMPONENTS
    ]
    spdx = {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": f"DryMachina-{VERSION}",
        "documentNamespace": (
            f"https://github.com/dmytro-yemelianov/dry/spdx/DryMachina-{VERSION}"
        ),
        "creationInfo": {
            "creators": [
                f"Tool: dry-sbom-generator-{VERSION}",
                "Person: Dmytro Yemelianov",
            ],
            "created": timestamp,
        },
        "packages": spdx_packages,
    }
    spdx_path = OUTPUT_DIR / "spdx.sbom.json"
    spdx_path.write_text(json.dumps(spdx, indent=2) + "\n", encoding="utf-8")
    print(f"✓ Wrote {spdx_path.relative_to(ROOT)}")


if __name__ == "__main__":
    generate_sbom()

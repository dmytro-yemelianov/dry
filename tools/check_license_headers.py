#!/usr/bin/env python3
"""Validate repository license, attribution, and clean-room boundary compliance."""

import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def check_license_files():
    print("[1/4] Checking core license and attribution documents...")
    license_file = ROOT / "LICENSE"
    notice_file = ROOT / "NOTICE"
    authors_file = ROOT / "AUTHORS.md"

    assert license_file.exists(), "Missing root LICENSE file"
    assert "Functional Source License, Version 1.1" in license_file.read_text(encoding="utf-8"), "LICENSE is not FSL-1.1-MIT"
    print("  ✓ Root LICENSE is FSL-1.1-MIT")

    assert notice_file.exists(), "Missing root NOTICE file"
    assert "FSL-1.1-MIT" in notice_file.read_text(encoding="utf-8"), "NOTICE does not reference FSL-1.1-MIT"
    print("  ✓ Root NOTICE references FSL-1.1-MIT")

    assert authors_file.exists(), "Missing AUTHORS.md file"
    assert "Dmytro Yemelianov" in authors_file.read_text(encoding="utf-8"), "AUTHORS.md missing primary author"
    print("  ✓ AUTHORS.md contains authors & academic attribution")

def check_manifests():
    print("[2/4] Checking package manifest license declarations...")
    # py/pyproject.toml
    pyproject = (ROOT / "py" / "pyproject.toml").read_text(encoding="utf-8")
    assert 'license = "FSL-1.1-MIT"' in pyproject, "py/pyproject.toml does not declare FSL-1.1-MIT"
    print("  ✓ py/pyproject.toml declared FSL-1.1-MIT")

    # sdk/ts/package.json
    pkg_json = json.loads((ROOT / "sdk" / "ts" / "package.json").read_text(encoding="utf-8"))
    assert pkg_json.get("license") == "FSL-1.1-MIT", "sdk/ts/package.json does not declare FSL-1.1-MIT"
    print("  ✓ sdk/ts/package.json declared FSL-1.1-MIT")

def check_sbom():
    print("[3/4] Checking CycloneDX & SPDX SBOM compliance...")
    cyclonedx_path = ROOT / "docs" / "compliance" / "cyclonedx.sbom.json"
    spdx_path = ROOT / "docs" / "compliance" / "spdx.sbom.json"

    assert cyclonedx_path.exists(), "Missing CycloneDX SBOM"
    assert spdx_path.exists(), "Missing SPDX SBOM"

    cyclonedx = json.loads(cyclonedx_path.read_text(encoding="utf-8"))
    assert cyclonedx.get("metadata", {}).get("component", {}).get("licenses", [{}])[0].get("license", {}).get("id") == "FSL-1.1-MIT"
    print("  ✓ CycloneDX SBOM verified (FSL-1.1-MIT)")

    spdx = json.loads(spdx_path.read_text(encoding="utf-8"))
    assert spdx.get("packages", [{}])[0].get("licenseDeclared") == "FSL-1.1-MIT"
    print("  ✓ SPDX SBOM verified (FSL-1.1-MIT)")

def check_third_party_licenses():
    print("[4/4] Checking third-party licenses directory...")
    tp_dir = ROOT / "third_party" / "licenses"
    assert (tp_dir / "MIT.txt").exists(), "Missing third_party/licenses/MIT.txt"
    assert (tp_dir / "Apache-2.0.txt").exists(), "Missing third_party/licenses/Apache-2.0.txt"
    print("  ✓ Third-party permissive license texts preserved")

def main():
    print("=== Running Dry Licensing & Attribution Compliance Gate ===")
    try:
        check_license_files()
        check_manifests()
        check_sbom()
        check_third_party_licenses()
        print("\n=======================================================")
        print("✓ All Licensing & Attribution Compliance Checks Passed!")
        print("=======================================================")
    except AssertionError as e:
        print(f"\n[FAILED] Licensing Compliance Check Failed: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()

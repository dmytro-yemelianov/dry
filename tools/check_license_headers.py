#!/usr/bin/env python3
"""Validate repository license, attribution, and clean-room boundary compliance."""

import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LICENSE_ID = "BUSL-1.1"
LICENSED_VERSION = "0.10.0"
CHANGE_DATE = "2030-09-05"
CANONICAL_TERMS_SHA256 = (
    "464186c664e7f8ae8afa9060424b0f769fcace1a21c4c6267c0d91a8dce94a84"
)


def check_license_files():
    print("[1/4] Checking core license and attribution documents...")
    license_path = ROOT / "LICENSE"
    notice_path = ROOT / "NOTICE"
    authors_path = ROOT / "AUTHORS.md"

    license_text = license_path.read_text(encoding="utf-8")
    assert license_text.startswith("Business Source License 1.1\n"), (
        "LICENSE is not BUSL-1.1"
    )
    assert f"Licensed Work: DryMachina version {LICENSED_VERSION}" in license_text
    assert "one natural person (a \"User\")" in license_text
    assert 'one physical manufacturing or robotic machine (a "Production' in license_text
    assert "does not provide a Competing Service" in license_text
    assert f"Change Date: {CHANGE_DATE}" in license_text
    assert "Change License: MIT License" in license_text

    marker = "-----------------------------------------------------------------------------\n\n"
    assert marker in license_text, "LICENSE is missing the canonical BUSL terms separator"
    canonical_terms = license_text.split(marker, 1)[1].encode("utf-8")
    actual_hash = hashlib.sha256(canonical_terms).hexdigest()
    assert actual_hash == CANONICAL_TERMS_SHA256, (
        "canonical BUSL-1.1 terms were modified "
        f"(got sha256:{actual_hash}, expected sha256:{CANONICAL_TERMS_SHA256})"
    )
    print("  ✓ Root LICENSE has the expected parameters and canonical BUSL-1.1 terms")

    root_license = license_path.read_bytes()
    for relative in (
        "crates/core/LICENSE",
        "crates/cli/LICENSE",
        "crates/license/LICENSE",
        "crates/llm/LICENSE",
        "crates/moonraker/LICENSE",
        "py/LICENSE",
        "sdk/ts/LICENSE",
        "sdk/mcp/LICENSE",
    ):
        assert (ROOT / relative).read_bytes() == root_license, (
            f"{relative} is not byte-identical to LICENSE"
        )
    root_notice = notice_path.read_bytes()
    for relative in (
        "crates/core/NOTICE",
        "crates/cli/NOTICE",
        "crates/license/NOTICE",
        "crates/llm/NOTICE",
        "crates/moonraker/NOTICE",
        "py/NOTICE",
        "sdk/ts/NOTICE",
        "sdk/mcp/NOTICE",
    ):
        assert (ROOT / relative).read_bytes() == root_notice, (
            f"{relative} is not byte-identical to NOTICE"
        )
    assert LICENSE_ID in root_notice.decode("utf-8")
    print("  ✓ Package-local LICENSE and NOTICE copies are byte-identical")

    assert authors_path.exists(), "Missing AUTHORS.md file"
    assert "Dmytro Yemelianov" in authors_path.read_text(encoding="utf-8")
    print("  ✓ AUTHORS.md contains authors & academic attribution")


def check_manifests():
    print("[2/4] Checking package manifest license declarations...")
    manifests = {
        "py/pyproject.toml": 'license = "BUSL-1.1"',
        "sdk/ts/package.json": '"license": "BUSL-1.1"',
        "sdk/mcp/package.json": '"license": "BUSL-1.1"',
        "web/package.json": '"license": "BUSL-1.1"',
        "services/cloud/package.json": '"license": "BUSL-1.1"',
        "tools/license-issuer/package.json": '"license": "BUSL-1.1"',
        "deploy/cloudflare/package.json": '"license": "BUSL-1.1"',
    }
    for relative, expected in manifests.items():
        text = (ROOT / relative).read_text(encoding="utf-8")
        assert expected in text, f"{relative} does not declare {LICENSE_ID}"

    explicit_cargo_manifests = (
        "Cargo.toml", "crates/wasm/Cargo.toml", "crates/cloud/Cargo.toml",
        "py/Cargo.toml", "containers/verify-runner/Cargo.toml",
    )
    for relative in explicit_cargo_manifests:
        text = (ROOT / relative).read_text(encoding="utf-8")
        assert 'license = "BUSL-1.1"' in text, (
            f"{relative} does not declare {LICENSE_ID}"
        )
    for crate in ("core", "cli", "license", "llm", "moonraker"):
        relative = f"crates/{crate}/Cargo.toml"
        text = (ROOT / relative).read_text(encoding="utf-8")
        assert "license.workspace = true" in text, (
            f"{relative} does not inherit {LICENSE_ID}"
        )
    container_workflow = (
        ROOT / ".github/workflows/verify-runner.yml"
    ).read_text(encoding="utf-8")
    for expected in (
        "org.opencontainers.image.title=DryMachina Verify Runner",
        "org.opencontainers.image.description=DryMachina toolpath verification service",
        "org.opencontainers.image.licenses=BUSL-1.1",
        "org.opencontainers.image.vendor=DryMachina",
    ):
        assert expected in container_workflow, (
            f"verify-runner image metadata is missing {expected}"
        )
    print("  ✓ Package and container manifests declare DryMachina/BUSL-1.1")


def check_sbom():
    print("[3/4] Checking CycloneDX & SPDX SBOM compliance...")
    cyclonedx = json.loads(
        (ROOT / "docs/compliance/cyclonedx.sbom.json").read_text(encoding="utf-8")
    )
    components = [cyclonedx["metadata"]["component"], *cyclonedx["components"]]
    for component in components:
        actual = component["licenses"][0]["license"]["id"]
        assert actual == LICENSE_ID, (
            f"CycloneDX component {component['name']} declares {actual}"
        )

    spdx = json.loads(
        (ROOT / "docs/compliance/spdx.sbom.json").read_text(encoding="utf-8")
    )
    for package in spdx["packages"]:
        assert package["licenseDeclared"] == LICENSE_ID
        assert package["licenseConcluded"] == LICENSE_ID
    print("  ✓ CycloneDX and SPDX SBOMs declare BUSL-1.1 throughout")


def check_third_party_licenses():
    print("[4/4] Checking third-party licenses directory...")
    tp_dir = ROOT / "third_party" / "licenses"
    for filename in ("MIT.txt", "Apache-2.0.txt", "BSD-3-Clause.txt", "Zlib.txt"):
        assert (tp_dir / filename).exists(), f"Missing {filename}"
    print("  ✓ Third-party permissive license texts preserved")


def main():
    print("=== Running DryMachina Licensing & Attribution Compliance Gate ===")
    try:
        check_license_files()
        check_manifests()
        check_sbom()
        check_third_party_licenses()
        print("\n✓ All licensing and attribution compliance checks passed")
    except (AssertionError, KeyError) as exc:
        print(f"\n[FAILED] Licensing compliance check failed: {exc}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()

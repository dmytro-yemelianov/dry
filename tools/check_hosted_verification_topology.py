#!/usr/bin/env python3
"""Fail if a second public hosted-verification ingress reappears.

ADR 0003 permits one public job API in ``services/cloud`` and one private native
verifier in ``containers/verify-runner``. Pages may serve catalog helpers, but
must not implement verification directly or advertise the retired hosted MCP
stub. This source gate cannot withdraw historical Pages deployments; that is an
explicit owner-side launch blocker checked by ``check_pages_exposure.sh``.
"""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def require(path: str, needle: str) -> None:
    text = (ROOT / path).read_text(encoding="utf-8")
    assert needle in text, f"{path} is missing required topology marker: {needle!r}"


def forbid_file(path: str) -> None:
    assert not (ROOT / path).exists(), f"retired hosted-verification ingress returned: {path}"


def forbid_text(path: str, needle: str) -> None:
    text = (ROOT / path).read_text(encoding="utf-8")
    assert needle not in text, f"{path} advertises retired hosted surface: {needle!r}"


def main() -> None:
    require("services/cloud/src/index.ts", 'url.pathname === "/v1/jobs/verify"')
    require("services/cloud/src/index.ts", 'request.method !== "POST"')
    require("services/cloud/src/index.ts", "handlePostVerifyJob")

    for retired in (
        "functions/api/verify.ts",
        "functions/api/mcp.ts",
        "deploy/cloudflare/src/index.ts",
    ):
        forbid_file(retired)

    for surface in (
        "web/api.html",
        "web/src/components/RightInspector/ApiPortal.tsx",
        "scripts/build_site.sh",
    ):
        forbid_text(surface, "/api/verify")
        forbid_text(surface, "https://drymachina.com/api/mcp")

    for surface in (
        "web/api.html",
        "web/src/components/RightInspector/ApiPortal.tsx",
        "docs/18-cloudflare-publishing.md",
        "CHANGELOG.md",
    ):
        forbid_text(surface, "@drymachina/mcp")

    require("web/api.html", '"@dry/mcp"')
    require("web/src/components/RightInspector/ApiPortal.tsx", '"@dry/mcp"')

    archived = (ROOT / "crates/cloud/src/lib.rs").read_text(encoding="utf-8")
    assert '"/spike/verify"' in archived, "archived Worker spike route is missing"
    assert 'path() == "/verify"' not in archived, "archived Worker regained product /verify"

    print("hosted verification topology: one async public ingress, one private native verifier")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Validate the bounded Dependabot update policy without third-party YAML tooling."""

from pathlib import Path
import re


CONFIG = Path(__file__).parents[1] / ".github" / "dependabot.yml"
EXPECTED = {
    ("cargo", "/"),
    ("cargo", "/py"),
    ("cargo", "/crates/wasm"),
    ("cargo", "/crates/cloud"),
    ("cargo", "/containers/verify-runner"),
    ("npm", "/sdk/ts"),
    ("npm", "/sdk/mcp"),
    ("npm", "/web"),
    ("npm", "/docs/site"),
    ("npm", "/services/cloud"),
    ("npm", "/tools/license-issuer"),
    ("github-actions", "/"),
}


def fail(message: str) -> None:
    raise SystemExit(f"dependabot policy: {message}")


def main() -> None:
    text = CONFIG.read_text(encoding="utf-8")
    if not text.startswith("version: 2\nupdates:\n"):
        fail("expected a version 2 updates document")

    blocks = text.split("\n  - package-ecosystem: ")[1:]
    found: list[tuple[str, str]] = []
    for block in blocks:
        ecosystem, separator, rest = block.partition("\n")
        if not separator:
            fail("truncated package-ecosystem entry")
        directory_match = re.search(r"^    directory: (\S+)\s*$", rest, re.MULTILINE)
        if directory_match is None:
            fail(f"{ecosystem}: missing directory")
        key = (ecosystem.strip(), directory_match.group(1))
        found.append(key)

        limits = re.findall(r"^    open-pull-requests-limit: (\d+)\s*$", rest, re.MULTILINE)
        if limits != ["2"]:
            fail(f"{key}: expected exactly one open-pull-requests-limit: 2")
        if not re.search(
            r"^    groups:\n      non-major:\n        patterns: \[\"\*\"\]\n"
            r"        update-types: \[\"minor\", \"patch\"\]\s*$",
            rest,
            re.MULTILINE,
        ):
            fail(f"{key}: expected the canonical minor/patch group")

    if len(found) != len(set(found)):
        fail("duplicate ecosystem/directory entry")
    actual = set(found)
    if actual != EXPECTED:
        fail(f"entry mismatch; missing={sorted(EXPECTED - actual)}, extra={sorted(actual - EXPECTED)}")

    print(f"dependabot policy: ok ({len(found)} bounded update roots)")


if __name__ == "__main__":
    main()

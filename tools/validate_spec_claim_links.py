#!/usr/bin/env python3
"""Validate normative clause-to-claim links independently of dry-core."""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib


ROOT = Path(__file__).resolve().parents[1]
CLAIMS = ROOT / "proofs" / "claims.toml"
LINKS = ROOT / "proofs" / "spec-claim-links.toml"
CLAUSE_ID = re.compile(r"^DRY\.[A-Z][A-Z0-9_.-]*$")


def load(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        value = tomllib.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"{path.relative_to(ROOT)} must contain a TOML table")
    return value


def require_string(table: dict[str, Any], key: str, context: str, errors: list[str]) -> str:
    value = table.get(key)
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{context}.{key} must be a non-empty string")
        return ""
    return value


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--registry",
        type=Path,
        default=LINKS,
        help="claim-link TOML path (defaults to proofs/spec-claim-links.toml)",
    )
    args = parser.parse_args()
    links_path = args.registry if args.registry.is_absolute() else ROOT / args.registry
    errors: list[str] = []
    try:
        claims = load(CLAIMS)
        document = load(links_path)
    except (OSError, ValueError, tomllib.TOMLDecodeError) as error:
        print(f"error: cannot read claim-link registry: {error}", file=sys.stderr)
        return 1

    if document.get("schema_version") != 1:
        errors.append("spec-claim-links.toml schema_version must be 1")
    claim_tables = claims.get("claim")
    clause_tables = document.get("clause")
    link_tables = document.get("link")
    if not isinstance(claim_tables, list):
        errors.append("claims registry claim must be an array")
        claim_tables = []
    if not isinstance(clause_tables, list) or not clause_tables:
        errors.append("spec-claim-links clause must be a non-empty array")
        clause_tables = []
    if not isinstance(link_tables, list):
        errors.append("spec-claim-links link must be an array")
        link_tables = []

    claim_ids = {claim.get("id") for claim in claim_tables if isinstance(claim, dict)}
    clause_ids: set[str] = set()
    for index, clause in enumerate(clause_tables):
        context = f"clause[{index}]"
        if not isinstance(clause, dict):
            errors.append(f"{context} must be a table")
            continue
        clause_id = require_string(clause, "id", context, errors)
        if clause_id and not CLAUSE_ID.fullmatch(clause_id):
            errors.append(f"{context}.id has invalid format: {clause_id}")
        if clause_id in clause_ids:
            errors.append(f"duplicate clause id: {clause_id}")
        clause_ids.add(clause_id)
        for key in ("title", "source", "section", "normative_text"):
            value = require_string(clause, key, context, errors)
            if key == "source" and value:
                path = ROOT / value
                if Path(value).is_absolute() or ".." in Path(value).parts or not path.is_file():
                    errors.append(f"{context}.source is not a repository file: {value}")

    linked_claims: set[str] = set()
    for index, link in enumerate(link_tables):
        context = f"link[{index}]"
        if not isinstance(link, dict):
            errors.append(f"{context} must be a table")
            continue
        claim_id = require_string(link, "claim_id", context, errors)
        clause_id = require_string(link, "clause_id", context, errors)
        if claim_id not in claim_ids:
            errors.append(f"{context}.claim_id is not registered: {claim_id}")
        if clause_id not in clause_ids:
            errors.append(f"{context}.clause_id is not registered: {clause_id}")
        if claim_id in linked_claims:
            errors.append(f"claim has duplicate normative links: {claim_id}")
        linked_claims.add(claim_id)

    missing = sorted(claim_ids - linked_claims)
    if missing:
        errors.append("claims without normative links: " + ", ".join(missing))
    extra = sorted(linked_claims - claim_ids)
    if extra:
        errors.append("links without claims: " + ", ".join(extra))

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    print(
        f"spec claim links: ok ({len(claim_ids)} claims, "
        f"{len(clause_ids)} normative clauses)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

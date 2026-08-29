#!/usr/bin/env python3
"""Validate the Dry formal-assurance claim registry without importing dry-core."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys
from typing import Any

from jsonschema import Draft202012Validator
from jsonschema.exceptions import SchemaError

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier
    import tomli as tomllib


RELATIONS = {
    "exact",
    "bit-exact",
    "trace-exact",
    "approximate",
    "observational",
    "capability-refinement",
    "invariant-preservation",
    "rejection",
}
ABSTRACT_STATUSES = {"specified", "proved", "not-applicable"}
NUMERIC_STATUSES = {"pending", "bounded", "empirical", "not-applicable"}
REFINEMENT_STATUSES = {"pending", "checked", "not-applicable"}
SCOPES = {"abstract", "implementation"}
CLAIM_ID = re.compile(r"^FM1\.[A-Z][A-Z0-9_.-]*$")
THEOREM_NAME = re.compile(r"^[A-Za-z][A-Za-z0-9_.]*$")
PLACEHOLDER = re.compile(r"\b(?:sorry|admit)\b")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "registry",
        nargs="?",
        default="proofs/claims.toml",
        type=Path,
        help="claim registry relative to the repository root",
    )
    parser.add_argument(
        "--schema",
        default=Path("proofs/claims.schema.json"),
        type=Path,
        help="registry JSON Schema relative to the repository root",
    )
    return parser.parse_args()


def repository_root() -> Path:
    return Path(__file__).resolve().parents[1]


def load_toml(path: Path) -> dict[str, Any]:
    try:
        with path.open("rb") as handle:
            value = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ValueError(f"cannot read TOML registry {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"registry root must be a table: {path}")
    return value


def load_schema(path: Path) -> dict[str, Any]:
    try:
        with path.open(encoding="utf-8") as handle:
            value = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read JSON Schema {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"schema root must be an object: {path}")
    return value


def require_string(
    claim: dict[str, Any], field: str, claim_id: str, errors: list[str]
) -> str:
    value = claim.get(field)
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{claim_id}: {field} must be a non-empty string")
        return ""
    return value


def require_string_list(
    claim: dict[str, Any],
    field: str,
    claim_id: str,
    errors: list[str],
    *,
    non_empty: bool = False,
) -> list[str]:
    value = claim.get(field)
    if not isinstance(value, list) or any(
        not isinstance(item, str) or not item.strip() for item in value
    ):
        errors.append(f"{claim_id}: {field} must be a list of non-empty strings")
        return []
    if non_empty and not value:
        errors.append(f"{claim_id}: {field} must contain at least one path")
    return value


def resolve_repository_path(
    root: Path, raw: str, field: str, claim_id: str, errors: list[str]
) -> Path | None:
    path = Path(raw)
    if path.is_absolute() or ".." in path.parts:
        errors.append(f"{claim_id}: {field} must be a repository-relative path: {raw}")
        return None
    resolved = (root / path).resolve()
    try:
        resolved.relative_to(root)
    except ValueError:
        errors.append(f"{claim_id}: {field} escapes the repository: {raw}")
        return None
    if not resolved.is_file():
        errors.append(f"{claim_id}: {field} does not exist: {raw}")
        return None
    return resolved


def validate_claim(
    root: Path,
    claim: Any,
    index: int,
    seen_ids: set[str],
    errors: list[str],
) -> None:
    if not isinstance(claim, dict):
        errors.append(f"claim[{index}] must be a table")
        return

    claim_id = require_string(claim, "id", f"claim[{index}]", errors)
    label = claim_id or f"claim[{index}]"
    if claim_id and not CLAIM_ID.fullmatch(claim_id):
        errors.append(f"{label}: id must match {CLAIM_ID.pattern}")
    if claim_id in seen_ids:
        errors.append(f"{label}: duplicate claim id")
    seen_ids.add(claim_id)

    require_string(claim, "title", label, errors)
    # Optional, and checked against the abstract status further down: a claim may be registered with
    # no Lean model at all (ADR 0001 allows `abstract = "specified"` / `"not-applicable"`), and the
    # registry has to be able to say so without inventing a theorem to point at.
    theorem = claim.get("theorem")
    if theorem is not None and (not isinstance(theorem, str) or not theorem.strip()):
        errors.append(f"{label}: theorem must be a non-empty string when present")
        theorem = None
    if theorem and not THEOREM_NAME.fullmatch(theorem):
        errors.append(f"{label}: invalid theorem name: {theorem}")
    for field in (
        "spec_version",
        "source_dialect",
        "target_dialect",
        "numeric_domain",
    ):
        require_string(claim, field, label, errors)

    relation = require_string(claim, "relation", label, errors)
    if relation and relation not in RELATIONS:
        errors.append(f"{label}: unsupported relation: {relation}")
    scope = require_string(claim, "scope", label, errors)
    if scope and scope not in SCOPES:
        errors.append(f"{label}: unsupported scope: {scope}")

    require_string_list(claim, "assumptions", label, errors)
    require_string_list(claim, "exclusions", label, errors)
    rust_sources = require_string_list(
        claim, "rust_sources", label, errors, non_empty=True
    )
    numeric_evidence = require_string_list(
        claim, "numeric_evidence", label, errors
    )
    refinement_evidence = require_string_list(
        claim, "refinement_evidence", label, errors
    )

    lean_source = claim.get("lean_source")
    if lean_source is not None and (
        not isinstance(lean_source, str) or not lean_source.strip()
    ):
        errors.append(f"{label}: lean_source must be a non-empty string when present")
        lean_source = None
    lean_path = (
        resolve_repository_path(root, lean_source, "lean_source", label, errors)
        if lean_source
        else None
    )
    if lean_path and theorem:
        theorem_leaf = theorem.rsplit(".", 1)[-1]
        declaration = re.compile(
            rf"\b(?:theorem|lemma)\s+{re.escape(theorem_leaf)}\b"
        )
        source = lean_path.read_text(encoding="utf-8")
        if not declaration.search(source):
            errors.append(
                f"{label}: theorem {theorem} is not declared in {lean_source}"
            )

    for raw in rust_sources:
        resolve_repository_path(root, raw, "rust_sources", label, errors)
    for raw in numeric_evidence:
        resolve_repository_path(root, raw, "numeric_evidence", label, errors)
    for raw in refinement_evidence:
        resolve_repository_path(root, raw, "refinement_evidence", label, errors)

    status = claim.get("status")
    if not isinstance(status, dict):
        errors.append(f"{label}: status must be a table")
        return
    abstract = status.get("abstract")
    numeric = status.get("numeric")
    refinement = status.get("refinement")
    if abstract not in ABSTRACT_STATUSES:
        errors.append(f"{label}: invalid abstract status: {abstract!r}")
    if numeric not in NUMERIC_STATUSES:
        errors.append(f"{label}: invalid numeric status: {numeric!r}")
    if refinement not in REFINEMENT_STATUSES:
        errors.append(f"{label}: invalid refinement status: {refinement!r}")

    # A theorem name is present exactly when the abstract layer is proved. Both directions matter:
    # a `proved` claim with no theorem is unfalsifiable, and an unproved claim that still prints a
    # theorem name in the sitemap reads as verified — the failure mode ADR 0001 exists to prevent.
    if abstract == "proved":
        if not theorem:
            errors.append(f"{label}: a proved abstract claim requires a theorem")
        if not lean_source:
            errors.append(f"{label}: a proved abstract claim requires a lean_source")
    else:
        if theorem:
            errors.append(
                f"{label}: abstract status {abstract!r} must not register a theorem"
            )
        if lean_source:
            errors.append(
                f"{label}: abstract status {abstract!r} must not register a lean_source"
            )
    if numeric == "bounded" and not numeric_evidence:
        errors.append(f"{label}: bounded numeric status requires numeric evidence")
    if refinement == "checked" and not refinement_evidence:
        errors.append(f"{label}: checked refinement status requires evidence")
    if scope == "implementation":
        if abstract != "proved":
            errors.append(f"{label}: implementation claim requires abstract proof")
        if numeric not in {"bounded", "not-applicable"}:
            errors.append(
                f"{label}: implementation claim requires bounded or inapplicable "
                "numeric status"
            )
        if refinement != "checked":
            errors.append(
                f"{label}: implementation claim requires checked refinement"
            )


def validate_formal_sources(root: Path, errors: list[str]) -> None:
    formal_root = root / "formal" / "Dry"
    if not formal_root.is_dir():
        errors.append("formal/Dry does not exist")
        return
    for path in sorted(formal_root.rglob("*.lean")):
        source = path.read_text(encoding="utf-8")
        # Strip block comments /- ... -/ and line comments -- ...
        clean_source = re.sub(r"/-\s*!?(?:(?!--/).)*?-/", "", source, flags=re.DOTALL)
        clean_source = re.sub(r"--[^\n]*", "", clean_source)
        match = PLACEHOLDER.search(clean_source)
        if match:
            relative = path.relative_to(root)
            errors.append(
                f"{relative}: proof placeholder {match.group(0)!r} is forbidden"
            )


def validate_schema_instance(
    schema: dict[str, Any], registry: dict[str, Any], errors: list[str]
) -> None:
    try:
        Draft202012Validator.check_schema(schema)
    except SchemaError as error:
        errors.append(f"invalid proof claim JSON Schema: {error.message}")
        return
    validator = Draft202012Validator(schema)
    for error in sorted(
        validator.iter_errors(registry),
        key=lambda item: ".".join(str(part) for part in item.absolute_path),
    ):
        location = ".".join(str(part) for part in error.absolute_path) or "<root>"
        errors.append(f"schema {location}: {error.message}")


def main() -> int:
    args = parse_args()
    root = repository_root()
    registry_path = root / args.registry
    schema_path = root / args.schema
    errors: list[str] = []

    try:
        schema = load_schema(schema_path)
        registry = load_toml(registry_path)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 1

    if schema.get("$id") != "https://dry.dev/schemas/proof-claims-v1.schema.json":
        errors.append("proof claim schema has an unexpected or missing $id")
    validate_schema_instance(schema, registry, errors)
    if registry.get("schema_version") != 1:
        errors.append("schema_version must be 1")
    claims = registry.get("claim")
    if not isinstance(claims, list):
        errors.append("claim must be an array of tables")
        claims = []

    seen_ids: set[str] = set()
    for index, claim in enumerate(claims):
        validate_claim(root, claim, index, seen_ids, errors)
    validate_formal_sources(root, errors)

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1

    print(
        f"proof claims: ok ({len(claims)} claims, schema v1, no placeholders)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

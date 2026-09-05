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
PROOF_METHODS = {"kernel", "native_decide"}
LEAN_DECLARATION = re.compile(
    r"(?m)^(?:theorem|lemma)\s+(?P<name>[A-Za-z_][A-Za-z0-9_']*)\b"
)
LEAN_DECLARATION_BOUNDARY = re.compile(
    r"(?m)^(?:theorem|lemma|def|abbrev|opaque|structure|class|inductive|namespace|end)\b"
)


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


def strip_lean_comments(source: str) -> str:
    """Mask Lean comments and strings before inspecting proof tactics.

    Block comments nest in Lean. The scanner preserves newlines so declaration anchors remain
    valid, masks comments and literals, and therefore cannot treat comment/string text as a
    tactic or theorem dependency.
    """
    masked: list[str] = []
    index = 0
    length = len(source)

    def mask(character: str) -> None:
        masked.append("\n" if character == "\n" else " ")

    def mask_until(index: int, closing: str) -> int:
        """Mask an already-open literal through its closing delimiter."""
        while index < length:
            if source.startswith(closing, index):
                for character in closing:
                    mask(character)
                return index + len(closing)
            character = source[index]
            mask(character)
            index += 1
            if character == "\\" and index < length:
                mask(source[index])
                index += 1
        return index

    while index < length:
        if source.startswith("/-", index):
            depth = 0
            while index < length:
                if source.startswith("/-", index):
                    depth += 1
                    mask("/")
                    mask("-")
                    index += 2
                elif source.startswith("-/", index):
                    depth -= 1
                    mask("-")
                    mask("/")
                    index += 2
                    if depth == 0:
                        break
                else:
                    mask(source[index])
                    index += 1
            continue
        if source.startswith("--", index):
            while index < length and source[index] != "\n":
                mask(source[index])
                index += 1
            continue

        if source[index] == "r" and (
            index == 0 or not (source[index - 1].isalnum() or source[index - 1] == "_")
        ):
            quote = index + 1
            while quote < length and source[quote] == "#":
                quote += 1
            if quote < length and source[quote] == '"':
                hashes = source[index + 1 : quote]
                for character in source[index : quote + 1]:
                    mask(character)
                index = mask_until(quote + 1, '"' + hashes)
                continue
        if source[index] == '"':
            mask(source[index])
            index = mask_until(index + 1, '"')
            continue

        masked.append(source[index])
        index += 1
    return "".join(masked)


def lean_theorem_bodies(source: str) -> dict[str, str]:
    """Return exact local theorem/lemma bodies, bounded before the next declaration.

    The proof-method check must not accept a `native_decide` in an unrelated theorem or a
    comment. Lean's local fixture theorems occasionally wrap a helper theorem, so callers can
    follow only explicit references among these bounded bodies.
    """
    clean_source = strip_lean_comments(source)
    matches = list(LEAN_DECLARATION.finditer(clean_source))
    bodies: dict[str, str] = {}
    for match in matches:
        boundary = LEAN_DECLARATION_BOUNDARY.search(clean_source, match.end())
        end = boundary.start() if boundary else len(clean_source)
        bodies[match.group("name")] = clean_source[match.start() : end]
    return bodies


def theorem_reaches_native_decide(source: str, theorem_leaf: str) -> bool | None:
    """Whether a theorem or its same-file theorem dependencies use `native_decide`.

    `None` means the declaration is absent. The traversal uses whole identifiers in bounded
    declaration bodies, preventing a tactic in a later declaration or a comment from changing
    the registered proof method.
    """
    bodies = lean_theorem_bodies(source)
    if theorem_leaf not in bodies:
        return None

    def visit(name: str, visited: set[str]) -> bool:
        if name in visited:
            return False
        visited.add(name)
        body = bodies[name]
        if re.search(r"\bnative_decide\b", body):
            return True
        return any(
            re.search(rf"\b{re.escape(candidate)}\b", body)
            and visit(candidate, visited)
            for candidate in bodies
            if candidate != name
        )

    return visit(theorem_leaf, set())


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
    proof_method = claim.get("proof_method")
    if proof_method is not None and proof_method not in PROOF_METHODS:
        errors.append(f"{label}: invalid proof_method: {proof_method!r}")
    if lean_path and theorem:
        theorem_leaf = theorem.rsplit(".", 1)[-1]
        source = lean_path.read_text(encoding="utf-8")
        reaches_native_decide = theorem_reaches_native_decide(source, theorem_leaf)
        if reaches_native_decide is None:
            errors.append(
                f"{label}: theorem {theorem} is not declared in {lean_source}"
            )
        elif proof_method == "native_decide" and not reaches_native_decide:
            errors.append(
                f"{label}: proof_method native_decide does not reach native_decide "
                f"from theorem {theorem}"
            )
        elif proof_method == "kernel" and reaches_native_decide:
            errors.append(
                f"{label}: proof_method kernel conflicts with native_decide reached "
                f"from theorem {theorem}"
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
        if proof_method not in PROOF_METHODS:
            errors.append(
                f"{label}: a proved abstract claim requires proof_method kernel or native_decide"
            )
    else:
        if theorem:
            errors.append(
                f"{label}: abstract status {abstract!r} must not register a theorem"
            )
        if lean_source:
            errors.append(
                f"{label}: abstract status {abstract!r} must not register a lean_source"
            )
        if proof_method is not None:
            errors.append(
                f"{label}: abstract status {abstract!r} must not register a proof_method"
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
        clean_source = strip_lean_comments(source)
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

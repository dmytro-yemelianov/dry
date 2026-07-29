#!/usr/bin/env python3
"""Validate the feature-expansion numeric-boundary inventory without importing dry-core."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sys
from typing import Any

from jsonschema import Draft202012Validator
from jsonschema.exceptions import SchemaError

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier
    import tomli as tomllib


ROOT = Path(__file__).resolve().parents[1]
EXPECTED_MODEL = "feature-numeric-boundaries-v0"
EXPECTED_SCHEMA_ID = "https://dry.dev/schemas/numeric-boundaries-v1.schema.json"
EXPECTED_SOURCES = {
    "crates/core/src/features.rs",
    "crates/core/src/resolve.rs",
}
EXPECTED_BOUNDARIES = {
    "FM1.F64.FEATURE.POSE.FINITE",
    "FM1.F64.FEATURE.COORDINATE.INHERIT",
    "FM1.F64.FEATURE.ANGLE.DEGREES",
    "FM1.F64.FEATURE.TRIG.LIBM",
    "FM1.F64.FEATURE.COMPOSE.ROTATION",
    "FM1.F64.FEATURE.COMPOSE.TRANSLATION",
    "FM1.F64.FEATURE.APPLY.POINT",
    "FM1.F64.FEATURE.APPLY.ARC.CENTER",
    "FM1.F64.FEATURE.APPLY.ORIENTATION",
    "FM1.F64.FEATURE.ARC.CENTER.UNCHECKED",
    "FM1.F64.FEATURE.ORIENTATION.UNCHECKED",
    "FM1.F64.FEATURE.MANUAL.IDENTITY",
    "FM1.F64.FEATURE.OP.PASSTHROUGH",
}
STATUS_BY_CLASSIFICATION = {
    "exact-in-range": {"bounded", "pending"},
    "rejected": {"not-applicable"},
    "interval-bound-pending": {"pending"},
    "deterministic-but-unbounded": {"pending", "empirical"},
    "pass-through": {"not-applicable"},
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "inventory",
        nargs="?",
        default=Path("proofs/feature-numeric-boundaries-v0.toml"),
        type=Path,
        help="inventory relative to the repository root",
    )
    parser.add_argument(
        "--schema",
        default=Path("proofs/numeric-boundaries.schema.json"),
        type=Path,
        help="inventory JSON Schema relative to the repository root",
    )
    parser.add_argument(
        "--claims",
        default=Path("proofs/claims.toml"),
        type=Path,
        help="claim registry relative to the repository root",
    )
    return parser.parse_args()


def load_toml(path: Path, label: str) -> dict[str, Any]:
    try:
        with path.open("rb") as handle:
            value = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ValueError(f"cannot read {label} {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{label} root must be a table: {path}")
    return value


def load_json(path: Path) -> dict[str, Any]:
    try:
        with path.open(encoding="utf-8") as handle:
            value = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read JSON Schema {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"schema root must be an object: {path}")
    return value


def root_path(path: Path) -> Path:
    return path if path.is_absolute() else ROOT / path


def repository_file(raw: object, context: str, errors: list[str]) -> Path | None:
    if not isinstance(raw, str) or not raw:
        errors.append(f"{context} must be a nonempty repository-relative path")
        return None
    path = Path(raw)
    if path.is_absolute() or ".." in path.parts:
        errors.append(f"{context} must be a repository-relative path: {raw}")
        return None
    resolved = (ROOT / path).resolve()
    try:
        resolved.relative_to(ROOT)
    except ValueError:
        errors.append(f"{context} escapes the repository: {raw}")
        return None
    if not resolved.is_file():
        errors.append(f"{context} does not exist: {raw}")
        return None
    return resolved


def validate_schema(
    schema: dict[str, Any], inventory: dict[str, Any], errors: list[str]
) -> None:
    try:
        Draft202012Validator.check_schema(schema)
    except SchemaError as error:
        errors.append(f"invalid numeric-boundary JSON Schema: {error.message}")
        return
    for error in sorted(
        Draft202012Validator(schema).iter_errors(inventory),
        key=lambda item: ".".join(str(part) for part in item.absolute_path),
    ):
        location = ".".join(str(part) for part in error.absolute_path) or "<root>"
        errors.append(f"schema {location}: {error.message}")


def validate_sources(
    raw_sources: object, errors: list[str]
) -> dict[str, str]:
    if not isinstance(raw_sources, list):
        errors.append("source must be an array of tables")
        return {}

    sources: dict[str, str] = {}
    for index, source in enumerate(raw_sources):
        context = f"source[{index}]"
        if not isinstance(source, dict):
            errors.append(f"{context} must be a table")
            continue
        raw_path = source.get("path")
        digest = source.get("sha256")
        hash_mode = source.get("hash_mode")
        if not isinstance(raw_path, str):
            continue
        if raw_path in sources:
            errors.append(f"duplicate source path: {raw_path}")
            continue
        sources[raw_path] = digest if isinstance(digest, str) else ""
        path = repository_file(raw_path, f"{context}.path", errors)
        if path is not None and isinstance(digest, str):
            content = path.read_bytes()
            if hash_mode == "slice":
                text = content.decode("utf-8")
                anchor_start = source.get("anchor_start")
                anchor_end = source.get("anchor_end")
                if not isinstance(anchor_start, str) or not isinstance(
                    anchor_end, str
                ):
                    continue
                start_count = text.count(anchor_start)
                end_count = text.count(anchor_end)
                if start_count != 1 or end_count != 1:
                    errors.append(
                        f"{context}: slice anchors must occur exactly once; "
                        f"found start={start_count}, end={end_count}"
                    )
                    continue
                start = text.index(anchor_start)
                end = text.index(anchor_end)
                if end <= start:
                    errors.append(f"{context}: slice end precedes its start")
                    continue
                content = text[start:end].encode("utf-8")
            actual = hashlib.sha256(content).hexdigest()
            if actual != digest:
                errors.append(
                    f"{raw_path} changed without numeric-boundary review: "
                    f"expected {digest}, got {actual}"
                )

    source_paths = set(sources)
    if source_paths != EXPECTED_SOURCES:
        missing = sorted(EXPECTED_SOURCES - source_paths)
        extra = sorted(source_paths - EXPECTED_SOURCES)
        if missing:
            errors.append("missing pinned sources: " + ", ".join(missing))
        if extra:
            errors.append("unexpected pinned sources: " + ", ".join(extra))
    return sources


def claim_links(
    claims: dict[str, Any], errors: list[str]
) -> dict[str, set[str]]:
    raw_claims = claims.get("claim")
    if not isinstance(raw_claims, list):
        errors.append("claim registry claim must be an array")
        return {}
    links: dict[str, set[str]] = {}
    for index, claim in enumerate(raw_claims):
        if not isinstance(claim, dict):
            continue
        claim_id = claim.get("id")
        if not isinstance(claim_id, str):
            continue
        raw_links = claim.get("numeric_boundaries", [])
        if not isinstance(raw_links, list) or any(
            not isinstance(item, str) for item in raw_links
        ):
            errors.append(
                f"claim[{index}] {claim_id}: numeric_boundaries must be strings"
            )
            raw_links = []
        links[claim_id] = set(raw_links)
    return links


def validate_boundaries(
    raw_boundaries: object,
    sources: dict[str, str],
    claims: dict[str, set[str]],
    errors: list[str],
) -> None:
    if not isinstance(raw_boundaries, list):
        errors.append("boundary must be an array of tables")
        return

    ids: set[str] = set()
    backlinks: dict[str, set[str]] = {}
    for index, boundary in enumerate(raw_boundaries):
        context = f"boundary[{index}]"
        if not isinstance(boundary, dict):
            errors.append(f"{context} must be a table")
            continue
        boundary_id = boundary.get("id")
        if not isinstance(boundary_id, str):
            continue
        context = boundary_id
        if boundary_id in ids:
            errors.append(f"{context}: duplicate boundary id")
        ids.add(boundary_id)

        classification = boundary.get("classification")
        numeric_status = boundary.get("numeric_status")
        allowed_statuses = STATUS_BY_CLASSIFICATION.get(
            classification if isinstance(classification, str) else ""
        )
        if allowed_statuses is not None and numeric_status not in allowed_statuses:
            errors.append(
                f"{context}: {classification} requires numeric_status in "
                f"{sorted(allowed_statuses)}, got {numeric_status!r}"
            )
        evidence = boundary.get("evidence")
        if numeric_status == "bounded" and not evidence:
            errors.append(f"{context}: bounded status requires evidence")
        if classification == "exact-in-range" and not boundary.get("exact_envelope"):
            errors.append(f"{context}: exact-in-range requires exact_envelope")

        source_path = boundary.get("source_path")
        if source_path not in sources:
            errors.append(
                f"{context}: source_path is not pinned by the inventory: "
                f"{source_path!r}"
            )
        source_file = repository_file(
            source_path, f"{context}.source_path", errors
        )
        anchor = boundary.get("source_anchor")
        if source_file is not None and isinstance(anchor, str) and anchor:
            occurrences = source_file.read_text(encoding="utf-8").count(anchor)
            if occurrences != 1:
                errors.append(
                    f"{context}: source_anchor must occur exactly once in "
                    f"{source_path}, found {occurrences}"
                )

        if isinstance(evidence, list):
            for raw_path in evidence:
                repository_file(raw_path, f"{context}.evidence", errors)

        raw_claim_ids = boundary.get("claim_ids")
        if not isinstance(raw_claim_ids, list):
            continue
        for claim_id in raw_claim_ids:
            if claim_id not in claims:
                errors.append(f"{context}: unknown claim id {claim_id!r}")
                continue
            backlinks.setdefault(claim_id, set()).add(boundary_id)
            if boundary_id not in claims[claim_id]:
                errors.append(
                    f"{context}: claim {claim_id} is missing reciprocal "
                    "numeric_boundaries link"
                )

    if ids != EXPECTED_BOUNDARIES:
        missing = sorted(EXPECTED_BOUNDARIES - ids)
        extra = sorted(ids - EXPECTED_BOUNDARIES)
        if missing:
            errors.append("missing required boundaries: " + ", ".join(missing))
        if extra:
            errors.append("unexpected boundaries: " + ", ".join(extra))

    for claim_id, boundary_ids in claims.items():
        unknown = sorted(boundary_ids - ids)
        if unknown:
            errors.append(
                f"{claim_id}: unknown numeric boundaries: {', '.join(unknown)}"
            )
        missing_backlinks = sorted(boundary_ids - backlinks.get(claim_id, set()))
        if missing_backlinks:
            errors.append(
                f"{claim_id}: inventory is missing reciprocal claim_ids links: "
                + ", ".join(missing_backlinks)
            )


def main() -> int:
    args = parse_args()
    errors: list[str] = []
    try:
        inventory = load_toml(root_path(args.inventory), "inventory")
        schema = load_json(root_path(args.schema))
        claims = load_toml(root_path(args.claims), "claim registry")
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    if schema.get("$id") != EXPECTED_SCHEMA_ID:
        errors.append("numeric-boundary schema has an unexpected or missing $id")
    validate_schema(schema, inventory, errors)
    if inventory.get("schema_version") != 1:
        errors.append("schema_version must be 1")
    if inventory.get("model") != EXPECTED_MODEL:
        errors.append(f"model must be {EXPECTED_MODEL!r}")

    sources = validate_sources(inventory.get("source"), errors)
    links = claim_links(claims, errors)
    validate_boundaries(inventory.get("boundary"), sources, links, errors)

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1

    boundaries = inventory["boundary"]
    statuses: dict[str, int] = {}
    for boundary in boundaries:
        status = boundary["numeric_status"]
        statuses[status] = statuses.get(status, 0) + 1
    status_summary = ", ".join(
        f"{status}={count}" for status, count in sorted(statuses.items())
    )
    print(
        f"numeric boundaries: ok ({len(boundaries)} boundaries, "
        f"{status_summary}, 2 pinned sources)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

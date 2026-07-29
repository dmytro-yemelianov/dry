#!/usr/bin/env python3
"""Validate feature numeric boundaries and profiles without importing dry-core."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
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


ROOT = Path(__file__).resolve().parents[1]
EXPECTED_MODEL = "feature-numeric-boundaries-v0"
EXPECTED_SCHEMA_ID = "https://dry.dev/schemas/numeric-boundaries-v1.schema.json"
EXPECTED_PROFILE_SCHEMA_ID = "https://dry.dev/schemas/numeric-profile-v1.schema.json"
EXPECTED_PROFILE_ID = "FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0"
EXPECTED_TARGETS = {
    "x86_64-unknown-linux-gnu",
    "wasm32-unknown-unknown",
}
EXPECTED_LIBM_CONTRACT = {
    "source_tag": "libm-v0.2.16",
    "source_commit": "dfd2203a4d6110820ad7bb65cafe1bf331a03a3d",
    "trig_accuracy_basis": "upstream-mpfr-test-policy",
    "trig_max_ulp": 1,
    "trig_contract_status": "imported-assumption",
}
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
    "FM1.F64.FEATURE.ARC.CENTER.FINITE",
    "FM1.F64.FEATURE.ORIENTATION.FINITE",
    "FM1.F64.FEATURE.MANUAL.IDENTITY",
    "FM1.F64.FEATURE.OP.PASSTHROUGH",
}
EXPECTED_LIMITS = {
    f"{EXPECTED_PROFILE_ID}.LIMIT.LOCAL_COORDINATE_MM",
    f"{EXPECTED_PROFILE_ID}.LIMIT.POSE_TRANSLATION_MM",
    f"{EXPECTED_PROFILE_ID}.LIMIT.POSE_ROTATION_DEG",
    f"{EXPECTED_PROFILE_ID}.LIMIT.ARC_CENTER_MM",
    f"{EXPECTED_PROFILE_ID}.LIMIT.ORIENTATION_COMPONENT",
    f"{EXPECTED_PROFILE_ID}.LIMIT.TRANSFORM_COMPOSITIONS",
    f"{EXPECTED_PROFILE_ID}.LIMIT.LOCAL_MULTIPLY_EXACT_RESULT",
    f"{EXPECTED_PROFILE_ID}.LIMIT.LOCAL_ADD_SUB_EXACT_RESULT",
    f"{EXPECTED_PROFILE_ID}.LIMIT.RADIAN_INTERMEDIATE",
}
EXPECTED_BUDGETS = {
    f"{EXPECTED_PROFILE_ID}.BUDGET.COORDINATE_INHERIT_BIT_ERROR",
    f"{EXPECTED_PROFILE_ID}.BUDGET.ANGLE_RAD_ABS_ERROR",
    f"{EXPECTED_PROFILE_ID}.BUDGET.TRIG_COEFFICIENT_ABS_ERROR",
    f"{EXPECTED_PROFILE_ID}.BUDGET.COMPOSE_ROTATION_COMPONENT_ABS_ERROR",
    f"{EXPECTED_PROFILE_ID}.BUDGET.COMPOSE_TRANSLATION_COMPONENT_ABS_ERROR_MM",
    f"{EXPECTED_PROFILE_ID}.BUDGET.REPEAT_ROTATION_COMPONENT_ABS_ERROR",
    f"{EXPECTED_PROFILE_ID}.BUDGET.REPEAT_TRANSLATION_COMPONENT_ABS_ERROR_MM",
    f"{EXPECTED_PROFILE_ID}.BUDGET.POINT_COMPONENT_ABS_ERROR_MM",
    f"{EXPECTED_PROFILE_ID}.BUDGET.ARC_CENTER_COMPONENT_ABS_ERROR_MM",
    f"{EXPECTED_PROFILE_ID}.BUDGET.ORIENTATION_COMPONENT_ABS_ERROR",
    f"{EXPECTED_PROFILE_ID}.BUDGET.MANUAL_IDENTITY_COMPONENT_THRESHOLD",
    f"{EXPECTED_PROFILE_ID}.BUDGET.PASSTHROUGH_BIT_ERROR",
}
EXPECTED_BINARY64_LIMITS = {
    f"{EXPECTED_PROFILE_ID}.LIMIT.LOCAL_MULTIPLY_EXACT_RESULT": (
        -(2**20),
        2**20,
    ),
    f"{EXPECTED_PROFILE_ID}.LIMIT.LOCAL_ADD_SUB_EXACT_RESULT": (
        -(2**22),
        2**22,
    ),
    f"{EXPECTED_PROFILE_ID}.LIMIT.RADIAN_INTERMEDIATE": (-7, 7),
}
EXPECTED_BINARY64_BUDGETS = {
    f"{EXPECTED_PROFILE_ID}.BUDGET.ANGLE_RAD_ABS_ERROR": 2**-46,
    f"{EXPECTED_PROFILE_ID}.BUDGET.TRIG_COEFFICIENT_ABS_ERROR": 2**-45,
    f"{EXPECTED_PROFILE_ID}.BUDGET.COMPOSE_ROTATION_COMPONENT_ABS_ERROR": 2**-29,
    f"{EXPECTED_PROFILE_ID}.BUDGET.COMPOSE_TRANSLATION_COMPONENT_ABS_ERROR_MM": 2**-28,
    f"{EXPECTED_PROFILE_ID}.BUDGET.REPEAT_ROTATION_COMPONENT_ABS_ERROR": 2**-10,
    f"{EXPECTED_PROFILE_ID}.BUDGET.REPEAT_TRANSLATION_COMPONENT_ABS_ERROR_MM": 2**29,
    f"{EXPECTED_PROFILE_ID}.BUDGET.POINT_COMPONENT_ABS_ERROR_MM": 2**-28,
    f"{EXPECTED_PROFILE_ID}.BUDGET.ARC_CENTER_COMPONENT_ABS_ERROR_MM": 2**-28,
    f"{EXPECTED_PROFILE_ID}.BUDGET.ORIENTATION_COMPONENT_ABS_ERROR": 2**-29,
}
STATUS_BY_CLASSIFICATION = {
    "exact-in-range": {"bounded", "pending"},
    "rejected": {"not-applicable"},
    "interval-bounded": {"bounded"},
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
    parser.add_argument(
        "--profile",
        default=Path("proofs/feature-planar-numeric-profile-v0.toml"),
        type=Path,
        help="numeric profile relative to the repository root",
    )
    parser.add_argument(
        "--profile-schema",
        default=Path("proofs/numeric-profile.schema.json"),
        type=Path,
        help="numeric profile JSON Schema relative to the repository root",
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
    schema: dict[str, Any],
    instance: dict[str, Any],
    label: str,
    errors: list[str],
) -> None:
    try:
        Draft202012Validator.check_schema(schema)
    except SchemaError as error:
        errors.append(f"invalid {label} JSON Schema: {error.message}")
        return
    for error in sorted(
        Draft202012Validator(schema).iter_errors(instance),
        key=lambda item: ".".join(str(part) for part in item.absolute_path),
    ):
        location = ".".join(str(part) for part in error.absolute_path) or "<root>"
        errors.append(f"{label} schema {location}: {error.message}")


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
) -> dict[str, set[str]]:
    if not isinstance(raw_boundaries, list):
        errors.append("boundary must be an array of tables")
        return {}

    ids: set[str] = set()
    backlinks: dict[str, set[str]] = {}
    profile_links: dict[str, set[str]] = {}
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
        raw_profile_entries = boundary.get("profile_entries")
        if isinstance(raw_profile_entries, list):
            profile_links[boundary_id] = {
                entry for entry in raw_profile_entries if isinstance(entry, str)
            }

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
    return profile_links


def validate_expected_ids(
    actual: set[str], expected: set[str], label: str, errors: list[str]
) -> None:
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    if missing:
        errors.append(f"missing required {label}: " + ", ".join(missing))
    if extra:
        errors.append(f"unexpected {label}: " + ", ".join(extra))


def validate_toolchain(profile: dict[str, Any], errors: list[str]) -> None:
    try:
        toolchain = load_toml(ROOT / "rust-toolchain.toml", "Rust toolchain")
        cargo_lock = load_toml(ROOT / "Cargo.lock", "Cargo lockfile")
    except ValueError as error:
        errors.append(str(error))
        return

    channel = toolchain.get("toolchain", {}).get("channel")
    if profile.get("rust_toolchain") != channel:
        errors.append(
            "numeric profile rust_toolchain does not match rust-toolchain.toml: "
            f"{profile.get('rust_toolchain')!r} != {channel!r}"
        )
    raw_targets = profile.get("targets")
    targets = (
        {target for target in raw_targets if isinstance(target, str)}
        if isinstance(raw_targets, list)
        else set()
    )
    if targets != EXPECTED_TARGETS:
        errors.append(
            "numeric profile targets must be "
            + ", ".join(sorted(EXPECTED_TARGETS))
        )

    raw_libm = profile.get("libm")
    if not isinstance(raw_libm, dict):
        return
    packages = cargo_lock.get("package")
    if not isinstance(packages, list):
        errors.append("Cargo.lock package must be an array")
        return
    matches = [
        package
        for package in packages
        if isinstance(package, dict) and package.get("name") == "libm"
    ]
    if len(matches) != 1:
        errors.append(
            "Cargo.lock must contain exactly one libm package, "
            f"found {len(matches)}"
        )
        return
    locked = matches[0]
    for field in ("version", "checksum"):
        if raw_libm.get(field) != locked.get(field):
            errors.append(
                f"numeric profile libm.{field} does not match Cargo.lock: "
                f"{raw_libm.get(field)!r} != {locked.get(field)!r}"
            )
    for field, expected in EXPECTED_LIBM_CONTRACT.items():
        if raw_libm.get(field) != expected:
            errors.append(
                f"numeric profile libm.{field} does not match the imported "
                f"trigonometric contract: {raw_libm.get(field)!r} != {expected!r}"
            )


def validate_profile_entries(
    profile: dict[str, Any],
    boundary_links: dict[str, set[str]],
    errors: list[str],
) -> tuple[int, int, dict[str, int]]:
    entries: dict[str, set[str]] = {}
    raw_limits = profile.get("limit")
    raw_budgets = profile.get("budget")
    limits = raw_limits if isinstance(raw_limits, list) else []
    budgets = raw_budgets if isinstance(raw_budgets, list) else []
    limit_ids: set[str] = set()
    budget_ids: set[str] = set()
    budget_statuses: dict[str, int] = {}

    for limit in limits:
        if not isinstance(limit, dict):
            continue
        limit_id = limit.get("id")
        if not isinstance(limit_id, str):
            continue
        if limit_id in entries:
            errors.append(f"{limit_id}: duplicate profile entry id")
        limit_ids.add(limit_id)
        raw_lower = limit.get("lower")
        raw_upper = limit.get("upper")
        if (
            not isinstance(raw_lower, (int, float))
            or isinstance(raw_lower, bool)
            or not math.isfinite(raw_lower)
            or not isinstance(raw_upper, (int, float))
            or isinstance(raw_upper, bool)
            or not math.isfinite(raw_upper)
        ):
            errors.append(f"{limit_id}: lower and upper must be finite numbers")
        elif raw_lower > raw_upper:
            errors.append(f"{limit_id}: lower exceeds upper")
        if profile.get("contract") == "assurance-precondition-only":
            if limit.get("enforcement") != "assumption":
                errors.append(
                    f"{limit_id}: precondition-only profile limits must use "
                    "enforcement='assumption'"
                )
        raw_boundary_ids = limit.get("boundary_ids")
        entries[limit_id] = (
            {
                boundary_id
                for boundary_id in raw_boundary_ids
                if isinstance(boundary_id, str)
            }
            if isinstance(raw_boundary_ids, list)
            else set()
        )

    for budget in budgets:
        if not isinstance(budget, dict):
            continue
        budget_id = budget.get("id")
        if not isinstance(budget_id, str):
            continue
        if budget_id in entries:
            errors.append(f"{budget_id}: duplicate profile entry id")
        budget_ids.add(budget_id)
        status = budget.get("status")
        if isinstance(status, str):
            budget_statuses[status] = budget_statuses.get(status, 0) + 1
        ceiling = budget.get("ceiling")
        evidence = budget.get("evidence")
        if status == "pending":
            if ceiling is not None:
                errors.append(
                    f"{budget_id}: pending budget must not publish a ceiling"
                )
            if evidence:
                errors.append(
                    f"{budget_id}: pending budget must not cite evidence as a bound"
                )
        elif status in {"bounded", "policy"}:
            if (
                not isinstance(ceiling, (int, float))
                or isinstance(ceiling, bool)
                or not math.isfinite(ceiling)
                or ceiling < 0
            ):
                errors.append(
                    f"{budget_id}: {status} budget requires a finite "
                    "non-negative ceiling"
                )
            if not evidence:
                errors.append(f"{budget_id}: {status} budget requires evidence")
        if isinstance(evidence, list):
            for raw_path in evidence:
                repository_file(raw_path, f"{budget_id}.evidence", errors)
        raw_boundary_ids = budget.get("boundary_ids")
        entries[budget_id] = (
            {
                boundary_id
                for boundary_id in raw_boundary_ids
                if isinstance(boundary_id, str)
            }
            if isinstance(raw_boundary_ids, list)
            else set()
        )

    validate_expected_ids(limit_ids, EXPECTED_LIMITS, "profile limits", errors)
    validate_expected_ids(budget_ids, EXPECTED_BUDGETS, "profile budgets", errors)

    boundary_ids = set(boundary_links)
    for entry_id, linked_boundaries in entries.items():
        unknown = sorted(linked_boundaries - boundary_ids)
        if unknown:
            errors.append(
                f"{entry_id}: unknown boundary ids: {', '.join(unknown)}"
            )
        for boundary_id in linked_boundaries & boundary_ids:
            if entry_id not in boundary_links[boundary_id]:
                errors.append(
                    f"{entry_id}: boundary {boundary_id} is missing reciprocal "
                    "profile_entries link"
                )

    entry_ids = set(entries)
    for boundary_id, linked_entries in boundary_links.items():
        unknown = sorted(linked_entries - entry_ids)
        if unknown:
            errors.append(
                f"{boundary_id}: unknown profile entries: {', '.join(unknown)}"
            )
        for entry_id in linked_entries & entry_ids:
            if boundary_id not in entries[entry_id]:
                errors.append(
                    f"{boundary_id}: profile entry {entry_id} is missing "
                    "reciprocal boundary_ids link"
                )

    return len(limits), len(budgets), budget_statuses


def validate_profile_implementation_values(
    profile: dict[str, Any], errors: list[str]
) -> None:
    source = (ROOT / "crates/core/src/features.rs").read_text(encoding="utf-8")
    node_limit_match = re.search(
        r"pub const DEFAULT_MAX_EXPANDED_NODES: usize = ([0-9_]+);",
        source,
    )
    epsilon_match = re.search(r"const EPS: f64 = ([0-9.eE+-]+);", source)
    if node_limit_match is None:
        errors.append("cannot find DEFAULT_MAX_EXPANDED_NODES in features.rs")
    if epsilon_match is None:
        errors.append("cannot find Transform::is_identity EPS in features.rs")

    raw_limits = profile.get("limit")
    limits = (
        {
            item.get("id"): item
            for item in raw_limits
            if isinstance(item, dict) and isinstance(item.get("id"), str)
        }
        if isinstance(raw_limits, list)
        else {}
    )
    raw_budgets = profile.get("budget")
    budgets = (
        {
            item.get("id"): item
            for item in raw_budgets
            if isinstance(item, dict) and isinstance(item.get("id"), str)
        }
        if isinstance(raw_budgets, list)
        else {}
    )

    composition_limit = limits.get(
        f"{EXPECTED_PROFILE_ID}.LIMIT.TRANSFORM_COMPOSITIONS"
    )
    if node_limit_match is not None and composition_limit is not None:
        implementation_limit = int(node_limit_match.group(1).replace("_", ""))
        if composition_limit.get("upper") != implementation_limit:
            errors.append(
                "transform-composition profile ceiling does not match "
                f"DEFAULT_MAX_EXPANDED_NODES: "
                f"{composition_limit.get('upper')!r} != {implementation_limit}"
            )

    manual_budget = budgets.get(
        f"{EXPECTED_PROFILE_ID}.BUDGET.MANUAL_IDENTITY_COMPONENT_THRESHOLD"
    )
    if epsilon_match is not None and manual_budget is not None:
        implementation_epsilon = float(epsilon_match.group(1))
        if manual_budget.get("ceiling") != implementation_epsilon:
            errors.append(
                "manual-identity policy ceiling does not match "
                f"Transform::is_identity EPS: "
                f"{manual_budget.get('ceiling')!r} != {implementation_epsilon!r}"
            )

    for limit_id, (expected_lower, expected_upper) in EXPECTED_BINARY64_LIMITS.items():
        limit = limits.get(limit_id)
        if limit is None:
            continue
        if (
            limit.get("lower") != expected_lower
            or limit.get("upper") != expected_upper
        ):
            errors.append(
                f"{limit_id}: binary64 proof limits must remain "
                f"[{expected_lower}, {expected_upper}]"
            )

    for budget_id, expected_ceiling in EXPECTED_BINARY64_BUDGETS.items():
        budget = budgets.get(budget_id)
        if budget is None:
            continue
        if budget.get("status") != "bounded":
            errors.append(f"{budget_id}: checked binary64 budget must be bounded")
        if budget.get("ceiling") != expected_ceiling:
            errors.append(
                f"{budget_id}: binary64 proof ceiling must remain "
                f"{expected_ceiling!r}"
            )


def main() -> int:
    args = parse_args()
    errors: list[str] = []
    try:
        inventory = load_toml(root_path(args.inventory), "inventory")
        schema = load_json(root_path(args.schema))
        claims = load_toml(root_path(args.claims), "claim registry")
        profile = load_toml(root_path(args.profile), "numeric profile")
        profile_schema = load_json(root_path(args.profile_schema))
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    if schema.get("$id") != EXPECTED_SCHEMA_ID:
        errors.append("numeric-boundary schema has an unexpected or missing $id")
    if profile_schema.get("$id") != EXPECTED_PROFILE_SCHEMA_ID:
        errors.append("numeric-profile schema has an unexpected or missing $id")
    validate_schema(schema, inventory, "numeric-boundary", errors)
    validate_schema(profile_schema, profile, "numeric-profile", errors)
    if inventory.get("schema_version") != 1:
        errors.append("schema_version must be 1")
    if inventory.get("model") != EXPECTED_MODEL:
        errors.append(f"model must be {EXPECTED_MODEL!r}")
    if inventory.get("numeric_profile_id") != EXPECTED_PROFILE_ID:
        errors.append(f"numeric_profile_id must be {EXPECTED_PROFILE_ID!r}")
    if profile.get("id") != EXPECTED_PROFILE_ID:
        errors.append(f"numeric profile id must be {EXPECTED_PROFILE_ID!r}")

    sources = validate_sources(inventory.get("source"), errors)
    links = claim_links(claims, errors)
    boundary_links = validate_boundaries(
        inventory.get("boundary"), sources, links, errors
    )
    validate_toolchain(profile, errors)
    limit_count, budget_count, budget_statuses = validate_profile_entries(
        profile, boundary_links, errors
    )
    validate_profile_implementation_values(profile, errors)

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
        f"{status_summary}, 2 pinned sources; profile "
        f"{limit_count} limits/{budget_count} budgets: "
        + ", ".join(
            f"{status}={count}"
            for status, count in sorted(budget_statuses.items())
        )
        + ")"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

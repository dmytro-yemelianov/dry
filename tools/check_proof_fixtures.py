#!/usr/bin/env python3
"""Check or refresh snapshots produced by executable Lean proof fixtures."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys

from jsonschema import Draft202012Validator
from jsonschema.exceptions import SchemaError


ROOT = Path(__file__).resolve().parents[1]
FORMAL = ROOT / "formal"
TSV_SNAPSHOT = ROOT / "proofs" / "fixtures" / "l2-well-formedness-v0.tsv"
JSON_SNAPSHOT = ROOT / "proofs" / "fixtures" / "l2-logical-fixtures-v1.json"
JSON_SCHEMA = ROOT / "proofs" / "fixtures" / "l2-logical-fixtures.schema.json"
FEATURE_TSV_SNAPSHOT = ROOT / "proofs" / "fixtures" / "feature-expansion-v0.tsv"
FEATURE_REFINEMENT_SNAPSHOT = (
    ROOT / "proofs" / "fixtures" / "feature-refinement-v0.json"
)
FEATURE_REFINEMENT_SCHEMA = (
    ROOT / "proofs" / "fixtures" / "feature-refinement-fixtures.schema.json"
)
COMPOSITION_SHAPE_SNAPSHOT = (
    ROOT / "proofs" / "fixtures" / "composition-shape-refinement-v0.json"
)
COMPOSITION_SHAPE_SCHEMA = (
    ROOT
    / "proofs"
    / "fixtures"
    / "composition-shape-refinement-fixtures.schema.json"
)
NATIVE_NUMERIC_SNAPSHOT = (
    ROOT / "proofs" / "fixtures" / "native-feature-numeric-interval-v0.json"
)
NATIVE_NUMERIC_SCHEMA = (
    ROOT
    / "proofs"
    / "fixtures"
    / "native-feature-numeric-interval-fixtures.schema.json"
)
WELL_FORMED_LEAN_FIXTURE = "Dry/Tests/WellFormedFixtures.lean"
FEATURE_LEAN_FIXTURE = "Dry/Tests/ExpandFeaturesFixtures.lean"
FEATURE_REFINEMENT_LEAN_FIXTURE = "Dry/Tests/FeatureRefinementFixtures.lean"
COMPOSITION_SHAPE_LEAN_FIXTURE = "Dry/Tests/CompositionShapeFixtures.lean"
NATIVE_NUMERIC_LEAN_FIXTURE = "Dry/Tests/NativeNumericFixtures.lean"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--write",
        action="store_true",
        help="replace the committed snapshot instead of checking it",
    )
    return parser.parse_args()


def evaluate(lean_fixture: str, *arguments: str) -> str:
    lake = shutil.which("lake")
    if lake is None:
        elan_lake = Path.home() / ".elan" / "bin" / "lake"
        if not elan_lake.is_file():
            raise RuntimeError("lake is not available on PATH or under ~/.elan/bin")
        lake = str(elan_lake)
    result = subprocess.run(
        [lake, "env", "lean", "--run", lean_fixture, *arguments],
        cwd=FORMAL,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode:
        raise RuntimeError(result.stderr or result.stdout)
    return result.stdout


def validate_json_fixture(contents: str) -> dict[str, object]:
    try:
        document = json.loads(contents)
        schema = json.loads(JSON_SCHEMA.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read proof fixture JSON or schema: {error}") from error

    try:
        Draft202012Validator.check_schema(schema)
    except SchemaError as error:
        raise ValueError(f"invalid proof fixture schema: {error.message}") from error

    errors = sorted(
        Draft202012Validator(schema).iter_errors(document),
        key=lambda item: ".".join(str(part) for part in item.absolute_path),
    )
    if errors:
        messages = []
        for error in errors:
            location = ".".join(str(part) for part in error.absolute_path) or "<root>"
            messages.append(f"{location}: {error.message}")
        raise ValueError("invalid proof fixture JSON: " + "; ".join(messages))

    cases = document["cases"]
    case_ids = [case["id"] for case in cases]
    if len(case_ids) != len(set(case_ids)):
        raise ValueError("proof fixture case ids must be unique")
    return document


def validate_feature_refinement_fixture(contents: str) -> dict[str, object]:
    try:
        document = json.loads(contents)
        schema = json.loads(FEATURE_REFINEMENT_SCHEMA.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(
            f"cannot read feature-refinement fixture JSON or schema: {error}"
        ) from error

    try:
        Draft202012Validator.check_schema(schema)
    except SchemaError as error:
        raise ValueError(
            f"invalid feature-refinement fixture schema: {error.message}"
        ) from error

    errors = sorted(
        Draft202012Validator(schema).iter_errors(document),
        key=lambda item: ".".join(str(part) for part in item.absolute_path),
    )
    if errors:
        messages = []
        for error in errors:
            location = ".".join(str(part) for part in error.absolute_path) or "<root>"
            messages.append(f"{location}: {error.message}")
        raise ValueError(
            "invalid feature-refinement fixture JSON: " + "; ".join(messages)
        )

    cases = document["cases"]
    case_ids = [case["id"] for case in cases]
    if len(case_ids) != len(set(case_ids)):
        raise ValueError("feature-refinement fixture case ids must be unique")
    return document


def validate_composition_shape_fixture(contents: str) -> dict[str, object]:
    try:
        document = json.loads(contents)
        schema = json.loads(COMPOSITION_SHAPE_SCHEMA.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(
            f"cannot read composition-shape fixture JSON or schema: {error}"
        ) from error

    try:
        Draft202012Validator.check_schema(schema)
    except SchemaError as error:
        raise ValueError(
            f"invalid composition-shape fixture schema: {error.message}"
        ) from error

    errors = sorted(
        Draft202012Validator(schema).iter_errors(document),
        key=lambda item: ".".join(str(part) for part in item.absolute_path),
    )
    if errors:
        messages = []
        for error in errors:
            location = ".".join(str(part) for part in error.absolute_path) or "<root>"
            messages.append(f"{location}: {error.message}")
        raise ValueError(
            "invalid composition-shape fixture JSON: " + "; ".join(messages)
        )

    cases = document["cases"]
    case_ids = [case["id"] for case in cases]
    if len(case_ids) != len(set(case_ids)):
        raise ValueError("composition-shape fixture case ids must be unique")
    return document


def validate_native_numeric_fixture(contents: str) -> dict[str, object]:
    try:
        document = json.loads(contents)
        schema = json.loads(NATIVE_NUMERIC_SCHEMA.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(
            f"cannot read native numeric fixture JSON or schema: {error}"
        ) from error

    try:
        Draft202012Validator.check_schema(schema)
    except SchemaError as error:
        raise ValueError(
            f"invalid native numeric fixture schema: {error.message}"
        ) from error

    errors = sorted(
        Draft202012Validator(schema).iter_errors(document),
        key=lambda item: ".".join(str(part) for part in item.absolute_path),
    )
    if errors:
        messages = []
        for error in errors:
            location = ".".join(str(part) for part in error.absolute_path) or "<root>"
            messages.append(f"{location}: {error.message}")
        raise ValueError(
            "invalid native numeric fixture JSON: " + "; ".join(messages)
        )

    case_ids = [
        case["id"]
        for collection in ("pose_cases", "compose_cases")
        for case in document[collection]
    ]
    if len(case_ids) != len(set(case_ids)):
        raise ValueError("native numeric fixture case ids must be globally unique")
    return document


def main() -> int:
    args = parse_args()
    try:
        tsv_actual = evaluate(WELL_FORMED_LEAN_FIXTURE)
        json_actual = evaluate(WELL_FORMED_LEAN_FIXTURE, "--json")
        feature_tsv_actual = evaluate(FEATURE_LEAN_FIXTURE)
        feature_refinement_actual = evaluate(FEATURE_REFINEMENT_LEAN_FIXTURE)
        composition_shape_actual = evaluate(COMPOSITION_SHAPE_LEAN_FIXTURE)
        native_numeric_actual = evaluate(NATIVE_NUMERIC_LEAN_FIXTURE)
        json_document = validate_json_fixture(json_actual)
        feature_refinement_document = validate_feature_refinement_fixture(
            feature_refinement_actual
        )
        composition_shape_document = validate_composition_shape_fixture(
            composition_shape_actual
        )
        native_numeric_document = validate_native_numeric_fixture(
            native_numeric_actual
        )
    except (OSError, RuntimeError, ValueError) as error:
        print(f"error: cannot evaluate Lean proof fixtures: {error}", file=sys.stderr)
        return 1

    if "\tfixture-error\t" in tsv_actual or "\tfixture-error\t" in feature_tsv_actual:
        print("error: a Lean proof fixture disagrees with its expected result", file=sys.stderr)
        return 1

    outputs = {
        TSV_SNAPSHOT: tsv_actual,
        JSON_SNAPSHOT: json_actual,
        FEATURE_TSV_SNAPSHOT: feature_tsv_actual,
        FEATURE_REFINEMENT_SNAPSHOT: feature_refinement_actual,
        COMPOSITION_SHAPE_SNAPSHOT: composition_shape_actual,
        NATIVE_NUMERIC_SNAPSHOT: native_numeric_actual,
    }

    if args.write:
        for path, contents in outputs.items():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(contents, encoding="utf-8")
            print(f"updated {path.relative_to(ROOT)}")
        return 0

    for path, actual in outputs.items():
        try:
            expected = path.read_text(encoding="utf-8")
        except OSError as error:
            print(f"error: cannot read {path.relative_to(ROOT)}: {error}", file=sys.stderr)
            return 1
        if actual != expected:
            print(
                f"error: {path.relative_to(ROOT)} is stale; "
                "run tools/check_proof_fixtures.py --write",
                file=sys.stderr,
            )
            return 1

    feature_case_count = max(0, len(feature_tsv_actual.splitlines()) - 1)
    print(
        f"proof fixtures: ok ({len(json_document['cases'])} L2 validity cases, "
        f"{feature_case_count} feature-expansion cases, "
        f"{len(feature_refinement_document['cases'])} Rust-refinement cases, "
        f"{len(composition_shape_document['cases'])} composition-shape cases, "
        f"{len(native_numeric_document['pose_cases'])} native-pose cases, "
        f"{len(native_numeric_document['compose_cases'])} native-compose cases, "
        "TSV + schema-valid JSON)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

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
NESTED_APPLICATION_SNAPSHOT = (
    ROOT / "proofs" / "fixtures" / "nested-application-refinement-v0.json"
)
NESTED_APPLICATION_SCHEMA = (
    ROOT
    / "proofs"
    / "fixtures"
    / "nested-application-refinement-fixtures.schema.json"
)
ORIENTATION_CONTRACT_SNAPSHOT = (
    ROOT / "proofs" / "fixtures" / "orientation-contract-refinement-v0.json"
)
ORIENTATION_CONTRACT_SCHEMA = (
    ROOT
    / "proofs"
    / "fixtures"
    / "orientation-contract-refinement-fixtures.schema.json"
)
RESOLVE_ORIENTATION_SNAPSHOT = (
    ROOT / "proofs" / "fixtures" / "resolve-orientation-refinement-v0.json"
)
RESOLVE_ORIENTATION_SCHEMA = (
    ROOT
    / "proofs"
    / "fixtures"
    / "resolve-orientation-refinement-fixtures.schema.json"
)
RESOLVE_CHANNELS_SNAPSHOT = (
    ROOT / "proofs" / "fixtures" / "resolve-channels-refinement-v0.json"
)
RESOLVE_CHANNELS_SCHEMA = (
    ROOT
    / "proofs"
    / "fixtures"
    / "resolve-channels-refinement-fixtures.schema.json"
)
WELL_FORMED_LEAN_FIXTURE = "Dry/Tests/WellFormedFixtures.lean"
FEATURE_LEAN_FIXTURE = "Dry/Tests/ExpandFeaturesFixtures.lean"
FEATURE_REFINEMENT_LEAN_FIXTURE = "Dry/Tests/FeatureRefinementFixtures.lean"
COMPOSITION_SHAPE_LEAN_FIXTURE = "Dry/Tests/CompositionShapeFixtures.lean"
NATIVE_NUMERIC_LEAN_FIXTURE = "Dry/Tests/NativeNumericFixtures.lean"
NESTED_APPLICATION_LEAN_FIXTURE = "Dry/Tests/NestedApplicationFixtures.lean"
ORIENTATION_CONTRACT_LEAN_FIXTURE = "Dry/Tests/OrientationContractFixtures.lean"
RESOLVE_ORIENTATION_LEAN_FIXTURE = "Dry/Tests/ResolveOrientationFixtures.lean"
RESOLVE_CHANNELS_LEAN_FIXTURE = "Dry/Tests/ResolveChannelsFixtures.lean"




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
        for collection in (
            "pose_cases",
            "compose_cases",
            "point_application_cases",
            "xy_application_cases",
            "vector_application_cases",
        )
        for case in document[collection]
    ]
    if len(case_ids) != len(set(case_ids)):
        raise ValueError("native numeric fixture case ids must be globally unique")
    return document


def validate_nested_application_fixture(contents: str) -> dict[str, object]:
    try:
        document = json.loads(contents)
        schema = json.loads(NESTED_APPLICATION_SCHEMA.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(
            f"cannot read nested-application fixture JSON or schema: {error}"
        ) from error

    try:
        Draft202012Validator.check_schema(schema)
    except SchemaError as error:
        raise ValueError(
            f"invalid nested-application fixture schema: {error.message}"
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
            "invalid nested-application fixture JSON: " + "; ".join(messages)
        )

    cases = document["cases"]
    case_ids = [case["id"] for case in cases]
    if len(case_ids) != len(set(case_ids)):
        raise ValueError("nested-application fixture case ids must be unique")
    return document


def validate_orientation_contract_fixture(contents: str) -> dict[str, object]:
    try:
        document = json.loads(contents)
        schema = json.loads(ORIENTATION_CONTRACT_SCHEMA.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(
            f"cannot read orientation-contract fixture JSON or schema: {error}"
        ) from error

    try:
        Draft202012Validator.check_schema(schema)
    except SchemaError as error:
        raise ValueError(
            f"invalid orientation-contract fixture schema: {error.message}"
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
            "invalid orientation-contract fixture JSON: " + "; ".join(messages)
        )

    cases = document["cases"]
    case_ids = [case["id"] for case in cases]
    if len(case_ids) != len(set(case_ids)):
        raise ValueError("orientation-contract fixture case ids must be unique")
    return document


def validate_resolve_orientation_fixture(contents: str) -> dict[str, object]:
    try:
        document = json.loads(contents)
        schema = json.loads(RESOLVE_ORIENTATION_SCHEMA.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(
            f"cannot read resolve-orientation fixture JSON or schema: {error}"
        ) from error

    try:
        Draft202012Validator.check_schema(schema)
    except SchemaError as error:
        raise ValueError(
            f"invalid resolve-orientation fixture schema: {error.message}"
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
            "invalid resolve-orientation fixture JSON: " + "; ".join(messages)
        )

    cases = document["cases"]
    case_ids = [case["id"] for case in cases]
    if len(case_ids) != len(set(case_ids)):
        raise ValueError("resolve-orientation fixture case ids must be unique")
    return document


def validate_resolve_channels_fixture(contents: str) -> dict[str, object]:
    try:
        document = json.loads(contents)
        schema = json.loads(RESOLVE_CHANNELS_SCHEMA.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(
            f"cannot read resolve-channels fixture JSON or schema: {error}"
        ) from error

    try:
        Draft202012Validator.check_schema(schema)
    except SchemaError as error:
        raise ValueError(
            f"invalid resolve-channels fixture schema: {error.message}"
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
            "invalid resolve-channels fixture JSON: " + "; ".join(messages)
        )

    cases = document["cases"]
    case_ids = [case["id"] for case in cases]
    if len(case_ids) != len(set(case_ids)):
        raise ValueError("resolve-channels fixture case ids must be unique")
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
        nested_application_actual = evaluate(NESTED_APPLICATION_LEAN_FIXTURE)
        orientation_contract_actual = evaluate(ORIENTATION_CONTRACT_LEAN_FIXTURE)
        resolve_orientation_actual = evaluate(RESOLVE_ORIENTATION_LEAN_FIXTURE)
        resolve_channels_actual = evaluate(RESOLVE_CHANNELS_LEAN_FIXTURE)
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
        nested_application_document = validate_nested_application_fixture(
            nested_application_actual
        )
        orientation_contract_document = validate_orientation_contract_fixture(
            orientation_contract_actual
        )
        resolve_orientation_document = validate_resolve_orientation_fixture(
            resolve_orientation_actual
        )
        resolve_channels_document = validate_resolve_channels_fixture(
            resolve_channels_actual
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
        NESTED_APPLICATION_SNAPSHOT: nested_application_actual,
        ORIENTATION_CONTRACT_SNAPSHOT: orientation_contract_actual,
        RESOLVE_ORIENTATION_SNAPSHOT: resolve_orientation_actual,
        RESOLVE_CHANNELS_SNAPSHOT: resolve_channels_actual,
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
        f"{len(native_numeric_document['point_application_cases'])} "
        "native-point-application cases, "
        f"{len(native_numeric_document['xy_application_cases'])} "
        "native-XY-application cases, "
        f"{len(native_numeric_document['vector_application_cases'])} "
        "native-vector-application cases, "
        f"{len(nested_application_document['cases'])} "
        "nested-application cases, "
        f"{len(orientation_contract_document['cases'])} "
        "orientation-contract cases, "
        f"{len(resolve_orientation_document['cases'])} "
        "resolve-orientation cases, "
        f"{len(resolve_channels_document['cases'])} "
        "resolve-channels cases, "
        "TSV + schema-valid JSON)"
    )
    return 0




if __name__ == "__main__":
    raise SystemExit(main())

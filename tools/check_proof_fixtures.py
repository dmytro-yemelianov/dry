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
LEAN_FIXTURE = "Dry/Tests/WellFormedFixtures.lean"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--write",
        action="store_true",
        help="replace the committed snapshot instead of checking it",
    )
    return parser.parse_args()


def evaluate(*arguments: str) -> str:
    lake = shutil.which("lake")
    if lake is None:
        elan_lake = Path.home() / ".elan" / "bin" / "lake"
        if not elan_lake.is_file():
            raise RuntimeError("lake is not available on PATH or under ~/.elan/bin")
        lake = str(elan_lake)
    result = subprocess.run(
        [lake, "env", "lean", "--run", LEAN_FIXTURE, *arguments],
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


def main() -> int:
    args = parse_args()
    try:
        tsv_actual = evaluate()
        json_actual = evaluate("--json")
        json_document = validate_json_fixture(json_actual)
    except (OSError, RuntimeError, ValueError) as error:
        print(f"error: cannot evaluate Lean proof fixtures: {error}", file=sys.stderr)
        return 1

    if "\tfixture-error\t" in tsv_actual:
        print("error: a Lean proof fixture disagrees with its expected result", file=sys.stderr)
        return 1

    outputs = {
        TSV_SNAPSHOT: tsv_actual,
        JSON_SNAPSHOT: json_actual,
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

    print(
        f"proof fixtures: ok ({len(json_document['cases'])} L2 validity cases, "
        "TSV + schema-valid JSON)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Compile bounded source mutations and require the named proof fixture to kill each one."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier
    import tomli as tomllib


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "proofs" / "feature-refinement-mutations-v0.toml"
FIXTURE = ROOT / "proofs" / "fixtures" / "feature-refinement-v0.json"
COMPOSITION_SHAPE_FIXTURE = (
    ROOT / "proofs" / "fixtures" / "composition-shape-refinement-v0.json"
)
NATIVE_NUMERIC_FIXTURE = (
    ROOT / "proofs" / "fixtures" / "native-feature-numeric-interval-v0.json"
)
EXPECTED_MODEL = "feature-refinement-source-mutations-v0"
FEATURE_TEST = "feature-refinement"
NATIVE_NUMERIC_TEST = "native-numeric"
TEST_NAMES = {
    FEATURE_TEST: "rust_feature_expansion_refines_checked_lean_fixtures",
    NATIVE_NUMERIC_TEST: (
        "features::native_numeric_tests::native_f64_matches_lean_numeric_intervals"
    ),
}


@dataclass(frozen=True)
class Mutation:
    id: str
    witness: str
    test: str
    description: str
    old: str
    new: str


@dataclass(frozen=True)
class Manifest:
    source: Path
    source_sha256: str
    test: Path
    mutations: tuple[Mutation, ...]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--list",
        action="store_true",
        help="validate and list the manifest without compiling mutations",
    )
    parser.add_argument(
        "--mutation",
        action="append",
        default=[],
        metavar="ID",
        help="run only a named mutation; may be repeated",
    )
    return parser.parse_args()


def require_string(table: dict[str, object], key: str, context: str) -> str:
    value = table.get(key)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{context}.{key} must be a nonempty string")
    return value


def repository_path(value: str, field: str) -> Path:
    path = Path(value)
    if path.is_absolute() or ".." in path.parts:
        raise ValueError(f"{field} must be a repository-relative path")
    resolved = ROOT / path
    if not resolved.is_file():
        raise ValueError(f"{field} does not exist: {path}")
    return path


def load_manifest() -> Manifest:
    try:
        document = tomllib.loads(MANIFEST.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ValueError(f"cannot read mutation manifest: {error}") from error

    if document.get("schema_version") != 1:
        raise ValueError("schema_version must be 1")
    if document.get("model") != EXPECTED_MODEL:
        raise ValueError(f"model must be {EXPECTED_MODEL!r}")

    source = repository_path(
        require_string(document, "source", "<root>"), "source"
    )
    test = repository_path(require_string(document, "test", "<root>"), "test")
    source_sha256 = require_string(document, "source_sha256", "<root>")
    if len(source_sha256) != 64 or any(
        character not in "0123456789abcdef" for character in source_sha256
    ):
        raise ValueError("source_sha256 must be a lowercase SHA-256 digest")

    raw_mutations = document.get("mutation")
    if not isinstance(raw_mutations, list) or not raw_mutations:
        raise ValueError("mutation must be a nonempty array of tables")

    mutations = []
    ids = set()
    witnesses = set()
    source_text = (ROOT / source).read_text(encoding="utf-8")
    for index, raw in enumerate(raw_mutations):
        context = f"mutation[{index}]"
        if not isinstance(raw, dict):
            raise ValueError(f"{context} must be a table")
        mutation = Mutation(
            id=require_string(raw, "id", context),
            witness=require_string(raw, "witness", context),
            test=raw.get("test", FEATURE_TEST),
            description=require_string(raw, "description", context),
            old=require_string(raw, "old", context),
            new=require_string(raw, "new", context),
        )
        if not isinstance(mutation.test, str) or mutation.test not in TEST_NAMES:
            raise ValueError(
                f"{context}.test must be one of {', '.join(sorted(TEST_NAMES))}"
            )
        if mutation.id in ids:
            raise ValueError(f"duplicate mutation id {mutation.id!r}")
        if mutation.witness in witnesses:
            raise ValueError(f"duplicate mutation witness {mutation.witness!r}")
        if mutation.old == mutation.new:
            raise ValueError(f"{mutation.id}: old and new source are identical")
        count = source_text.count(mutation.old)
        if count != 1:
            raise ValueError(
                f"{mutation.id}: old source must occur exactly once, found {count}"
            )
        ids.add(mutation.id)
        witnesses.add(mutation.witness)
        mutations.append(mutation)

    actual_sha256 = hashlib.sha256((ROOT / source).read_bytes()).hexdigest()
    if actual_sha256 != source_sha256:
        raise ValueError(
            "feature source changed without mutation-manifest review: "
            f"expected {source_sha256}, got {actual_sha256}"
        )

    try:
        fixture_documents = [
            json.loads(FIXTURE.read_text(encoding="utf-8")),
            json.loads(COMPOSITION_SHAPE_FIXTURE.read_text(encoding="utf-8")),
        ]
        fixture_ids = {
            case["id"]
            for fixture_document in fixture_documents
            for case in fixture_document["cases"]
            if isinstance(case, dict)
        }
        numeric_document = json.loads(
            NATIVE_NUMERIC_FIXTURE.read_text(encoding="utf-8")
        )
        fixture_ids.update(
            case["id"]
            for collection in ("pose_cases", "compose_cases")
            for case in numeric_document[collection]
            if isinstance(case, dict)
        )
    except (KeyError, OSError, TypeError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read refinement fixture ids: {error}") from error
    missing_witnesses = sorted(witnesses - fixture_ids)
    if missing_witnesses:
        raise ValueError(
            "mutation witnesses missing from refinement fixture: "
            + ", ".join(missing_witnesses)
        )

    return Manifest(source, source_sha256, test, tuple(mutations))


def copy_minimal_workspace(destination: Path, manifest: Manifest) -> None:
    for name in ("Cargo.toml", "Cargo.lock", "LICENSE"):
        shutil.copy2(ROOT / name, destination / name)

    for member in ("core", "cli", "license", "llm", "moonraker"):
        shutil.copytree(
            ROOT / "crates" / member, destination / "crates" / member
        )
    fixture_destination = (
        destination / "proofs" / "fixtures" / "feature-refinement-v0.json"
    )
    fixture_destination.parent.mkdir(parents=True)
    shutil.copy2(FIXTURE, fixture_destination)
    shutil.copy2(
        COMPOSITION_SHAPE_FIXTURE,
        destination
        / "proofs"
        / "fixtures"
        / "composition-shape-refinement-v0.json",
    )
    shutil.copy2(
        NATIVE_NUMERIC_FIXTURE,
        destination
        / "proofs"
        / "fixtures"
        / "native-feature-numeric-interval-v0.json",
    )

    if not (destination / manifest.source).is_file():
        raise ValueError(f"minimal workspace omitted {manifest.source}")
    if not (destination / manifest.test).is_file():
        raise ValueError(f"minimal workspace omitted {manifest.test}")


def test_command(workspace: Path, test: str) -> list[str]:
    command = [
        "cargo",
        "test",
        "--manifest-path",
        str(workspace / "Cargo.toml"),
        "-p",
        "dry-core",
        "--locked",
    ]
    if test == FEATURE_TEST:
        command.extend(["--test", "feature_refinement", TEST_NAMES[test]])
    else:
        command.extend(["--lib", TEST_NAMES[test]])
    command.extend(["--", "--exact"])
    return command


def run_test(
    workspace: Path,
    witness: str | None,
    test: str,
    timeout_seconds: int = 180,
) -> subprocess.CompletedProcess[str]:
    environment = os.environ.copy()
    environment["CARGO_TERM_COLOR"] = "never"
    environment["CARGO_INCREMENTAL"] = "1"
    environment.pop("DRY_FEATURE_MUTATION_WITNESS", None)
    environment.pop("DRY_NUMERIC_MUTATION_WITNESS", None)
    if witness is not None and test == FEATURE_TEST:
        environment["DRY_FEATURE_MUTATION_WITNESS"] = witness
    elif witness is not None:
        environment["DRY_NUMERIC_MUTATION_WITNESS"] = witness
    return subprocess.run(
        test_command(workspace, test),
        cwd=workspace,
        env=environment,
        check=False,
        capture_output=True,
        text=True,
        timeout=timeout_seconds,
    )


def failure_output(result: subprocess.CompletedProcess[str]) -> str:
    return result.stdout + result.stderr


def select_mutations(
    manifest: Manifest, selected_ids: list[str]
) -> tuple[Mutation, ...]:
    if not selected_ids:
        return manifest.mutations
    requested = set(selected_ids)
    available = {mutation.id for mutation in manifest.mutations}
    unknown = sorted(requested - available)
    if unknown:
        raise ValueError(f"unknown mutation ids: {', '.join(unknown)}")
    return tuple(
        mutation for mutation in manifest.mutations if mutation.id in requested
    )


def main() -> int:
    args = parse_args()
    try:
        manifest = load_manifest()
        mutations = select_mutations(manifest, args.mutation)
    except (OSError, ValueError) as error:
        print(f"error: invalid feature mutation manifest: {error}", file=sys.stderr)
        return 1

    if args.list:
        for mutation in mutations:
            print(
                f"{mutation.id}\t{mutation.witness}\t"
                f"{mutation.test}\t{mutation.description}"
            )
        print(f"feature source mutations: {len(mutations)} declared")
        return 0

    cargo = shutil.which("cargo")
    if cargo is None:
        print("error: cargo is not available", file=sys.stderr)
        return 1

    try:
        with tempfile.TemporaryDirectory(prefix="dry-feature-mutations-") as temporary:
            workspace = Path(temporary)
            copy_minimal_workspace(workspace, manifest)
            mutated_source = workspace / manifest.source
            baseline_source = mutated_source.read_text(encoding="utf-8")

            for test in dict.fromkeys(mutation.test for mutation in mutations):
                baseline = run_test(workspace, None, test)
                if baseline.returncode:
                    print(
                        f"error: unmutated {test} test failed:\n"
                        + failure_output(baseline),
                        file=sys.stderr,
                    )
                    return 1

            failures = []
            for mutation in mutations:
                mutated_source.write_text(
                    baseline_source.replace(mutation.old, mutation.new),
                    encoding="utf-8",
                )
                result = run_test(workspace, mutation.witness, mutation.test)
                output = failure_output(result)
                killed_by_test = (
                    result.returncode != 0
                    and mutation.witness in output
                    and f"test {TEST_NAMES[mutation.test]} ... FAILED" in output
                )
                if killed_by_test:
                    print(f"KILLED\t{mutation.id}\t{mutation.witness}")
                else:
                    outcome = "SURVIVED" if result.returncode == 0 else "INVALID"
                    failures.append((mutation, outcome, output))
                    print(
                        f"{outcome}\t{mutation.id}\t{mutation.witness}",
                        file=sys.stderr,
                    )
                mutated_source.write_text(baseline_source, encoding="utf-8")

            if failures:
                for mutation, outcome, output in failures:
                    print(
                        f"\n--- {outcome}: {mutation.id} "
                        f"(witness {mutation.witness}) ---\n{output}",
                        file=sys.stderr,
                    )
                return 1
    except (OSError, subprocess.SubprocessError, ValueError) as error:
        print(f"error: cannot run feature source mutations: {error}", file=sys.stderr)
        return 1

    print(
        f"feature source mutations: ok "
        f"({len(mutations)}/{len(mutations)} killed by named proof fixtures)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

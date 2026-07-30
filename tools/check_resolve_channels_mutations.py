#!/usr/bin/env python3
"""Compile bounded resolve-channels source mutations and require the named proof fixture to kill each one."""

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
MANIFEST = ROOT / "proofs" / "resolve-channels-mutations-v0.toml"
FIXTURE = ROOT / "proofs" / "fixtures" / "resolve-channels-refinement-v0.json"
EXPECTED_MODEL = "resolve-channels-source-mutations-v0"
TEST_NAME = "native_resolve_channels_refines_generated_lean_corpus"


@dataclass(frozen=True)
class Mutation:
    id: str
    witness: str
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
        raise ValueError(f"cannot read manifest {MANIFEST.relative_to(ROOT)}: {error}") from error

    if document.get("schema_version") != 1:
        raise ValueError("manifest schema_version must be 1")
    if document.get("model") != EXPECTED_MODEL:
        raise ValueError(f"manifest model must be {EXPECTED_MODEL!r}")

    source = repository_path(require_string(document, "source", "manifest"), "source")
    test = repository_path(require_string(document, "test", "manifest"), "test")
    source_sha256 = require_string(document, "source_sha256", "manifest")

    actual_sha256 = hashlib.sha256((ROOT / source).read_bytes()).hexdigest()
    if actual_sha256 != source_sha256:
        raise ValueError(
            f"source checksum mismatch for {source}: expected {source_sha256}, got {actual_sha256}"
        )

    fixture_cases = json.loads(FIXTURE.read_text(encoding="utf-8"))["cases"]
    valid_case_ids = {case["id"] for case in fixture_cases}

    mutation_tables = document.get("mutation")
    if not isinstance(mutation_tables, list) or not mutation_tables:
        raise ValueError("manifest must contain a non-empty [[mutation]] array")

    mutations = []
    seen_ids = set()
    source_content = (ROOT / source).read_text(encoding="utf-8")
    for index, table in enumerate(mutation_tables):
        context = f"mutation[{index}]"
        if not isinstance(table, dict):
            raise ValueError(f"{context} must be a table")
        mid = require_string(table, "id", context)
        if mid in seen_ids:
            raise ValueError(f"duplicate mutation id: {mid}")
        seen_ids.add(mid)

        witness = require_string(table, "witness", context)
        if witness not in valid_case_ids:
            raise ValueError(f"{context}.witness {witness!r} is not present in {FIXTURE.relative_to(ROOT)}")

        description = require_string(table, "description", context)
        old = require_string(table, "old", context)
        new = require_string(table, "new", context)

        if old not in source_content:
            raise ValueError(f"{context}.old snippet not found in {source}")
        if source_content.count(old) != 1:
            raise ValueError(f"{context}.old snippet is not unique in {source}")

        mutations.append(Mutation(id=mid, witness=witness, description=description, old=old, new=new))

    return Manifest(
        source=source,
        source_sha256=source_sha256,
        test=test,
        mutations=tuple(mutations),
    )


def build_worktree(target_dir: Path) -> None:
    for name in ("Cargo.toml", "Cargo.lock", "LICENSE"):
        if (ROOT / name).is_file():
            shutil.copy2(ROOT / name, target_dir / name)

    for member in ("core", "cli", "license", "llm", "moonraker"):
        if (ROOT / "crates" / member).is_dir():
            shutil.copytree(
                ROOT / "crates" / member, target_dir / "crates" / member
            )

    shutil.copytree(ROOT / "proofs", target_dir / "proofs")


def run_test(workspace: Path, witness: str) -> subprocess.CompletedProcess[str]:
    command = [
        "cargo",
        "test",
        "--test",
        "resolve_channels_refinement",
        "--",
        TEST_NAME,
    ]
    return subprocess.run(
        command,
        cwd=workspace,
        capture_output=True,
        text=True,
        check=False,
    )


def failure_output(result: subprocess.CompletedProcess[str]) -> str:
    output = []
    if result.stdout.strip():
        output.append(result.stdout.strip())
    if result.stderr.strip():
        output.append(result.stderr.strip())
    return "\n".join(output)


def main() -> int:
    args = parse_args()
    try:
        manifest = load_manifest()
    except ValueError as error:
        print(f"error: invalid mutation manifest: {error}", file=sys.stderr)
        return 1

    if args.list:
        print(
            f"manifest ok: {len(manifest.mutations)} mutations in "
            f"{manifest.source} -> {manifest.test}"
        )
        for mutation in manifest.mutations:
            print(f"- {mutation.id} (witness: {mutation.witness})")
        return 0

    mutations = manifest.mutations
    if args.mutation:
        wanted = set(args.mutation)
        mutations = tuple(m for m in mutations if m.id in wanted)
        missing = wanted - {m.id for m in mutations}
        if missing:
            print(f"error: unknown mutation ids: {', '.join(sorted(missing))}", file=sys.stderr)
            return 1

    baseline_source = (ROOT / manifest.source).read_text(encoding="utf-8")

    try:
        with tempfile.TemporaryDirectory(prefix="dry-resolve-channels-mutation-") as temp_dir:
            workspace = Path(temp_dir)
            build_worktree(workspace)
            mutated_source = workspace / manifest.source

            baseline = run_test(workspace, "")
            if baseline.returncode:
                print(
                    f"error: unmutated test failed:\n" + failure_output(baseline),
                    file=sys.stderr,
                )
                return 1

            failures = []
            for mutation in mutations:
                mutated_source.write_text(
                    baseline_source.replace(mutation.old, mutation.new),
                    encoding="utf-8",
                )
                result = run_test(workspace, mutation.witness)
                output = failure_output(result)
                killed_by_test = (
                    result.returncode != 0
                    and mutation.witness in output
                    and f"test {TEST_NAME} ... FAILED" in output
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
        print(f"error: cannot run resolve channels source mutations: {error}", file=sys.stderr)
        return 1

    print(
        f"resolve channels source mutations: ok "
        f"({len(mutations)}/{len(mutations)} killed by named proof fixtures)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

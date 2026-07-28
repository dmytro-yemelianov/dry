#!/usr/bin/env python3
"""Check or refresh snapshots produced by executable Lean proof fixtures."""

from __future__ import annotations

import argparse
from pathlib import Path
import shutil
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
FORMAL = ROOT / "formal"
SNAPSHOT = ROOT / "proofs" / "fixtures" / "l2-well-formedness-v0.tsv"
LEAN_FIXTURE = "Dry/Tests/WellFormedFixtures.lean"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--write",
        action="store_true",
        help="replace the committed snapshot instead of checking it",
    )
    return parser.parse_args()


def evaluate() -> str:
    lake = shutil.which("lake")
    if lake is None:
        elan_lake = Path.home() / ".elan" / "bin" / "lake"
        if not elan_lake.is_file():
            raise RuntimeError("lake is not available on PATH or under ~/.elan/bin")
        lake = str(elan_lake)
    result = subprocess.run(
        [lake, "env", "lean", "--run", LEAN_FIXTURE],
        cwd=FORMAL,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode:
        raise RuntimeError(result.stderr or result.stdout)
    return result.stdout


def main() -> int:
    args = parse_args()
    try:
        actual = evaluate()
    except (OSError, RuntimeError) as error:
        print(f"error: cannot evaluate Lean proof fixtures: {error}", file=sys.stderr)
        return 1

    if "\tfixture-error\t" in actual:
        print("error: a Lean proof fixture disagrees with its expected result", file=sys.stderr)
        return 1

    if args.write:
        SNAPSHOT.parent.mkdir(parents=True, exist_ok=True)
        SNAPSHOT.write_text(actual, encoding="utf-8")
        print(f"updated {SNAPSHOT.relative_to(ROOT)}")
        return 0

    try:
        expected = SNAPSHOT.read_text(encoding="utf-8")
    except OSError as error:
        print(f"error: cannot read {SNAPSHOT.relative_to(ROOT)}: {error}", file=sys.stderr)
        return 1

    if actual != expected:
        print(
            "error: Lean proof fixture snapshot is stale; "
            "run tools/check_proof_fixtures.py --write",
            file=sys.stderr,
        )
        return 1

    cases = max(0, len(actual.splitlines()) - 1)
    print(f"proof fixtures: ok ({cases} L2 validity cases)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

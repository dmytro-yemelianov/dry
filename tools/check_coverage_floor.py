#!/usr/bin/env python3
"""Fail closed when an LCOV report misses a line-coverage floor."""

from __future__ import annotations

import argparse
from decimal import Decimal, InvalidOperation, ROUND_HALF_UP
from pathlib import Path
import sys


class CoverageError(ValueError):
    """The coverage input or requested floor is invalid."""


def parse_floor(raw: str) -> int:
    try:
        value = Decimal(raw)
    except InvalidOperation as exc:
        raise CoverageError(f"invalid line-coverage floor: {raw!r}") from exc
    if not value.is_finite() or value < 0 or value > 100:
        raise CoverageError("line-coverage floor must be between 0 and 100")
    if value.as_tuple().exponent < -2:
        raise CoverageError("line-coverage floor accepts at most two decimal places")
    return int(value * 100)


def parse_nonnegative_integer(raw: str, field: str, line_number: int) -> int:
    try:
        value = int(raw)
    except ValueError as exc:
        raise CoverageError(f"line {line_number}: {field} must be an integer") from exc
    if value < 0:
        raise CoverageError(f"line {line_number}: {field} must be non-negative")
    return value


def parse_lcov(path: Path) -> tuple[int, int]:
    total_found = 0
    total_hit = 0
    source: str | None = None
    found: int | None = None
    hit: int | None = None

    for line_number, raw_line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw_line.strip()
        if not line or line.startswith("TN:"):
            continue
        if line.startswith("SF:"):
            if source is not None:
                raise CoverageError(f"line {line_number}: nested SF record")
            source = line[3:]
            if not source:
                raise CoverageError(f"line {line_number}: empty SF path")
            found = None
            hit = None
        elif line.startswith("LF:"):
            if source is None:
                raise CoverageError(f"line {line_number}: LF outside an SF record")
            if found is not None:
                raise CoverageError(f"line {line_number}: duplicate LF in {source}")
            found = parse_nonnegative_integer(line[3:], "LF", line_number)
        elif line.startswith("LH:"):
            if source is None:
                raise CoverageError(f"line {line_number}: LH outside an SF record")
            if hit is not None:
                raise CoverageError(f"line {line_number}: duplicate LH in {source}")
            hit = parse_nonnegative_integer(line[3:], "LH", line_number)
        elif line == "end_of_record":
            if source is None:
                raise CoverageError(f"line {line_number}: end_of_record without SF")
            if found is None or hit is None:
                missing = "LF" if found is None else "LH"
                raise CoverageError(f"line {line_number}: missing {missing} in {source}")
            if hit > found:
                raise CoverageError(f"line {line_number}: LH exceeds LF in {source}")
            total_found += found
            total_hit += hit
            source = None
            found = None
            hit = None

    if source is not None:
        raise CoverageError(f"unterminated SF record: {source}")
    if total_found == 0:
        raise CoverageError("LCOV report contains zero instrumented lines")
    return total_hit, total_found


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lcov", type=Path, required=True)
    parser.add_argument("--minimum-line-percent", required=True)
    args = parser.parse_args()

    try:
        minimum_basis_points = parse_floor(args.minimum_line_percent)
        total_hit, total_found = parse_lcov(args.lcov)
    except (CoverageError, OSError) as exc:
        print(f"coverage floor error: {exc}", file=sys.stderr)
        return 2

    actual_basis_points = Decimal(total_hit * 10000) / Decimal(total_found)
    display = (actual_basis_points / 100).quantize(Decimal("0.01"), rounding=ROUND_HALF_UP)
    minimum = Decimal(minimum_basis_points) / 100
    print(
        f"workspace line coverage: {display:.2f}% "
        f"({total_hit}/{total_found}); required {minimum:.2f}%"
    )
    if total_hit * 10000 < total_found * minimum_basis_points:
        print("coverage floor not met", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

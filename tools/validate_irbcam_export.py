#!/usr/bin/env python3
"""Independent structural validator for IRBCAM target list exports (JSON and CSV).

Validates IRBCAM target lists according to the normative contract in
docs/28-apt-irbcam-dialects.md and spec/dry-irbcam-v0.schema.json.
Does NOT depend on dry-core (uses only Python stdlib + jsonschema).

Checks:
1. JSON Schema validation against spec/dry-irbcam-v0.schema.json.
2. Invariants matching Lean 4 model Dry.Language.Irbcam.WellFormed:
   - Target velocities are either -1 (rapid) or > 0 (feed).
   - Target types are in {0, 1} (0=linear endpoint, 1=arc midpoint).
   - Every type 1 target is immediately followed by a type 0 target.
   - Last target is not type 1.
   - Sparse vectors (toolNumber, spindleSpeed) have strictly increasing indices < len(targets).
3. Fractional precision: numeric fields carry at least 6 decimal places (vendor requirement).
4. CSV/JSON logical target parity when matching pair exists.

Usage:
    python tools/validate_irbcam_export.py [path/to/files_or_dirs...]
"""

from __future__ import annotations

import csv
import json
import math
import re
import sys
from pathlib import Path

try:
    import jsonschema
except ImportError:
    sys.exit("Error: jsonschema package required. Run: pip install jsonschema")

REPO_ROOT = Path(__file__).resolve().parent.parent
SCHEMA_PATH = REPO_ROOT / "spec" / "dry-irbcam-v0.schema.json"

NUMERIC_FIELD_RE = re.compile(r"^-?\d+\.\d{6,}$")


def load_schema() -> dict:
    if not SCHEMA_PATH.exists():
        raise FileNotFoundError(f"Missing schema at {SCHEMA_PATH}")
    with open(SCHEMA_PATH, "r", encoding="utf-8") as f:
        return json.load(f)


def validate_irbcam_json_file(path: Path, schema: dict) -> dict:
    """Validates an IRBCAM JSON file against schema and well-formedness invariants."""
    text = path.read_text(encoding="utf-8")
    data = json.loads(text)

    # 1. JSON Schema validation
    jsonschema.validate(instance=data, schema=schema)

    targets = data.get("targets", [])
    num_targets = len(targets)

    # 2. Precision check: inspect raw text representations of numeric fields
    in_spindle_speed = False
    for line_idx, line in enumerate(text.splitlines(), start=1):
        line = line.strip()
        if "\"spindleSpeed\":" in line:
            in_spindle_speed = True
        elif "\"targets\":" in line or "\"toolNumber\":" in line:
            in_spindle_speed = False

        target_fields = ("\"x\":", "\"y\":", "\"z\":", "\"rz1\":", "\"ry\":", "\"rz2\":", "\"velocity\":")
        for field in target_fields:
            if line.startswith(field):
                val_str = line[len(field):].strip().rstrip(",")
                if not NUMERIC_FIELD_RE.match(val_str):
                    raise ValueError(
                        f"{path}:{line_idx} - field {field} value '{val_str}' "
                        f"does not have required >= 6 decimal places"
                    )

        if in_spindle_speed and line.startswith("\"value\":"):
            val_str = line[len("\"value\":"):].strip().rstrip(",")
            if not NUMERIC_FIELD_RE.match(val_str):
                raise ValueError(
                    f"{path}:{line_idx} - spindleSpeed value '{val_str}' "
                    f"does not have required >= 6 decimal places"
                )

    # 3. Well-formedness checks (Irbcam.WellFormed)
    for i, t in enumerate(targets):
        vel = t["velocity"]
        if not (vel == -1 or vel > 0):
            raise ValueError(f"{path}: target {i} has invalid velocity {vel} (must be -1 or > 0)")

        k = t["type"]
        if k not in (0, 1):
            raise ValueError(f"{path}: target {i} has invalid type {k} (must be 0 or 1)")

        if k == 1:
            if i + 1 >= num_targets:
                raise ValueError(f"{path}: target {i} is type 1 (arc midpoint) at end of target list")
            if targets[i + 1]["type"] != 0:
                raise ValueError(
                    f"{path}: target {i} type 1 (arc midpoint) is not followed by type 0 target "
                    f"(target {i+1} has type {targets[i+1]['type']})"
                )

    # 4. Sparse toolNumber checks
    tool_numbers = data.get("toolNumber", [])
    prev_idx = -1
    for entry in tool_numbers:
        idx = entry["targetIndex"]
        if idx <= prev_idx:
            raise ValueError(f"{path}: toolNumber indices not strictly increasing: {prev_idx} -> {idx}")
        if idx >= num_targets:
            raise ValueError(f"{path}: toolNumber targetIndex {idx} >= targets.len ({num_targets})")
        prev_idx = idx

    # 5. Sparse spindleSpeed checks
    spindle_speeds = data.get("spindleSpeed", [])
    prev_idx = -1
    for entry in spindle_speeds:
        idx = entry["targetIndex"]
        if idx <= prev_idx:
            raise ValueError(f"{path}: spindleSpeed indices not strictly increasing: {prev_idx} -> {idx}")
        if idx >= num_targets:
            raise ValueError(f"{path}: spindleSpeed targetIndex {idx} >= targets.len ({num_targets})")
        prev_idx = idx

    return data


def validate_irbcam_csv_file(path: Path) -> list[dict]:
    """Validates an IRBCAM CSV file against well-formedness invariants."""
    targets = []
    with open(path, "r", encoding="utf-8") as f:
        reader = csv.reader(f)
        header = next(reader, None)
        expected_header = ["x", "y", "z", "rz1", "ry", "rz2", "velocity", "type"]
        if header != expected_header:
            raise ValueError(f"{path}: invalid CSV header {header}, expected {expected_header}")

        for row_idx, row in enumerate(reader, start=2):
            if len(row) != 8:
                raise ValueError(f"{path}:{row_idx}: expected 8 columns, got {len(row)}")

            for col_idx, val_str in enumerate(row[:7]):
                if not NUMERIC_FIELD_RE.match(val_str):
                    raise ValueError(
                        f"{path}:{row_idx} col {expected_header[col_idx]}: '{val_str}' "
                        f"does not have required >= 6 decimal places"
                    )

            target = {
                "x": float(row[0]),
                "y": float(row[1]),
                "z": float(row[2]),
                "rz1": float(row[3]),
                "ry": float(row[4]),
                "rz2": float(row[5]),
                "velocity": float(row[6]),
                "type": int(row[7]),
            }
            targets.append(target)

    num_targets = len(targets)
    for i, t in enumerate(targets):
        vel = t["velocity"]
        if not (vel == -1 or vel > 0):
            raise ValueError(f"{path}: target {i} has invalid velocity {vel} (must be -1 or > 0)")

        k = t["type"]
        if k not in (0, 1):
            raise ValueError(f"{path}: target {i} has invalid type {k} (must be 0 or 1)")

        if k == 1:
            if i + 1 >= num_targets:
                raise ValueError(f"{path}: target {i} is type 1 (arc midpoint) at end of target list")
            if targets[i + 1]["type"] != 0:
                raise ValueError(
                    f"{path}: target {i} type 1 (arc midpoint) is not followed by type 0 target "
                    f"(target {i+1} has type {targets[i+1]['type']})"
                )

    return targets


def compare_json_csv_targets(json_data: dict, csv_targets: list[dict], base_name: str) -> None:
    """Verifies that JSON and CSV representations carry identical targets."""
    json_targets = json_data.get("targets", [])
    if len(json_targets) != len(csv_targets):
        raise ValueError(
            f"Target count mismatch between {base_name}.irbcam.json ({len(json_targets)}) "
            f"and {base_name}.irbcam.csv ({len(csv_targets)})"
        )

    for i, (jt, ct) in enumerate(zip(json_targets, csv_targets)):
        for key in ("x", "y", "z", "rz1", "ry", "rz2", "velocity"):
            if not math.isclose(jt[key], ct[key], rel_tol=1e-9, abs_tol=1e-6):
                raise ValueError(
                    f"Target {i} field '{key}' mismatch in {base_name}: "
                    f"JSON={jt[key]} vs CSV={ct[key]}"
                )
        if jt["type"] != ct["type"]:
            raise ValueError(
                f"Target {i} type mismatch in {base_name}: JSON={jt['type']} vs CSV={ct['type']}"
            )


def find_export_files(paths: list[Path]) -> list[Path]:
    collected = []
    for p in paths:
        if p.is_file():
            if ("irbcam" in p.name) and (p.name.endswith(".json") or p.name.endswith(".csv")):
                collected.append(p)
        elif p.is_dir():
            for f in p.glob("**/*"):
                if f.is_file() and ("irbcam" in f.name) and (f.name.endswith(".json") or f.name.endswith(".csv")):
                    collected.append(f)
    return sorted(set(collected))


def main() -> int:
    args = [Path(a) for a in sys.argv[1:]]
    if not args:
        # Default search in conformance/
        args = [REPO_ROOT / "conformance"]

    files = find_export_files(args)
    if not files:
        print("No IRBCAM export files (*irbcam*.json / *irbcam*.csv) found to validate.")
        return 0

    schema = load_schema()
    validated_json: dict[str, dict] = {}
    validated_csv: dict[str, list] = {}
    errors: list[str] = []

    print(f"Validating {len(files)} IRBCAM export file(s)...")
    for f in files:
        try:
            base_key = str(f.with_suffix(""))
            if base_key.endswith(".irbcam"):
                base_key = base_key[:-7]
            if f.name.endswith(".json"):
                data = validate_irbcam_json_file(f, schema)
                validated_json[base_key] = data
                print(f"  OK (JSON): {f.relative_to(REPO_ROOT)}")
            elif f.name.endswith(".csv"):
                targets = validate_irbcam_csv_file(f)
                validated_csv[base_key] = targets
                print(f"  OK (CSV):  {f.relative_to(REPO_ROOT)}")
        except Exception as e:
            errors.append(str(e))
            print(f"  FAIL:      {f.relative_to(REPO_ROOT)}: {e}")

    # Check parity on matched JSON/CSV pairs
    common_bases = set(validated_json.keys()) & set(validated_csv.keys())
    for base in common_bases:
        try:
            compare_json_csv_targets(validated_json[base], validated_csv[base], Path(base).name)
            print(f"  PARITY OK: {Path(base).name}.irbcam.{{json,csv}}")
        except Exception as e:
            errors.append(str(e))
            print(f"  PARITY FAIL: {Path(base).name}: {e}")

    if errors:
        print(f"\n{len(errors)} error(s) detected during IRBCAM export validation.")
        return 1

    print("\nAll IRBCAM exports valid and conform to schema and well-formedness invariants.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

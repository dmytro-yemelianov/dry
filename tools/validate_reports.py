#!/usr/bin/env python3
"""Independent schema validator for Dry's report outputs and example profiles.

Validates every golden report under `conformance/reports/` against the matching subschema in
`spec/dry-reports-v1.schema.json`, and every example profile under `spec/examples/profiles/` against
`spec/dry-profile-v1.schema.json`. Uses only `jsonschema` (no `dry-core`), so it independently confirms
that the published schemas actually describe the engine's real output (see docs/11-profiles-and-reports.md).

Usage:
    python tools/validate_reports.py [repo-root]
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

# golden filename -> the $def in the reports schema it must satisfy.
REPORT_KINDS = {
    "verify.json": "VerifyReport",
    "review.json": "ReviewReport",
    "trace.json": "TraceReport",
    "forensics.json": "ForensicsReport",
}


def subschema(full: dict, defname: str) -> dict:
    """A standalone schema that validates a document against `#/$defs/<defname>`."""
    return {
        "$schema": full.get("$schema", "https://json-schema.org/draft/2020-12/schema"),
        "$ref": f"#/$defs/{defname}",
        "$defs": full["$defs"],
    }


def main(argv: list[str]) -> int:
    try:
        from jsonschema import Draft202012Validator
    except ImportError:
        print("error: jsonschema is required — pip install -r tools/requirements.txt", file=sys.stderr)
        return 2

    root = Path(argv[1]).resolve() if len(argv) > 1 else Path.cwd()
    reports_schema = json.loads((root / "spec" / "dry-reports-v1.schema.json").read_text())
    profile_schema = json.loads((root / "spec" / "dry-profile-v1.schema.json").read_text())
    profile_validator = Draft202012Validator(profile_schema)
    report_validators = {
        name: Draft202012Validator(subschema(reports_schema, defname))
        for name, defname in REPORT_KINDS.items()
    }

    errors: list[str] = []
    n_reports = 0
    n_profiles = 0

    reports_dir = root / "conformance" / "reports"
    for case_dir in sorted(p for p in reports_dir.iterdir() if p.is_dir()):
        for fname, validator in report_validators.items():
            path = case_dir / fname
            if not path.exists():
                continue
            doc = json.loads(path.read_text())
            found = False
            for e in sorted(validator.iter_errors(doc), key=lambda e: list(e.path)):
                errors.append(f"[{case_dir.name}/{fname}] {e.message}")
                found = True
            if not found:
                n_reports += 1

    profiles_dir = root / "spec" / "examples" / "profiles"
    for path in sorted(profiles_dir.glob("*.json")):
        doc = json.loads(path.read_text())
        found = False
        for e in sorted(profile_validator.iter_errors(doc), key=lambda e: list(e.path)):
            errors.append(f"[profiles/{path.name}] {e.message}")
            found = True
        if not found:
            n_profiles += 1

    if errors:
        print(f"FAIL — {len(errors)} schema problem(s):", file=sys.stderr)
        for e in errors:
            print("  " + e, file=sys.stderr)
        return 1
    print(
        f"OK — {n_reports} golden reports and {n_profiles} example profiles validate against the "
        f"published schemas with no dry-core."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

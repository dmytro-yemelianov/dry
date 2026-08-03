#!/usr/bin/env python3
"""Independent schema validator for Dry's report outputs, example profiles and TPMS options.

Validates every golden report under `conformance/reports/` against the matching subschema in
`spec/dry-reports-v1.schema.json`, every example profile under `spec/examples/profiles/` against
`spec/dry-profile-v1.schema.json`, and every labelled TPMS option document under
`spec/examples/tpms-options/` against `spec/dry-tpms-options-v1.schema.json`. Uses only `jsonschema`
(no `dry-core`), so it independently confirms that the published schemas actually describe the
engine's real behaviour (see docs/11-profiles-and-reports.md and docs/07-tpms-codegen.md).

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
    "report.json": "RewriteReport",
    "explain.json": "ExplainBundle",
    "expected.json": "CompareDelta",
}


def validate_tpms_options(root: Path, validator_factory, errors: list[str]) -> int:
    """Check the labelled TPMS option corpus against the published option schema.

    The manifest carries two verdicts per case: the schema's and the engine's. Only the schema
    column is checkable here (this validator never imports dry-core); the engine column is checked
    by `crates/core/tests/tpms_options_schema.rs` against the same manifest. What this function adds
    on top of "the schema agrees with its own labels" is the invariant that binds the two columns:
    a case may never be schema-invalid while the engine accepts it. That is the direction which
    would make the published schema wrong — it would refuse a bundle the engine happily runs.
    """
    directory = root / "spec" / "examples" / "tpms-options"
    schema_path = root / "spec" / "dry-tpms-options-v1.schema.json"
    if not directory.is_dir():
        errors.append("[tpms-options] spec/examples/tpms-options is missing")
        return 0
    validator = validator_factory(json.loads(schema_path.read_text()))
    manifest = json.loads((directory / "manifest.json").read_text())
    cases = manifest.get("cases", [])

    listed = set()
    checked = 0
    for case in cases:
        name = case.get("file", "<unnamed>")
        listed.add(name)
        if case.get("schema") not in {"valid", "invalid"}:
            errors.append(f"[tpms-options/{name}] schema verdict must be valid or invalid")
            continue
        if case.get("engine") not in {"accepted", "refused"}:
            errors.append(f"[tpms-options/{name}] engine verdict must be accepted or refused")
            continue
        if case["schema"] == "invalid" and case["engine"] == "accepted":
            errors.append(
                f"[tpms-options/{name}] the schema refuses a bundle the engine accepts; the "
                "published schema must stay a necessary condition"
            )
        if case["engine"] == "refused" and not case.get("refusal"):
            errors.append(f"[tpms-options/{name}] a refused case must quote its refusal text")
        path = directory / name
        if not path.is_file():
            errors.append(f"[tpms-options/{name}] listed in the manifest but missing on disk")
            continue
        found = sorted(validator.iter_errors(json.loads(path.read_text())), key=lambda e: list(e.path))
        if case["schema"] == "valid" and found:
            errors.append(f"[tpms-options/{name}] expected schema-valid: {found[0].message}")
        elif case["schema"] == "invalid" and not found:
            errors.append(f"[tpms-options/{name}] expected schema-invalid, but it validates")
        else:
            checked += 1

    on_disk = {f"cases/{p.name}" for p in (directory / "cases").glob("*.json")}
    for orphan in sorted(on_disk - listed):
        errors.append(f"[tpms-options/{orphan}] present on disk but not listed in the manifest")
    return checked


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

    # The supported firmware/printer matrix: each entry's profile.json (profile schema) and its golden
    # review.json (ReviewReport).
    matrix_dir = root / "conformance" / "profile-matrix"
    review_validator = report_validators["review.json"]
    if matrix_dir.is_dir():
        for entry in sorted(p for p in matrix_dir.iterdir() if p.is_dir()):
            prof = entry / "profile.json"
            if prof.exists():
                doc = json.loads(prof.read_text())
                ok = True
                for e in sorted(profile_validator.iter_errors(doc), key=lambda e: list(e.path)):
                    errors.append(f"[matrix/{entry.name}/profile.json] {e.message}")
                    ok = False
                if ok:
                    n_profiles += 1
            review = entry / "review.json"
            if review.exists():
                doc = json.loads(review.read_text())
                ok = True
                for e in sorted(review_validator.iter_errors(doc), key=lambda e: list(e.path)):
                    errors.append(f"[matrix/{entry.name}/review.json] {e.message}")
                    ok = False
                if ok:
                    n_reports += 1

    n_tpms = validate_tpms_options(root, Draft202012Validator, errors)

    if errors:
        print(f"FAIL — {len(errors)} schema problem(s):", file=sys.stderr)
        for e in errors:
            print("  " + e, file=sys.stderr)
        return 1
    print(
        f"OK — {n_reports} golden reports, {n_profiles} profiles and {n_tpms} labelled TPMS "
        f"option bundles validate against the published schemas with no dry-core."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

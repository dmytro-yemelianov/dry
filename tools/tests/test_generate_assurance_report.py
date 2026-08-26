from __future__ import annotations

import importlib.util
from pathlib import Path
import subprocess
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
GENERATOR = ROOT / "tools" / "generate_assurance_report.py"
REPORT = ROOT / "docs" / "assurance" / "01-assurance-sitemap.md"

SPEC = importlib.util.spec_from_file_location("generate_assurance_report", GENERATOR)
assert SPEC is not None and SPEC.loader is not None
report_module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(report_module)


class AssuranceReportTests(unittest.TestCase):
    def test_committed_report_is_current(self) -> None:
        result = subprocess.run(
            [sys.executable, str(GENERATOR), "--check"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_report_is_portable_and_does_not_mislabel_build_jobs(self) -> None:
        report = report_module.generate_report()
        self.assertNotIn("file:///", report)
        self.assertNotIn("3,138 compiled proof", report)
        self.assertIn("build-system job count, not a theorem count", report)
        self.assertIn("../../proofs/claims.toml", report)
        self.assertIn("Normative clause", report)
        self.assertIn("DRY.FEATURE.EXPANSION_V0", report)

    def test_report_exposes_independent_status_layers_and_obligations(self) -> None:
        claims = report_module.load_claims()
        report = report_module.generate_report(claims)
        numeric_pending = sum(
            claim["status"]["numeric"] == "pending" for claim in claims
        )
        refinement_pending = sum(
            claim["status"]["refinement"] == "pending" for claim in claims
        )
        self.assertIn(f"### Numeric refinement pending ({numeric_pending})", report)
        self.assertIn(f"### Rust refinement pending ({refinement_pending})", report)
        self.assertIn(
            "an abstract Lean theorem does **not** establish binary64 behavior",
            report,
        )

    def test_a_claim_with_no_lean_model_says_so_instead_of_linking_nothing(self) -> None:
        """An unmodelled claim must not render as a link to the repository root.

        `repository_link("")` produces `[``](../../.)`, which reads like a theorem whose file is
        merely mislinked. The registry now allows claims with no Lean model at all, so the report
        has to name that state.
        """
        unmodelled = {
            "id": "FM1.TEST.UNMODELLED",
            "title": "Test claim with no Lean model",
            "spec_version": "test-v0",
            "source_dialect": "L0",
            "target_dialect": "L1",
            "relation": "rejection",
            "scope": "abstract",
            "numeric_domain": "Real",
            "assumptions": [],
            "exclusions": [],
            "rust_sources": ["crates/kernel/src/features.rs"],
            "numeric_evidence": [],
            "refinement_evidence": [],
            "status": {
                "abstract": "specified",
                "numeric": "pending",
                "refinement": "pending",
            },
        }
        report = report_module.generate_report([unmodelled])
        self.assertIn("— no Lean model", report)
        self.assertNotIn("](../../.)", report)


if __name__ == "__main__":
    unittest.main()

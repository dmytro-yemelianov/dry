from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile
import textwrap
import unittest


ROOT = Path(__file__).resolve().parents[2]
VALIDATOR = ROOT / "tools" / "validate_proof_claims.py"


def claim_table(
    claim_id: str,
    *,
    theorem: str = "Dry.Geometry.PlanarTransform.apply_compose",
    scope: str = "abstract",
    numeric: str = "pending",
    refinement: str = "pending",
) -> str:
    return textwrap.dedent(
        f"""
        [[claim]]
        id = "{claim_id}"
        title = "Test claim"
        theorem = "{theorem}"
        spec_version = "test-v0"
        source_dialect = "L0"
        target_dialect = "L1"
        relation = "exact"
        scope = "{scope}"
        numeric_domain = "Real"
        assumptions = []
        exclusions = []
        lean_source = "formal/Dry/Geometry/PlanarTransform.lean"
        rust_sources = ["crates/core/src/features.rs"]
        numeric_evidence = []
        refinement_evidence = []

        [claim.status]
        abstract = "proved"
        numeric = "{numeric}"
        refinement = "{refinement}"
        """
    )


class ProofClaimValidatorTests(unittest.TestCase):
    def run_registry(self, contents: str) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as directory:
            registry = Path(directory) / "claims.toml"
            registry.write_text(contents, encoding="utf-8")
            return subprocess.run(
                [sys.executable, str(VALIDATOR), str(registry)],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )

    def test_repository_registry_passes(self) -> None:
        result = subprocess.run(
            [sys.executable, str(VALIDATOR)],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("proof claims: ok (25 claims", result.stdout)

    def test_duplicate_claim_ids_are_rejected(self) -> None:
        registry = (
            "schema_version = 1\n"
            + claim_table("FM1.TEST.DUPLICATE")
            + claim_table("FM1.TEST.DUPLICATE")
        )
        result = self.run_registry(registry)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("duplicate claim id", result.stderr)

    def test_missing_theorem_declaration_is_rejected(self) -> None:
        registry = "schema_version = 1\n" + claim_table(
            "FM1.TEST.MISSING",
            theorem="Dry.Geometry.PlanarTransform.does_not_exist",
        )
        result = self.run_registry(registry)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("is not declared", result.stderr)

    def test_unrefined_implementation_claim_is_rejected(self) -> None:
        registry = "schema_version = 1\n" + claim_table(
            "FM1.TEST.UNREFINED",
            scope="implementation",
            numeric="pending",
            refinement="pending",
        )
        result = self.run_registry(registry)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "implementation claim requires bounded or inapplicable numeric status",
            result.stderr,
        )
        self.assertIn(
            "implementation claim requires checked refinement",
            result.stderr,
        )


if __name__ == "__main__":
    unittest.main()

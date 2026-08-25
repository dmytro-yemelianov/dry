from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile
import textwrap
import unittest

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib


ROOT = Path(__file__).resolve().parents[2]
VALIDATOR = ROOT / "tools" / "validate_proof_claims.py"


def claim_table(
    claim_id: str,
    *,
    theorem: str | None = "Dry.Geometry.PlanarTransform.apply_compose",
    lean_source: str | None = "formal/Dry/Geometry/PlanarTransform.lean",
    scope: str = "abstract",
    abstract: str = "proved",
    numeric: str = "pending",
    refinement: str = "pending",
) -> str:
    lean_lines = ""
    if theorem is not None:
        lean_lines += f'theorem = "{theorem}"\n        '
    if lean_source is not None:
        lean_lines += f'lean_source = "{lean_source}"\n        '
    return textwrap.dedent(
        f"""
        [[claim]]
        id = "{claim_id}"
        title = "Test claim"
        {lean_lines}spec_version = "test-v0"
        source_dialect = "L0"
        target_dialect = "L1"
        relation = "exact"
        scope = "{scope}"
        numeric_domain = "Real"
        assumptions = []
        exclusions = []
        rust_sources = ["crates/kernel/src/features.rs"]
        numeric_evidence = []
        refinement_evidence = []

        [claim.status]
        abstract = "{abstract}"
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
        with (ROOT / "proofs" / "claims.toml").open("rb") as handle:
            expected_claims = len(tomllib.load(handle)["claim"])
        result = subprocess.run(
            [sys.executable, str(VALIDATOR)],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(
            f"proof claims: ok ({expected_claims} claims", result.stdout
        )

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

    def test_unmodelled_claim_without_a_theorem_is_accepted(self) -> None:
        """A claim may record that no Lean model exists (ADR 0001 `specified`).

        Before #198 the registry could not express this: `theorem` and `lean_source` were required,
        so registering an unmodelled boundary meant pointing at a theorem that was not about it.
        """
        registry = "schema_version = 1\n" + claim_table(
            "FM1.TEST.UNMODELLED",
            theorem=None,
            lean_source=None,
            abstract="specified",
        )
        result = self.run_registry(registry)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_unproved_claim_may_not_register_a_theorem(self) -> None:
        registry = "schema_version = 1\n" + claim_table(
            "FM1.TEST.UNPROVED_WITH_THEOREM", abstract="specified"
        )
        result = self.run_registry(registry)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must not register a theorem", result.stderr)
        self.assertIn("must not register a lean_source", result.stderr)

    def test_proved_claim_without_a_lean_source_is_rejected(self) -> None:
        registry = "schema_version = 1\n" + claim_table(
            "FM1.TEST.PROVED_WITHOUT_SOURCE", lean_source=None
        )
        result = self.run_registry(registry)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("requires a lean_source", result.stderr)


if __name__ == "__main__":
    unittest.main()

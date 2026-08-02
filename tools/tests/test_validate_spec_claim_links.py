from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier
    import tomli as tomllib


ROOT = Path(__file__).resolve().parents[2]
VALIDATOR = ROOT / "tools" / "validate_spec_claim_links.py"


class SpecClaimLinkValidatorTests(unittest.TestCase):
    def test_repository_links_pass(self) -> None:
        result = subprocess.run(
            [sys.executable, str(VALIDATOR)],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        # Derived from the registries rather than frozen at a literal count, matching
        # `test_validate_proof_claims.test_repository_registry_passes`: the assertion is that the
        # validator reports what the files actually contain, not that the corpus never grows.
        with (ROOT / "proofs" / "spec-claim-links.toml").open("rb") as handle:
            clauses = len(tomllib.load(handle)["clause"])
        with (ROOT / "proofs" / "claims.toml").open("rb") as handle:
            claims = len(tomllib.load(handle)["claim"])
        self.assertIn(f"{claims} claims, {clauses} normative clauses", result.stdout)

    def test_missing_claim_link_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            copied = Path(directory) / "links.toml"
            source = (ROOT / "proofs" / "spec-claim-links.toml").read_text(
                encoding="utf-8"
            )
            copied.write_text(
                source.replace(
                    '\n[[link]]\nclaim_id = "FM1.TRANSFORM.COMPOSE_ACTION"\nclause_id = "DRY.FEATURE.EXPANSION_V0"\n',
                    "",
                    1,
                ),
                encoding="utf-8",
            )
            result = subprocess.run(
                [sys.executable, str(VALIDATOR), "--registry", str(copied)],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("claims without normative links", result.stderr)


if __name__ == "__main__":
    unittest.main()

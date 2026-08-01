from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


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
        self.assertRegex(result.stdout, r"\d+ claims, 12 normative clauses")

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

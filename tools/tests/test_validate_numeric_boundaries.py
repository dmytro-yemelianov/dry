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
INVENTORY = ROOT / "proofs" / "feature-numeric-boundaries-v0.toml"
PROFILE = ROOT / "proofs" / "feature-planar-numeric-profile-v0.toml"
VALIDATOR = ROOT / "tools" / "validate_numeric_boundaries.py"


class NumericBoundaryValidatorTests(unittest.TestCase):
    def run_inventory(self, contents: str) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as directory:
            inventory = Path(directory) / "numeric-boundaries.toml"
            inventory.write_text(contents, encoding="utf-8")
            return subprocess.run(
                [sys.executable, str(VALIDATOR), str(inventory)],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )

    def repository_inventory(self) -> str:
        return INVENTORY.read_text(encoding="utf-8")

    def run_profile(self, contents: str) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as directory:
            profile = Path(directory) / "numeric-profile.toml"
            profile.write_text(contents, encoding="utf-8")
            return subprocess.run(
                [sys.executable, str(VALIDATOR), "--profile", str(profile)],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )

    def repository_profile(self) -> str:
        return PROFILE.read_text(encoding="utf-8")

    def test_repository_inventory_passes(self) -> None:
        result = subprocess.run(
            [sys.executable, str(VALIDATOR)],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("numeric boundaries: ok (13 boundaries", result.stdout)
        self.assertIn("profile 9 limits/14 budgets", result.stdout)

    def test_source_hash_drift_is_rejected(self) -> None:
        contents = self.repository_inventory().replace(
            "e97355ff5d0e03095ae5d9b2e2305bdfe0e466e4fe312650018fdcbd8e514946",
            "0" * 64,
            1,
        )
        result = self.run_inventory(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "changed without numeric-boundary review",
            result.stderr,
        )

    def test_missing_boundary_is_rejected(self) -> None:
        document = tomllib.loads(self.repository_inventory())
        last_id = document["boundary"][-1]["id"]
        marker = f'\n[[boundary]]\nid = "{last_id}"'
        start = self.repository_inventory().index(marker)
        result = self.run_inventory(self.repository_inventory()[:start])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing required boundaries", result.stderr)

    def test_source_anchor_drift_is_rejected(self) -> None:
        contents = self.repository_inventory().replace(
            'source_anchor = "degrees * PI / 180.0"',
            'source_anchor = "missing angle conversion"',
            1,
        )
        result = self.run_inventory(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("source_anchor must occur exactly once", result.stderr)

    def test_unlinked_claim_is_rejected(self) -> None:
        contents = self.repository_inventory().replace(
            'claim_ids = [\n  "FM1.TRANSFORM.COMPOSE_ACTION",\n'
            '  "FM1.FEATURE.COMPOSE_ACTION",\n'
            '  "FM1.NUMERIC.TRIG.COEFFICIENTS",\n'
            '  "FM1.NUMERIC.NATIVE.CARDINAL_INTERVALS"\n]',
            'claim_ids = ["FM1.DOES.NOT.EXIST"]',
            1,
        )
        result = self.run_inventory(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unknown claim id", result.stderr)

    def test_profile_libm_drift_is_rejected(self) -> None:
        contents = self.repository_profile().replace(
            'version = "0.2.16"',
            'version = "0.2.15"',
            1,
        )
        result = self.run_profile(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not match Cargo.lock", result.stderr)

    def test_profile_libm_accuracy_contract_drift_is_rejected(self) -> None:
        contents = self.repository_profile().replace(
            'trig_max_ulp = 1',
            'trig_max_ulp = 2',
            1,
        )
        result = self.run_profile(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "does not match the imported trigonometric contract",
            result.stderr,
        )

    def test_pending_budget_cannot_publish_an_unchecked_ceiling(self) -> None:
        contents = self.repository_profile().replace(
            'status = "bounded"\nceiling = 2.842170943040401e-14\n'
            'rationale = "Under the named imported',
            'status = "pending"\nceiling = 2.842170943040401e-14\n'
            'rationale = "Under the named imported',
            1,
        )
        result = self.run_profile(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "pending budget must not publish a ceiling",
            result.stderr,
        )

    def test_implementation_policy_value_drift_is_rejected(self) -> None:
        contents = self.repository_profile().replace(
            "ceiling = 1e-12",
            "ceiling = 2e-12",
            1,
        )
        result = self.run_profile(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "does not match Transform::is_identity EPS",
            result.stderr,
        )

    def test_binary64_proof_ceiling_drift_is_rejected(self) -> None:
        contents = self.repository_profile().replace(
            "ceiling = 1.862645149230957e-9",
            "ceiling = 2e-9",
            1,
        )
        result = self.run_profile(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "binary64 proof ceiling must remain",
            result.stderr,
        )

    def test_angle_proof_ceiling_drift_is_rejected(self) -> None:
        contents = self.repository_profile().replace(
            "ceiling = 1.4210854715202004e-14",
            "ceiling = 2e-14",
            1,
        )
        result = self.run_profile(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "binary64 proof ceiling must remain",
            result.stderr,
        )

    def test_trig_proof_ceiling_drift_is_rejected(self) -> None:
        contents = self.repository_profile().replace(
            "ceiling = 2.842170943040401e-14",
            "ceiling = 3e-14",
            1,
        )
        result = self.run_profile(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "binary64 proof ceiling must remain",
            result.stderr,
        )

    def test_repeat_rotation_proof_ceiling_drift_is_rejected(self) -> None:
        contents = self.repository_profile().replace(
            "ceiling = 0.0009765625",
            "ceiling = 0.001",
            1,
        )
        result = self.run_profile(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "binary64 proof ceiling must remain",
            result.stderr,
        )

    def test_repeat_translation_proof_ceiling_drift_is_rejected(self) -> None:
        contents = self.repository_profile().replace(
            "ceiling = 536870912.0",
            "ceiling = 536870911.0",
            1,
        )
        result = self.run_profile(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "binary64 proof ceiling must remain",
            result.stderr,
        )

    def test_composition_tree_rotation_proof_ceiling_drift_is_rejected(
        self,
    ) -> None:
        contents = self.repository_profile().replace(
            'metric = "Maximum absolute error of either rotation coefficient '
            'for an arbitrary parenthesized transform-composition tree '
            'against its exact-real tree"\n'
            'unit = "dimensionless"\n'
            'status = "bounded"\n'
            "ceiling = 0.0009765625",
            'metric = "Maximum absolute error of either rotation coefficient '
            'for an arbitrary parenthesized transform-composition tree '
            'against its exact-real tree"\n'
            'unit = "dimensionless"\n'
            'status = "bounded"\n'
            "ceiling = 0.001",
            1,
        )
        result = self.run_profile(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "binary64 proof ceiling must remain",
            result.stderr,
        )

    def test_composition_tree_translation_proof_ceiling_drift_is_rejected(
        self,
    ) -> None:
        contents = self.repository_profile().replace(
            "ceiling = 1073741824.0",
            "ceiling = 1073741823.0",
            1,
        )
        result = self.run_profile(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "binary64 proof ceiling must remain",
            result.stderr,
        )

    def test_binary64_operation_limit_drift_is_rejected(self) -> None:
        contents = self.repository_profile().replace(
            "upper = 4194304.0",
            "upper = 8388608.0",
            1,
        )
        result = self.run_profile(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "binary64 proof limits must remain",
            result.stderr,
        )

    def test_profile_links_must_be_reciprocal(self) -> None:
        contents = self.repository_inventory().replace(
            '"FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.'
            'BUDGET.ANGLE_RAD_ABS_ERROR"',
            '"FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.'
            'BUDGET.TRIG_COEFFICIENT_ABS_ERROR"',
            1,
        )
        result = self.run_inventory(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing reciprocal", result.stderr)


if __name__ == "__main__":
    unittest.main()

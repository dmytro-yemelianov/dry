from __future__ import annotations

import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier
    import tomli as tomllib


ROOT = Path(__file__).resolve().parents[2]
INVENTORY = ROOT / "proofs" / "feature-numeric-boundaries-v0.toml"
PROFILE = ROOT / "proofs" / "feature-planar-numeric-profile-v0.toml"
VERIFY_INVENTORY = ROOT / "proofs" / "verify-numeric-boundaries-v0.toml"
EMIT_INVENTORY = ROOT / "proofs" / "emit-numeric-boundaries-v0.toml"
CONTRACTS = ROOT / "crates" / "contracts" / "src" / "lib.rs"
VALIDATOR = ROOT / "tools" / "validate_numeric_boundaries.py"

# The tests above drive the validator as a subprocess, which is right for whole-inventory
# behaviour. The source-side helpers below (per-owner constant resolution, the duplicate sweep)
# read Rust files under `ROOT`, so testing them that way would mean mutating the real tree.
# Importing the module lets `ROOT` be pointed at a fixture instead.
_SPEC = importlib.util.spec_from_file_location("validate_numeric_boundaries", VALIDATOR)
assert _SPEC is not None and _SPEC.loader is not None
validator = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(validator)


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
        self.assertIn("profile 9 limits/17 budgets", result.stdout)

    def test_source_hash_drift_is_rejected(self) -> None:
        # Read the pinned digest rather than spelling it out: a literal here goes stale every time
        # features.rs is touched, and a stale literal turns the corruption into a no-op.
        document = tomllib.loads(self.repository_inventory())
        contents = self.repository_inventory().replace(
            document["source"][0]["sha256"],
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
            '"FM1.NUMERIC.ANGLE.RADIANS"',
            '"FM1.DOES.NOT.EXIST"',
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

    def test_composition_tree_orientation_angular_ceiling_drift_is_rejected(
        self,
    ) -> None:
        contents = self.repository_profile().replace(
            'id = "FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.BUDGET.'
            'COMPOSITION_TREE_ORIENTATION_ANGULAR_ERROR_RAD"\n'
            'metric = "Maximum unoriented angular error for a unit orientation '
            'transformed by an arbitrary parenthesized transform-composition tree"\n'
            'unit = "radian"\n'
            'status = "bounded"\n'
            "ceiling = 0.25",
            'id = "FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.BUDGET.'
            'COMPOSITION_TREE_ORIENTATION_ANGULAR_ERROR_RAD"\n'
            'metric = "Maximum unoriented angular error for a unit orientation '
            'transformed by an arbitrary parenthesized transform-composition tree"\n'
            'unit = "radian"\n'
            'status = "bounded"\n'
            "ceiling = 0.26",
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


class VerifyToleranceOwnershipTests(unittest.TestCase):
    """The per-owner pin mechanism, added when `ARC_RADIUS_TOLERANCE_MM` moved to kmet-contracts.

    Ownership used to be a per-inventory fact — verify.rs held all four verify epsilons — so nothing
    tested it. It is now per-constant, and Task 5 moves three more of them, so it needs a net.
    """

    def build_tree(self, directory: str, verify_body: str, contracts_body: str) -> Path:
        root = Path(directory)
        (root / "crates" / "core" / "src").mkdir(parents=True)
        (root / "crates" / "contracts" / "src").mkdir(parents=True)
        (root / "crates" / "core" / "src" / "verify.rs").write_text(
            verify_body, encoding="utf-8"
        )
        (root / "crates" / "contracts" / "src" / "lib.rs").write_text(
            contracts_body, encoding="utf-8"
        )
        return root

    def test_owner_map_covers_every_pinned_constant(self) -> None:
        for constant in validator.VERIFY_IMPLEMENTATION_TOLERANCES.values():
            self.assertIn(constant, validator.VERIFY_TOLERANCE_OWNERS)
            owner = validator.VERIFY_TOLERANCE_OWNERS[constant]
            self.assertTrue(
                (ROOT / owner).is_file(), f"{constant}: owner {owner} does not exist"
            )

    def test_owner_roots_are_swept_for_duplicates(self) -> None:
        # Every owner must live under a root the duplicate sweep actually walks, or a second
        # definition of that constant would be invisible.
        for owner in set(validator.VERIFY_TOLERANCE_OWNERS.values()):
            self.assertTrue(
                any(owner.startswith(root) for root in validator.SINGLE_DEFINITION_ROOTS),
                f"{owner} is outside SINGLE_DEFINITION_ROOTS",
            )

    def test_unowned_constant_is_a_diagnostic_not_a_traceback(self) -> None:
        errors: list[str] = []
        tolerances = dict(validator.VERIFY_IMPLEMENTATION_TOLERANCES)
        tolerances["FM1.MADE.UP.BUDGET"] = "NO_SUCH_TOLERANCE_MM"
        with mock.patch.object(
            validator, "VERIFY_IMPLEMENTATION_TOLERANCES", tolerances
        ):
            validator.validate_verify_implementation_values({"budget": []}, errors)
        self.assertTrue(
            any("NO_SUCH_TOLERANCE_MM has no entry" in e for e in errors), errors
        )

    def test_each_epsilon_is_read_from_its_own_owner(self) -> None:
        # The contracts-owned constant is 2e-6 here and the verify-owned one 1e-6. Reading either
        # from the wrong file yields the other value, so a regression cannot pass this silently.
        with tempfile.TemporaryDirectory() as directory:
            root = self.build_tree(
                directory,
                "const CONTINUITY_TOLERANCE_MM: f64 = 1e-6;\n",
                "pub const ARC_RADIUS_TOLERANCE_MM: f64 = 2e-6;\n",
            )
            profile = {
                "budget": [
                    {"id": "B.CONTINUITY", "ceiling": 1e-6},
                    {"id": "B.ARC", "ceiling": 2e-6},
                ]
            }
            errors: list[str] = []
            with mock.patch.object(validator, "ROOT", root), mock.patch.object(
                validator,
                "VERIFY_IMPLEMENTATION_TOLERANCES",
                {
                    "B.CONTINUITY": "CONTINUITY_TOLERANCE_MM",
                    "B.ARC": "ARC_RADIUS_TOLERANCE_MM",
                },
            ), mock.patch.object(
                validator,
                "VERIFY_TOLERANCE_OWNERS",
                {
                    "CONTINUITY_TOLERANCE_MM": "crates/core/src/verify.rs",
                    "ARC_RADIUS_TOLERANCE_MM": "crates/contracts/src/lib.rs",
                },
            ):
                validator.validate_verify_implementation_values(profile, errors)
            self.assertEqual(errors, [])

    def test_ceiling_drift_in_the_contracts_owned_epsilon_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.build_tree(
                directory,
                "",
                "pub const ARC_RADIUS_TOLERANCE_MM: f64 = 2e-6;\n",
            )
            errors: list[str] = []
            with mock.patch.object(validator, "ROOT", root), mock.patch.object(
                validator, "VERIFY_IMPLEMENTATION_TOLERANCES", {"B.ARC": "ARC_RADIUS_TOLERANCE_MM"}
            ), mock.patch.object(
                validator,
                "VERIFY_TOLERANCE_OWNERS",
                {"ARC_RADIUS_TOLERANCE_MM": "crates/contracts/src/lib.rs"},
            ):
                validator.validate_verify_implementation_values(
                    {"budget": [{"id": "B.ARC", "ceiling": 1e-6}]}, errors
                )
            self.assertTrue(
                any("does not match ARC_RADIUS_TOLERANCE_MM" in e for e in errors), errors
            )

    def test_duplicate_definition_is_caught_across_both_crate_roots(self) -> None:
        # The sweep used to walk crates/core alone. A constant owned by kmet-contracts and restated
        # in the kernel is the exact regression that would have reintroduced.
        with tempfile.TemporaryDirectory() as directory:
            root = self.build_tree(
                directory,
                "const ARC_RADIUS_TOLERANCE_MM: f64 = 1e-6;\n",
                "pub const ARC_RADIUS_TOLERANCE_MM: f64 = 1e-6;\n",
            )
            errors: list[str] = []
            with mock.patch.object(validator, "ROOT", root):
                validator.require_single_definition(
                    {"ARC_RADIUS_TOLERANCE_MM": "crates/contracts/src/lib.rs"}, errors
                )
            self.assertTrue(
                any("must have one definition" in e for e in errors), errors
            )

    def test_sole_definition_in_the_contracts_owner_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.build_tree(
                directory, "", "pub const ARC_RADIUS_TOLERANCE_MM: f64 = 1e-6;\n"
            )
            errors: list[str] = []
            with mock.patch.object(validator, "ROOT", root):
                validator.require_single_definition(
                    {"ARC_RADIUS_TOLERANCE_MM": "crates/contracts/src/lib.rs"}, errors
                )
            self.assertEqual(errors, [])

    def test_restating_the_owner_definition_twice_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.build_tree(
                directory,
                "",
                "pub const ARC_RADIUS_TOLERANCE_MM: f64 = 1e-6;\n"
                "pub const ARC_RADIUS_TOLERANCE_MM: f64 = 1e-6;\n",
            )
            errors: list[str] = []
            with mock.patch.object(validator, "ROOT", root):
                validator.require_single_definition(
                    {"ARC_RADIUS_TOLERANCE_MM": "crates/contracts/src/lib.rs"}, errors
                )
            self.assertTrue(any("is defined 2 times" in e for e in errors), errors)


class ContractsSliceCoverageTests(unittest.TestCase):
    """`kmet-contracts` is pinned by two slices, one per inventory, split by provenance.

    The first attempt pinned 460 of the crate's 31 812 bytes and left `RuleId::default_severity`,
    `parse_bounds_csv` and `REFERENCE_FIVE_AXIS_MACHINE` outside numeric review. These tests are the
    net for that: the slices must resolve, and nothing executable may fall between them.
    """

    def slice_span(self, inventory: Path) -> tuple[int, int]:
        document = tomllib.loads(inventory.read_text(encoding="utf-8"))
        text = CONTRACTS.read_text(encoding="utf-8")
        for source in document["source"]:
            if source["path"] != "crates/contracts/src/lib.rs":
                continue
            self.assertEqual(source["hash_mode"], "slice")
            start = text.index(source["anchor_start"])
            end = (
                len(text)
                if "anchor_end" not in source
                else text.index(source["anchor_end"])
            )
            self.assertLess(start, end)
            return start, end
        self.fail(f"{inventory.name} does not pin crates/contracts/src/lib.rs")

    def test_both_inventories_pin_a_contracts_slice(self) -> None:
        verify_span = self.slice_span(VERIFY_INVENTORY)
        emit_span = self.slice_span(EMIT_INVENTORY)
        # Contiguous and non-overlapping: the verify half ends exactly where the emit half begins.
        self.assertEqual(verify_span[1], emit_span[0])
        self.assertEqual(emit_span[1], len(CONTRACTS.read_text(encoding="utf-8")))

    def test_no_executable_line_falls_outside_the_pinned_slices(self) -> None:
        text = CONTRACTS.read_text(encoding="utf-8")
        covered_from = min(
            self.slice_span(VERIFY_INVENTORY)[0], self.slice_span(EMIT_INVENTORY)[0]
        )
        uncovered = text[:covered_from]
        for number, line in enumerate(uncovered.splitlines(), start=1):
            stripped = line.strip()
            self.assertTrue(
                stripped == ""
                or stripped.startswith("//!")
                or stripped.startswith("#!")
                or stripped.startswith("use "),
                f"crates/contracts/src/lib.rs:{number} is outside every pinned slice "
                f"but is not header material: {line!r}",
            )

    def test_policy_surface_is_inside_a_pinned_slice(self) -> None:
        # Named explicitly so the three drifts the review demonstrated cannot silently escape again.
        text = CONTRACTS.read_text(encoding="utf-8")
        verify_span = self.slice_span(VERIFY_INVENTORY)
        emit_span = self.slice_span(EMIT_INVENTORY)
        for anchor in (
            "pub const REFERENCE_FIVE_AXIS_MACHINE",
            "pub fn parse_bounds_csv",
            "pub fn parse_speed_range_csv",
            "pub fn default_severity",
            "pub fn is_evaluated",
            "pub fn from_wire",
            "pub const ALL: [RuleId; 27]",
            "pub const ARC_RADIUS_TOLERANCE_MM",
            "impl Serialize for Kinematics",
        ):
            offset = text.index(anchor)
            self.assertTrue(
                verify_span[0] <= offset < verify_span[1]
                or emit_span[0] <= offset < emit_span[1],
                f"{anchor} is outside every pinned slice",
            )


class SliceHashModeTests(unittest.TestCase):
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

    def test_slice_to_end_of_file_passes_on_the_repository_inventory(self) -> None:
        # The emit inventory's contracts slice omits `anchor_end`, which means "to EOF".
        document = tomllib.loads(EMIT_INVENTORY.read_text(encoding="utf-8"))
        contracts = [
            source
            for source in document["source"]
            if source["path"] == "crates/contracts/src/lib.rs"
        ]
        self.assertEqual(len(contracts), 1)
        self.assertNotIn("anchor_end", contracts[0])
        result = subprocess.run(
            [sys.executable, str(VALIDATOR), str(EMIT_INVENTORY)],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("2 pinned sources", result.stdout)

    def test_slice_hash_drift_is_rejected(self) -> None:
        document = tomllib.loads(VERIFY_INVENTORY.read_text(encoding="utf-8"))
        digest = next(
            source["sha256"]
            for source in document["source"]
            if source["path"] == "crates/contracts/src/lib.rs"
        )
        contents = VERIFY_INVENTORY.read_text(encoding="utf-8").replace(
            digest, "0" * 64, 1
        )
        result = self.run_inventory(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("changed without numeric-boundary review", result.stderr)

    def test_slice_anchor_drift_is_rejected(self) -> None:
        contents = VERIFY_INVENTORY.read_text(encoding="utf-8").replace(
            'anchor_start = "/// The limits a toolpath is checked against."',
            'anchor_start = "/// no such anchor"',
            1,
        )
        result = self.run_inventory(contents)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("slice anchors must occur exactly once", result.stderr)


if __name__ == "__main__":
    unittest.main()

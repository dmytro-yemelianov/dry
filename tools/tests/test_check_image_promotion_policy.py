"""The promotion policy gate must reject every way the digest binding can lapse."""

import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "tools" / "check_image_promotion_policy.py"
BUILDER = ".github/workflows/verify-runner.yml"
DEPLOY = ".github/workflows/deploy-verify.yml"


class ImagePromotionPolicyTests(unittest.TestCase):
    def run_checker(self, root: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(CHECKER), "--root", str(root)],
            text=True,
            capture_output=True,
            check=False,
        )

    def mutate(self, workflow: str, before: str, after: str) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / ".github" / "workflows").mkdir(parents=True)
            for relative in (BUILDER, DEPLOY):
                shutil.copyfile(ROOT / relative, root / relative)
            target = root / workflow
            text = target.read_text(encoding="utf-8")
            self.assertIn(before, text, f"fixture no longer matches {workflow}")
            target.write_text(text.replace(before, after, 1), encoding="utf-8")
            return self.run_checker(root)

    def test_the_committed_workflows_satisfy_the_policy(self) -> None:
        result = self.run_checker(ROOT)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("attested digests", result.stdout)

    def test_the_dry_run_job_is_not_held_to_the_promotion_config_rule(self) -> None:
        # The control-plane job deliberately validates the committed config with a
        # bare `wrangler deploy --dry-run`; the rule must stay scoped to `deploy`.
        text = (ROOT / DEPLOY).read_text(encoding="utf-8")
        self.assertIn("npx wrangler deploy --dry-run", text)
        self.assertEqual(self.run_checker(ROOT).returncode, 0)

    def test_an_unpinned_action_fails(self) -> None:
        result = self.mutate(
            BUILDER,
            "docker/setup-qemu-action@c7c53464625b32c7a7e944ae62b3e17d2b600130 # v3",
            "docker/setup-qemu-action@v3",
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("unpinned action", result.stderr)

    def test_dropping_the_semver_tag_fails(self) -> None:
        result = self.mutate(BUILDER, "type=semver,pattern={{version}}", "type=raw,value=release")
        self.assertEqual(result.returncode, 1)
        self.assertIn("type=semver", result.stderr)

    def test_dropping_the_release_tag_trigger_fails(self) -> None:
        result = self.mutate(BUILDER, '- "v*"', '- "never-matched-*"')
        self.assertEqual(result.returncode, 1)

    def test_dropping_the_attestation_fails(self) -> None:
        result = self.mutate(
            BUILDER, "push-to-registry: true", "push-to-registry: false"
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("push-to-registry", result.stderr)

    def test_dropping_the_attestation_permission_fails(self) -> None:
        result = self.mutate(BUILDER, "\n      attestations: write", "\n      attestations: none")
        self.assertEqual(result.returncode, 1)

    def test_removing_the_promotion_gate_fails(self) -> None:
        result = self.mutate(
            DEPLOY,
            "python3 tools/promote_runner_image.py resolve",
            "echo skipping the promotion gate",
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("promote_runner_image.py resolve", result.stderr)

    def test_removing_the_evidence_close_fails(self) -> None:
        result = self.mutate(
            DEPLOY,
            "python3 tools/promote_runner_image.py finalize",
            "echo skipping the evidence",
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("promote_runner_image.py finalize", result.stderr)

    def test_deploying_without_the_promotion_config_fails(self) -> None:
        result = self.mutate(
            DEPLOY,
            'npx wrangler deploy -c "$PROMOTION_CONFIG" --env staging',
            "npx wrangler deploy --env staging",
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("without the digest-pinned", result.stderr)

    def test_deploying_before_the_gate_fails(self) -> None:
        text = (ROOT / DEPLOY).read_text(encoding="utf-8")
        marker = "      - name: resolve and verify the runner image digest"
        self.assertIn(marker, text)
        result = self.mutate(
            DEPLOY,
            marker,
            "      - name: sneak a deploy in first\n"
            '        run: npx wrangler deploy\n' + marker,
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("before the promotion gate", result.stderr)

    def test_dropping_the_registry_read_permission_fails(self) -> None:
        result = self.mutate(DEPLOY, "\n      packages: read", "\n      packages: none")
        self.assertEqual(result.returncode, 1)
        self.assertIn("packages: read", result.stderr)

    def test_a_comment_cannot_satisfy_a_policy_marker(self) -> None:
        result = self.mutate(
            DEPLOY,
            "\n      packages: read",
            "\n      # packages: read",
        )
        self.assertEqual(result.returncode, 1)

    def test_passing_a_registry_credential_as_an_argument_fails(self) -> None:
        result = self.mutate(
            DEPLOY,
            '            --github-output "$GITHUB_OUTPUT"',
            '            --github-token "$GH_TOKEN" \\\n'
            '            --github-output "$GITHUB_OUTPUT"',
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("credential as an argument", result.stderr)

    def test_a_missing_workflow_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            result = self.run_checker(Path(directory))
            self.assertEqual(result.returncode, 1)
            self.assertIn("missing required workflow", result.stderr)


if __name__ == "__main__":
    unittest.main()

"""The promotion gate must bind a deployment to one attested image digest."""

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))

import promote_runner_image as promote  # noqa: E402


DIGEST = "sha256:" + "ab" * 32
OTHER_DIGEST = "sha256:" + "cd" * 32
COMMIT = "0" * 39 + "1"
IMAGE = "ghcr.io/dmytro-yemelianov/dry-verify-runner"
WORKFLOW = ".github/workflows/verify-runner.yml"


def attestation(
    *,
    digest: str = DIGEST,
    commit: str = COMMIT,
    repository: str = "dmytro-yemelianov/dry",
    workflow: str = WORKFLOW,
) -> list[dict]:
    return [
        {
            "verificationResult": {
                "statement": {
                    "subject": [{"digest": {"sha256": digest.split(":", 1)[1]}}],
                    "predicate": {
                        "buildDefinition": {
                            "externalParameters": {
                                "workflow": {
                                    "repository": f"https://github.com/{repository}",
                                    "path": workflow,
                                    "ref": "refs/tags/v0.11.0",
                                }
                            },
                            "resolvedDependencies": [
                                {"digest": {"gitCommit": commit}},
                            ],
                        }
                    },
                }
            }
        }
    ]


class DigestTests(unittest.TestCase):
    def test_accepts_a_lowercase_sha256_digest(self) -> None:
        self.assertEqual(promote.require_digest(DIGEST), DIGEST)

    def test_rejects_short_uppercase_and_untagged_forms(self) -> None:
        for value in (
            "sha256:" + "ab" * 31,
            "sha256:" + "AB" * 32,
            "sha512:" + "ab" * 32,
            "ab" * 32,
            "latest",
            "",
        ):
            with self.subTest(value=value):
                with self.assertRaises(promote.PromotionError):
                    promote.require_digest(value)

    def test_immutable_reference_rejects_a_mutable_tag(self) -> None:
        for reference in (
            f"{IMAGE}:latest",
            f"{IMAGE}:v0.11.0",
            IMAGE,
        ):
            with self.subTest(reference=reference):
                with self.assertRaises(promote.PromotionError):
                    promote.require_immutable_reference(reference)

    def test_immutable_reference_splits_repository_and_digest(self) -> None:
        self.assertEqual(
            promote.require_immutable_reference(f"{IMAGE}@{DIGEST}"),
            (IMAGE, DIGEST),
        )


class ReleaseTagTests(unittest.TestCase):
    def test_production_uses_the_release_version(self) -> None:
        self.assertEqual(
            promote.expected_release_tag("production", "refs/tags/v0.11.0", COMMIT),
            "0.11.0",
        )
        self.assertEqual(
            promote.expected_release_tag("production", "v0.11.0", COMMIT),
            "0.11.0",
        )

    def test_production_refuses_a_branch_ref(self) -> None:
        for ref in ("refs/heads/main", "main", "v0.11", "release-0.11.0", ""):
            with self.subTest(ref=ref):
                with self.assertRaises(promote.PromotionError):
                    promote.expected_release_tag("production", ref, COMMIT)

    def test_staging_uses_the_short_commit_tag(self) -> None:
        self.assertEqual(
            promote.expected_release_tag("staging", "refs/heads/main", COMMIT),
            "sha-" + COMMIT[:7],
        )

    def test_staging_refuses_a_partial_commit(self) -> None:
        with self.assertRaises(promote.PromotionError):
            promote.expected_release_tag("staging", "refs/heads/main", COMMIT[:7])

    def test_unknown_environment_fails_closed(self) -> None:
        with self.assertRaises(promote.PromotionError):
            promote.expected_release_tag("dev", "refs/heads/main", COMMIT)

    def test_expected_builder_ref_pins_the_release_tag(self) -> None:
        self.assertEqual(
            promote.expected_workflow_ref("production", "v0.11.0"), "refs/tags/v0.11.0"
        )
        self.assertEqual(
            promote.expected_workflow_ref("production", "refs/tags/v0.11.0"),
            "refs/tags/v0.11.0",
        )

    def test_expected_builder_ref_pins_staging_to_main(self) -> None:
        self.assertEqual(
            promote.expected_workflow_ref("staging", "refs/heads/main"), "refs/heads/main"
        )

    def test_expected_builder_ref_refuses_a_staging_promotion_off_main(self) -> None:
        for ref in ("refs/heads/topic", "refs/tags/v0.11.0"):
            with self.subTest(ref=ref):
                with self.assertRaises(promote.PromotionError):
                    promote.expected_workflow_ref("staging", ref)


class CloudflareReferenceTests(unittest.TestCase):
    def test_reference_is_derived_from_the_source_digest(self) -> None:
        self.assertEqual(
            promote.cloudflare_image_reference("acct123", "dry-verify-runner", DIGEST),
            "registry.cloudflare.com/acct123/dry-verify-runner:sha256-" + "ab" * 32,
        )

    def test_reference_requires_a_digest_not_a_tag(self) -> None:
        with self.assertRaises(promote.PromotionError):
            promote.cloudflare_image_reference("acct123", "dry-verify-runner", "latest")

    def test_reference_requires_an_account_and_name(self) -> None:
        for account, name in (("", "dry-verify-runner"), ("acct123", "")):
            with self.subTest(account=account, name=name):
                with self.assertRaises(promote.PromotionError):
                    promote.cloudflare_image_reference(account, name, DIGEST)


class AttestationTests(unittest.TestCase):
    def verify(self, payload, **overrides):
        arguments = {
            "digest": DIGEST,
            "repository": "dmytro-yemelianov/dry",
            "source_commit": COMMIT,
            "workflow_path": WORKFLOW,
        }
        arguments.update(overrides)
        return promote.verify_attestation_payload(payload, **arguments)

    def test_matching_attestation_passes(self) -> None:
        self.verify(attestation())

    def test_rejects_a_different_subject_digest(self) -> None:
        with self.assertRaises(promote.PromotionError):
            self.verify(attestation(digest=OTHER_DIGEST))

    def test_rejects_a_different_source_commit(self) -> None:
        with self.assertRaises(promote.PromotionError):
            self.verify(attestation(commit="f" * 40))

    def test_rejects_a_different_builder_workflow(self) -> None:
        with self.assertRaises(promote.PromotionError):
            self.verify(attestation(workflow=".github/workflows/attacker.yml"))

    def test_rejects_a_different_source_repository(self) -> None:
        with self.assertRaises(promote.PromotionError):
            self.verify(attestation(repository="attacker/dry"))

    def test_rejects_a_build_from_another_ref(self) -> None:
        with self.assertRaises(promote.PromotionError):
            self.verify(attestation(), workflow_ref="refs/heads/main")

    def test_accepts_the_expected_ref(self) -> None:
        self.verify(attestation(), workflow_ref="refs/tags/v0.11.0")

    def test_rejects_an_empty_payload(self) -> None:
        for payload in ([], {}, None, ""):
            with self.subTest(payload=payload):
                with self.assertRaises(promote.PromotionError):
                    self.verify(payload)


class ReleaseEvidenceTests(unittest.TestCase):
    def test_reads_the_recorded_digest(self) -> None:
        payload = {"image": IMAGE, "digest": DIGEST, "source_commit": COMMIT}
        self.assertEqual(promote.release_evidence_digest(payload, image=IMAGE), DIGEST)

    def test_rejects_a_digest_recorded_for_another_image(self) -> None:
        payload = {"image": "ghcr.io/attacker/runner", "digest": DIGEST}
        with self.assertRaises(promote.PromotionError):
            promote.release_evidence_digest(payload, image=IMAGE)

    def test_rejects_a_missing_digest(self) -> None:
        with self.assertRaises(promote.PromotionError):
            promote.release_evidence_digest({"image": IMAGE}, image=IMAGE)


class PromotionConfigTests(unittest.TestCase):
    def base(self) -> dict:
        return {
            "name": "dry-cloud",
            "containers": [
                {
                    "name": "dry-verify-runner",
                    "class_name": "VerifyContainer",
                    "image": "../../containers/verify-runner/Dockerfile",
                    "image_build_context": "../..",
                    "instance_type": "standard-3",
                    "max_instances": 5,
                }
            ],
            "env": {
                "staging": {
                    "name": "dry-cloud-staging",
                    "containers": [
                        {
                            "name": "dry-verify-runner-staging",
                            "class_name": "VerifyContainer",
                            "image": "../../containers/verify-runner/Dockerfile",
                            "image_build_context": "../..",
                            "instance_type": "standard-3",
                            "max_instances": 2,
                        }
                    ],
                }
            },
        }

    REFERENCE = "registry.cloudflare.com/acct123/dry-verify-runner:sha256-" + "ab" * 32

    def test_production_pins_the_top_level_container(self) -> None:
        result = promote.build_promotion_config(
            self.base(), environment="production", image=self.REFERENCE
        )
        container = result["containers"][0]
        self.assertEqual(container["image"], self.REFERENCE)
        self.assertNotIn("image_build_context", container)
        self.assertEqual(
            result["env"]["staging"]["containers"][0]["image"],
            "../../containers/verify-runner/Dockerfile",
        )

    def test_staging_pins_only_the_staging_container(self) -> None:
        result = promote.build_promotion_config(
            self.base(), environment="staging", image=self.REFERENCE
        )
        self.assertEqual(result["env"]["staging"]["containers"][0]["image"], self.REFERENCE)
        self.assertNotIn("image_build_context", result["env"]["staging"]["containers"][0])
        self.assertEqual(
            result["containers"][0]["image"],
            "../../containers/verify-runner/Dockerfile",
        )

    def test_no_other_field_changes(self) -> None:
        base = self.base()
        result = promote.build_promotion_config(
            base, environment="production", image=self.REFERENCE
        )
        expected = self.base()
        expected["containers"][0]["image"] = self.REFERENCE
        del expected["containers"][0]["image_build_context"]
        self.assertEqual(result, expected)

    def test_the_input_config_is_not_mutated(self) -> None:
        base = self.base()
        untouched = copy.deepcopy(base)
        promote.build_promotion_config(base, environment="production", image=self.REFERENCE)
        self.assertEqual(base, untouched)

    def test_refuses_an_image_outside_the_cloudflare_registry(self) -> None:
        with self.assertRaises(promote.PromotionError):
            promote.build_promotion_config(
                self.base(), environment="production", image=f"{IMAGE}@{DIGEST}"
            )

    def test_refuses_a_mutable_cloudflare_tag(self) -> None:
        with self.assertRaises(promote.PromotionError):
            promote.build_promotion_config(
                self.base(),
                environment="production",
                image="registry.cloudflare.com/acct123/dry-verify-runner:latest",
            )

    def test_refuses_a_config_without_exactly_one_container(self) -> None:
        empty = self.base()
        empty["containers"] = []
        two = self.base()
        two["containers"] = two["containers"] * 2
        missing = self.base()
        del missing["containers"]
        for config in (empty, two, missing):
            with self.subTest(containers=config.get("containers")):
                with self.assertRaises(promote.PromotionError):
                    promote.build_promotion_config(
                        config, environment="production", image=self.REFERENCE
                    )

    def test_refuses_a_missing_named_environment(self) -> None:
        config = self.base()
        del config["env"]["staging"]
        with self.assertRaises(promote.PromotionError):
            promote.build_promotion_config(
                config, environment="staging", image=self.REFERENCE
            )


class JsoncTests(unittest.TestCase):
    def test_comments_are_stripped(self) -> None:
        text = """{
          // leading comment
          "a": 1, /* inline */
          "b": 2
        }"""
        self.assertEqual(promote.load_jsonc(text), {"a": 1, "b": 2})

    def test_a_url_inside_a_string_survives(self) -> None:
        text = '{"url": "https://api.dry.yemelianov.dev", "path": "a/*b*/c"}'
        self.assertEqual(
            promote.load_jsonc(text),
            {"url": "https://api.dry.yemelianov.dev", "path": "a/*b*/c"},
        )

    def test_an_escaped_quote_does_not_end_the_string(self) -> None:
        text = '{"quote": "a\\"// not a comment", "b": 1}'
        self.assertEqual(promote.load_jsonc(text), {"quote": 'a"// not a comment', "b": 1})

    def test_the_committed_worker_config_parses(self) -> None:
        config = promote.load_jsonc(
            (ROOT / "services" / "cloud" / "wrangler.jsonc").read_text(encoding="utf-8")
        )
        self.assertEqual(config["name"], "dry-cloud")
        self.assertEqual(config["vars"]["REGISTRY_URL"], "https://api.dry.yemelianov.dev")
        self.assertEqual(len(config["containers"]), 1)
        self.assertEqual(len(config["env"]["staging"]["containers"]), 1)


class EvidenceTests(unittest.TestCase):
    def record(self, **overrides) -> dict:
        arguments = {
            "environment": "production",
            "release": "v0.11.0",
            "source_commit": COMMIT,
            "source_image": IMAGE,
            "source_digest": DIGEST,
            "promoted_image": PromotionConfigTests.REFERENCE,
        }
        arguments.update(overrides)
        return promote.evidence_record(**arguments)

    def test_record_carries_every_required_field(self) -> None:
        record = self.record()
        for key in (
            "release",
            "source_commit",
            "source_image_digest",
            "environment",
            "environment_revision",
        ):
            self.assertIn(key, record)
        self.assertEqual(record["source_image_digest"], DIGEST)
        self.assertIsNone(record["environment_revision"])

    def test_incomplete_evidence_fails_closed(self) -> None:
        with self.assertRaises(promote.PromotionError):
            promote.require_complete_evidence(self.record())

    def test_evidence_completes_with_an_environment_revision(self) -> None:
        record = self.record()
        completed = promote.finalize_evidence(record, environment_revision="7")
        promote.require_complete_evidence(completed)
        self.assertEqual(completed["environment_revision"], "7")

    def test_a_blank_revision_does_not_complete_the_record(self) -> None:
        for revision in ("", "   "):
            with self.subTest(revision=revision):
                with self.assertRaises(promote.PromotionError):
                    promote.finalize_evidence(self.record(), environment_revision=revision)


class CommandLineTests(unittest.TestCase):
    def run_tool(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(ROOT / "tools" / "promote_runner_image.py"), *args],
            text=True,
            capture_output=True,
            check=False,
        )

    def test_render_pins_the_committed_config(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory) / "wrangler.promotion.jsonc"
            result = self.run_tool(
                "render",
                "--environment",
                "staging",
                "--digest",
                DIGEST,
                "--account-id",
                "acct123",
                "--config",
                str(ROOT / "services" / "cloud" / "wrangler.jsonc"),
                "--out-config",
                str(out),
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            written = json.loads(out.read_text(encoding="utf-8"))
            container = written["env"]["staging"]["containers"][0]
            self.assertEqual(
                container["image"],
                "registry.cloudflare.com/acct123/dry-verify-runner:sha256-" + "ab" * 32,
            )
            self.assertNotIn("image_build_context", container)
            self.assertEqual(container["instance_type"], "standard-3")

    def test_render_refuses_a_mutable_digest(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            result = self.run_tool(
                "render",
                "--environment",
                "production",
                "--digest",
                "latest",
                "--account-id",
                "acct123",
                "--out-config",
                str(Path(directory) / "out.jsonc"),
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("immutable sha256 digest", result.stderr)

    def test_finalize_rejects_a_missing_revision(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory) / "evidence.json"
            evidence.write_text(
                json.dumps(
                    promote.evidence_record(
                        environment="staging",
                        release="refs/heads/main",
                        source_commit=COMMIT,
                        source_image=IMAGE,
                        source_digest=DIGEST,
                        promoted_image=PromotionConfigTests.REFERENCE,
                    )
                ),
                encoding="utf-8",
            )
            result = self.run_tool("finalize", "--evidence", str(evidence), "--revision", "")
            self.assertEqual(result.returncode, 1, result.stdout)
            self.assertIn("environment revision", result.stderr)

    def test_finalize_writes_a_complete_record(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory) / "evidence.json"
            evidence.write_text(
                json.dumps(
                    promote.evidence_record(
                        environment="staging",
                        release="refs/heads/main",
                        source_commit=COMMIT,
                        source_image=IMAGE,
                        source_digest=DIGEST,
                        promoted_image=PromotionConfigTests.REFERENCE,
                    )
                ),
                encoding="utf-8",
            )
            result = self.run_tool("finalize", "--evidence", str(evidence), "--revision", "12")
            self.assertEqual(result.returncode, 0, result.stderr)
            written = json.loads(evidence.read_text(encoding="utf-8"))
            self.assertEqual(written["environment_revision"], "12")


if __name__ == "__main__":
    unittest.main()

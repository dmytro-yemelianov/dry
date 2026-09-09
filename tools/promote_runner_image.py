#!/usr/bin/env python3
"""Bind a hosted deployment to one attested verify-runner image digest.

Issue #305. A deployment must never promote a mutable tag. This gate resolves
the release-named tag in GHCR to its immutable digest, requires a signed build
provenance attestation that names the same digest, the same source commit and
the builder workflow in this repository, then emits both the Wrangler promotion
config and the deployment evidence record.

Cloudflare Containers pull only from the Cloudflare managed registry, Docker
Hub, Amazon ECR and Google Artifact Registry -- never from GHCR. The verified
GHCR image is therefore re-pushed to ``registry.cloudflare.com`` under a tag
derived from the source digest, so the deployed reference stays immutable and
traceable back to the attested artifact.

The gate fails closed: any missing, unparseable or mismatched input is an
error, never a warning.
"""

from __future__ import annotations

import argparse
import datetime as _datetime
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Callable, Iterable


ROOT = Path(__file__).resolve().parents[1]
DIGEST_PATTERN = re.compile(r"^sha256:[0-9a-f]{64}$")
COMMIT_PATTERN = re.compile(r"^[0-9a-f]{40}$")
RELEASE_TAG_PATTERN = re.compile(r"^(?:refs/tags/)?v(\d+\.\d+\.\d+)$")
CLOUDFLARE_REGISTRY = "registry.cloudflare.com"
DEFAULT_WORKFLOW = ".github/workflows/verify-runner.yml"
EVIDENCE_SCHEMA = "dry.deployment-evidence.v1"
REQUIRED_EVIDENCE_FIELDS = (
    "environment",
    "release",
    "source_commit",
    "source_image",
    "source_image_digest",
    "promoted_image",
    "environment_revision",
)


class PromotionError(ValueError):
    """A promotion input is missing, malformed or does not match the release."""


# --- reference and tag rules -------------------------------------------------


def require_digest(value: object) -> str:
    if not isinstance(value, str) or not DIGEST_PATTERN.match(value):
        raise PromotionError(f"not an immutable sha256 digest: {value!r}")
    return value


def require_immutable_reference(reference: str) -> tuple[str, str]:
    """Split ``repository@sha256:...`` and refuse any tag-only reference."""
    if not isinstance(reference, str) or "@" not in reference:
        raise PromotionError(
            f"deployment reference must be pinned by digest, not by tag: {reference!r}"
        )
    repository, _, digest = reference.partition("@")
    if not repository:
        raise PromotionError(f"deployment reference has no repository: {reference!r}")
    return repository, require_digest(digest)


def expected_release_tag(environment: str, release_ref: str, source_commit: str) -> str:
    """Return the registry tag the named release is expected to publish."""
    if environment == "production":
        match = RELEASE_TAG_PATTERN.match(release_ref or "")
        if not match:
            raise PromotionError(
                f"production promotion requires a vX.Y.Z release tag, got {release_ref!r}"
            )
        return match.group(1)
    if environment == "staging":
        if not COMMIT_PATTERN.match(source_commit or ""):
            raise PromotionError(
                f"staging promotion requires a full 40-character commit, got {source_commit!r}"
            )
        return f"sha-{source_commit[:7]}"
    raise PromotionError(f"unknown promotion environment: {environment!r}")


def expected_workflow_ref(environment: str, release_ref: str) -> str:
    """Return the git ref the builder workflow must have run from."""
    if environment == "production":
        match = RELEASE_TAG_PATTERN.match(release_ref or "")
        if not match:
            raise PromotionError(
                f"production promotion requires a vX.Y.Z release tag, got {release_ref!r}"
            )
        return f"refs/tags/v{match.group(1)}"
    if environment == "staging":
        if release_ref not in ("refs/heads/main", "main"):
            raise PromotionError(
                f"staging promotes only main, got {release_ref!r}"
            )
        return "refs/heads/main"
    raise PromotionError(f"unknown promotion environment: {environment!r}")


def cloudflare_image_reference(account_id: str, name: str, digest: str) -> str:
    """Derive the immutable Cloudflare registry tag for a verified source digest."""
    if not account_id:
        raise PromotionError("a Cloudflare account id is required to promote an image")
    if not name:
        raise PromotionError("a container image name is required to promote an image")
    algorithm, _, hexdigest = require_digest(digest).partition(":")
    return f"{CLOUDFLARE_REGISTRY}/{account_id}/{name}:{algorithm}-{hexdigest}"


# --- attestation and release evidence ----------------------------------------


def _entries(payload: object) -> list[dict]:
    if isinstance(payload, dict):
        return [payload]
    if isinstance(payload, list):
        return [entry for entry in payload if isinstance(entry, dict)]
    return []


def _statement(entry: dict) -> dict:
    result = entry.get("verificationResult")
    if isinstance(result, dict) and isinstance(result.get("statement"), dict):
        return result["statement"]
    return entry.get("statement") if isinstance(entry.get("statement"), dict) else {}


def _subject_digests(statement: dict) -> set[str]:
    digests = set()
    for subject in statement.get("subject") or []:
        if not isinstance(subject, dict):
            continue
        value = (subject.get("digest") or {}).get("sha256")
        if isinstance(value, str):
            digests.add(f"sha256:{value}")
    return digests


def _build_definition(statement: dict) -> dict:
    predicate = statement.get("predicate")
    if isinstance(predicate, dict) and isinstance(predicate.get("buildDefinition"), dict):
        return predicate["buildDefinition"]
    return {}


def _source_commits(build_definition: dict) -> set[str]:
    commits = set()
    for dependency in build_definition.get("resolvedDependencies") or []:
        if not isinstance(dependency, dict):
            continue
        value = (dependency.get("digest") or {}).get("gitCommit")
        if isinstance(value, str):
            commits.add(value)
    return commits


def verify_attestation_payload(
    payload: object,
    *,
    digest: str,
    repository: str,
    source_commit: str,
    workflow_path: str,
    workflow_ref: str | None = None,
    allow_release_tags: bool = False,
) -> dict:
    """Return the attestation statement that names exactly this build, or fail."""
    require_digest(digest)
    entries = _entries(payload)
    if not entries:
        raise PromotionError("no build provenance attestation was returned for the image")

    reasons: list[str] = []
    for entry in entries:
        statement = _statement(entry)
        if digest not in _subject_digests(statement):
            reasons.append("attested subject digest does not match the resolved digest")
            continue
        build_definition = _build_definition(statement)
        workflow = (build_definition.get("externalParameters") or {}).get("workflow") or {}
        attested_repository = str(workflow.get("repository") or "")
        if not attested_repository.endswith(f"/{repository}"):
            reasons.append(f"attested source repository {attested_repository!r} is not {repository!r}")
            continue
        if workflow.get("path") != workflow_path:
            reasons.append(f"attested builder workflow {workflow.get('path')!r} is not {workflow_path!r}")
            continue
        if workflow_ref is not None:
            ref = str(workflow.get("ref") or "")
            ref_ok = (ref == workflow_ref) or (
                allow_release_tags and bool(RELEASE_TAG_PATTERN.match(ref))
            )
            if not ref_ok:
                expected_desc = (
                    f"{workflow_ref!r} or a release tag"
                    if allow_release_tags
                    else f"{workflow_ref!r}"
                )
                reasons.append(f"attested builder ref {ref!r} is not {expected_desc}")
                continue
        if source_commit not in _source_commits(build_definition):
            reasons.append("attested source commit does not match the promoted commit")
            continue
        return statement

    raise PromotionError("; ".join(dict.fromkeys(reasons)) or "no attestation matched the release")


def release_evidence_digest(payload: object, *, image: str) -> str:
    """Read the digest a release recorded for this image."""
    if not isinstance(payload, dict):
        raise PromotionError("release image evidence must be a JSON object")
    recorded_image = payload.get("image")
    if recorded_image != image:
        raise PromotionError(
            f"release evidence names image {recorded_image!r}, expected {image!r}"
        )
    return require_digest(payload.get("digest"))


# --- Wrangler configuration --------------------------------------------------


def load_jsonc(text: str) -> Any:
    """Parse JSON with ``//`` and ``/* */`` comments, preserving string bodies."""
    out: list[str] = []
    index = 0
    length = len(text)
    while index < length:
        character = text[index]
        if character == '"':
            out.append(character)
            index += 1
            while index < length:
                current = text[index]
                out.append(current)
                index += 1
                if current == "\\" and index < length:
                    out.append(text[index])
                    index += 1
                elif current == '"':
                    break
            continue
        if text.startswith("//", index):
            while index < length and text[index] != "\n":
                index += 1
            continue
        if text.startswith("/*", index):
            end = text.find("*/", index + 2)
            index = length if end == -1 else end + 2
            continue
        out.append(character)
        index += 1
    return json.loads("".join(out))


def _container_holder(config: dict, environment: str) -> dict:
    if environment == "production":
        return config
    if environment == "staging":
        environments = config.get("env")
        if not isinstance(environments, dict) or not isinstance(environments.get("staging"), dict):
            raise PromotionError("the Worker config has no staging environment to promote")
        return environments["staging"]
    raise PromotionError(f"unknown promotion environment: {environment!r}")


def build_promotion_config(config: dict, *, environment: str, image: str) -> dict:
    """Return a copy of the Worker config pinned to one registry image."""
    if not image.startswith(f"{CLOUDFLARE_REGISTRY}/"):
        raise PromotionError(
            f"Cloudflare pulls container images only from {CLOUDFLARE_REGISTRY}, got {image!r}"
        )
    _, _, tag = image.rpartition(":")
    if not re.match(r"^sha256-[0-9a-f]{64}$", tag):
        raise PromotionError(f"promotion image tag must be digest-derived, got {tag!r}")
    if not isinstance(config, dict):
        raise PromotionError("the Worker config must be a JSON object")

    promoted = json.loads(json.dumps(config))
    holder = _container_holder(promoted, environment)
    containers = holder.get("containers")
    if not isinstance(containers, list) or len(containers) != 1:
        raise PromotionError(
            f"{environment} must declare exactly one container to promote, "
            f"found {len(containers) if isinstance(containers, list) else 'none'}"
        )
    container = containers[0]
    if not isinstance(container, dict):
        raise PromotionError(f"{environment} container entry must be a JSON object")
    container["image"] = image
    container.pop("image_build_context", None)
    return promoted


# --- deployment evidence -----------------------------------------------------


def evidence_record(
    *,
    environment: str,
    release: str,
    source_commit: str,
    source_image: str,
    source_digest: str,
    promoted_image: str,
    generated_at: str | None = None,
) -> dict:
    return {
        "schema": EVIDENCE_SCHEMA,
        "environment": environment,
        "release": release,
        "source_commit": source_commit,
        "source_image": source_image,
        "source_image_digest": require_digest(source_digest),
        "promoted_image": promoted_image,
        "environment_revision": None,
        "generated_at": generated_at
        or _datetime.datetime.now(_datetime.timezone.utc).isoformat(timespec="seconds"),
    }


def finalize_evidence(record: dict, *, environment_revision: str) -> dict:
    if not isinstance(environment_revision, str) or not environment_revision.strip():
        raise PromotionError("a deployed environment revision is required to close the evidence")
    completed = dict(record)
    completed["environment_revision"] = environment_revision.strip()
    require_complete_evidence(completed)
    return completed


def require_complete_evidence(record: dict) -> dict:
    if not isinstance(record, dict):
        raise PromotionError("deployment evidence must be a JSON object")
    missing = [
        field
        for field in REQUIRED_EVIDENCE_FIELDS
        if not str(record.get(field) or "").strip()
    ]
    if missing:
        raise PromotionError(f"deployment evidence is incomplete: missing {', '.join(missing)}")
    require_digest(record["source_image_digest"])
    return record


# --- registry and attestation I/O --------------------------------------------


MANIFEST_ACCEPT = ", ".join(
    (
        "application/vnd.oci.image.index.v1+json",
        "application/vnd.oci.image.manifest.v1+json",
        "application/vnd.docker.distribution.manifest.list.v2+json",
        "application/vnd.docker.distribution.manifest.v2+json",
    )
)


def _registry_token(registry: str, repository_path: str, github_token: str | None) -> str:
    url = f"https://{registry}/token?service={registry}&scope=repository:{repository_path}:pull"
    request = urllib.request.Request(url)
    if github_token:
        import base64

        basic = base64.b64encode(f"x:{github_token}".encode()).decode()
        request.add_header("Authorization", f"Basic {basic}")
    with urllib.request.urlopen(request, timeout=30) as response:  # noqa: S310
        payload = json.loads(response.read().decode("utf-8"))
    token = payload.get("token") or payload.get("access_token")
    if not token:
        raise PromotionError(f"{registry} did not issue a pull token for {repository_path}")
    return str(token)


def resolve_tag_digest(image: str, tag: str, github_token: str | None = None) -> str:
    """Resolve ``image:tag`` to the registry's immutable content digest."""
    registry, _, repository_path = image.partition("/")
    if not repository_path:
        raise PromotionError(f"image must include a registry host: {image!r}")
    token = _registry_token(registry, repository_path, github_token)
    url = f"https://{registry}/v2/{repository_path}/manifests/{tag}"
    request = urllib.request.Request(url, method="HEAD")
    request.add_header("Accept", MANIFEST_ACCEPT)
    request.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(request, timeout=30) as response:  # noqa: S310
            digest = response.headers.get("Docker-Content-Digest")
    except urllib.error.HTTPError as exc:
        raise PromotionError(f"{image}:{tag} is not published: HTTP {exc.code}") from exc
    if not digest:
        raise PromotionError(f"{image}:{tag} returned no Docker-Content-Digest header")
    return require_digest(digest)


def fetch_attestation(
    image: str,
    digest: str,
    repository: str,
    runner: Callable[[list[str]], subprocess.CompletedProcess[str]] | None = None,
) -> object:
    command = [
        "gh",
        "attestation",
        "verify",
        f"oci://{image}@{digest}",
        "--repo",
        repository,
        "--format",
        "json",
    ]
    run = runner or (
        lambda argv: subprocess.run(argv, text=True, capture_output=True, check=False)
    )
    completed = run(command)
    if completed.returncode != 0:
        raise PromotionError(
            f"build provenance verification failed for {image}@{digest}: "
            f"{(completed.stderr or completed.stdout or '').strip()}"
        )
    try:
        return json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise PromotionError("gh returned unparseable attestation output") from exc


# --- commands ----------------------------------------------------------------


def _write_json(path: Path, payload: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def render_promotion_config(
    *, config_path: str, out_config: str, environment: str, image: str
) -> None:
    config = load_jsonc(Path(config_path).read_text(encoding="utf-8"))
    _write_json(
        Path(out_config),
        build_promotion_config(config, environment=environment, image=image),
    )


def command_render(args: argparse.Namespace) -> int:
    image = cloudflare_image_reference(args.account_id, args.image_name, args.digest)
    render_promotion_config(
        config_path=args.config,
        out_config=args.out_config,
        environment=args.environment,
        image=image,
    )
    print(f"{args.environment} promotion config pinned to {image}")
    return 0


def registry_token_from_environment() -> str | None:
    """Read the registry credential from the environment, never from argv.

    A token passed as a command-line argument is visible to every process on the
    runner and to anything that echoes the command.
    """
    for name in ("GH_TOKEN", "GITHUB_TOKEN"):
        value = os.environ.get(name)
        if value:
            return value
    return None


def command_resolve(args: argparse.Namespace) -> int:
    tag = expected_release_tag(args.environment, args.release_ref, args.source_commit)
    digest = resolve_tag_digest(args.image, tag, registry_token_from_environment())

    verify_attestation_payload(
        fetch_attestation(args.image, digest, args.repository),
        digest=digest,
        repository=args.repository,
        source_commit=args.source_commit,
        workflow_path=args.workflow,
        workflow_ref=expected_workflow_ref(args.environment, args.release_ref),
        allow_release_tags=(args.environment == "staging"),
    )

    if args.release_evidence:
        recorded = release_evidence_digest(
            json.loads(Path(args.release_evidence).read_text(encoding="utf-8")),
            image=args.image,
        )
        if recorded != digest:
            raise PromotionError(
                f"release evidence names {recorded}, but {args.image}:{tag} resolves to {digest}"
            )

    promoted_image = cloudflare_image_reference(
        args.account_id, args.image.rsplit("/", 1)[-1], digest
    )
    render_promotion_config(
        config_path=args.config,
        out_config=args.out_config,
        environment=args.environment,
        image=promoted_image,
    )
    _write_json(
        Path(args.evidence),
        evidence_record(
            environment=args.environment,
            release=args.release_ref,
            source_commit=args.source_commit,
            source_image=args.image,
            source_digest=digest,
            promoted_image=promoted_image,
        ),
    )

    print(f"source image:   {args.image}@{digest}")
    print(f"promoted image: {promoted_image}")
    print(f"release:        {args.release_ref} ({args.environment})")
    print(f"source commit:  {args.source_commit}")
    if args.github_output:
        with open(args.github_output, "a", encoding="utf-8") as handle:
            handle.write(f"source_digest={digest}\n")
            handle.write(f"source_reference={args.image}@{digest}\n")
            handle.write(f"promoted_image={promoted_image}\n")
    return 0


def command_finalize(args: argparse.Namespace) -> int:
    path = Path(args.evidence)
    record = json.loads(path.read_text(encoding="utf-8"))
    _write_json(path, finalize_evidence(record, environment_revision=args.revision))
    print(f"deployment evidence complete: {path}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    resolve = commands.add_parser("resolve", help="verify and pin the release image digest")
    resolve.add_argument("--repository", required=True, help="owner/name of the source repository")
    resolve.add_argument("--image", required=True, help="registry image, without a tag")
    resolve.add_argument("--environment", required=True, choices=("staging", "production"))
    resolve.add_argument("--release-ref", required=True, help="the promoted tag or branch ref")
    resolve.add_argument("--source-commit", required=True, help="full 40-character commit sha")
    resolve.add_argument("--account-id", required=True, help="Cloudflare account id")
    resolve.add_argument("--config", default=str(ROOT / "services/cloud/wrangler.jsonc"))
    resolve.add_argument("--out-config", required=True, help="generated promotion config path")
    resolve.add_argument("--evidence", required=True, help="deployment evidence output path")
    resolve.add_argument("--workflow", default=DEFAULT_WORKFLOW, help="expected builder workflow")
    resolve.add_argument("--release-evidence", help="release image evidence JSON to cross-check")
    resolve.add_argument("--github-output", help="GITHUB_OUTPUT file to append resolved values to")
    resolve.set_defaults(handler=command_resolve)

    render = commands.add_parser(
        "render", help="write a promotion config for an already-known digest"
    )
    render.add_argument("--environment", required=True, choices=("staging", "production"))
    render.add_argument("--digest", required=True, help="immutable sha256 image digest")
    render.add_argument("--account-id", required=True, help="Cloudflare account id")
    render.add_argument("--image-name", default="dry-verify-runner")
    render.add_argument("--config", default=str(ROOT / "services/cloud/wrangler.jsonc"))
    render.add_argument("--out-config", required=True)
    render.set_defaults(handler=command_render)

    finalize = commands.add_parser("finalize", help="close the evidence with the deployed revision")
    finalize.add_argument("--evidence", required=True)
    finalize.add_argument("--revision", required=True, help="deployed environment revision or version id")
    finalize.set_defaults(handler=command_finalize)

    return parser


def main(argv: Iterable[str] | None = None) -> int:
    args = build_parser().parse_args(list(argv) if argv is not None else None)
    try:
        return args.handler(args)
    except PromotionError as error:
        print(f"image promotion refused: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

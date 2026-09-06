#!/usr/bin/env python3
"""Fail if the deployment path can promote an unverified container image.

Issue #305. ``tools/promote_runner_image.py`` only helps if the workflows keep
using it. This gate pins the surrounding contract:

* the builder workflow publishes a semver-named image for a release, pins every
  action by commit, and attests the pushed digest to the registry;
* the deploy job resolves and verifies that digest before anything is deployed,
  deploys only through the generated digest-pinned promotion config, and closes
  the deployment evidence with the environment revision.

Textual assertions keep this runnable from the dependency-free security job.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


DEFAULT_ROOT = Path(__file__).resolve().parents[1]
BUILDER_WORKFLOW = ".github/workflows/verify-runner.yml"
DEPLOY_WORKFLOW = ".github/workflows/deploy-verify.yml"
PINNED_USES = re.compile(r"^\s*(?:-\s*)?uses:\s*(\S+)", re.MULTILINE)
ACTION_PIN = re.compile(r"^[^@]+@[0-9a-f]{40}$")
JOB_HEADING = re.compile(r"^  ([A-Za-z0-9_-]+):$", re.MULTILINE)


class PolicyError(AssertionError):
    """The promotion contract is not enforced by the workflows."""


def strip_comments(text: str) -> str:
    """Blank whole-line comments so prose can never satisfy a policy marker."""
    return "\n".join(
        "" if line.lstrip().startswith("#") else line for line in text.split("\n")
    )


def read(root: Path, relative: str) -> str:
    path = root / relative
    if not path.is_file():
        raise PolicyError(f"missing required workflow: {relative}")
    return strip_comments(path.read_text(encoding="utf-8"))


def require(text: str, needle: str, *, where: str) -> None:
    if needle not in text:
        raise PolicyError(f"{where} is missing required promotion marker: {needle!r}")


def require_pinned_actions(text: str, *, where: str) -> None:
    for reference in PINNED_USES.findall(text):
        if reference.startswith("./") or reference.startswith("."):
            continue
        if not ACTION_PIN.match(reference):
            raise PolicyError(f"{where} uses an unpinned action: {reference}")


def job_block(text: str, name: str, *, where: str) -> str:
    """Return one top-level job's body, so rules cannot leak between jobs."""
    headings = [(match.start(), match.group(1)) for match in JOB_HEADING.finditer(text)]
    for index, (start, heading) in enumerate(headings):
        if heading != name:
            continue
        end = headings[index + 1][0] if index + 1 < len(headings) else len(text)
        return text[start:end]
    raise PolicyError(f"{where} has no {name!r} job")


def check_builder(root: Path) -> None:
    text = read(root, BUILDER_WORKFLOW)
    where = BUILDER_WORKFLOW
    require_pinned_actions(text, where=where)
    require(text, '- "v*"', where=where)
    require(text, "type=semver,pattern={{version}}", where=where)
    require(text, "actions/attest-build-provenance@", where=where)
    require(text, "subject-digest: ${{ steps.build.outputs.digest }}", where=where)
    require(text, "push-to-registry: true", where=where)
    build = job_block(text, "docker-build", where=where)
    for permission in ("id-token: write", "attestations: write"):
        require(build, permission, where=f"{where} docker-build")


def check_deploy(root: Path) -> None:
    text = read(root, DEPLOY_WORKFLOW)
    where = DEPLOY_WORKFLOW
    require_pinned_actions(text, where=where)
    deploy = job_block(text, "deploy", where=where)

    require(deploy, "tools/promote_runner_image.py resolve", where=f"{where} deploy")
    require(deploy, "tools/promote_runner_image.py finalize", where=f"{where} deploy")
    for permission in ("packages: read", "attestations: read"):
        require(deploy, permission, where=f"{where} deploy")

    # A credential in argv is readable by every process on the runner.
    for argument in ("--github-token", "--gh-token"):
        if argument in deploy:
            raise PolicyError(
                f"{where} deploy passes a registry credential as an argument: {argument}"
            )

    resolve_at = deploy.index("tools/promote_runner_image.py resolve")
    for match in re.finditer(r"wrangler deploy(?P<flags>[^\n|]*)", deploy):
        flags = match.group("flags")
        if match.start() < resolve_at:
            raise PolicyError(
                f"{where} deploy runs `wrangler deploy` before the promotion gate"
            )
        if "-c " not in flags and "--config" not in flags:
            raise PolicyError(
                f"{where} deploy runs `wrangler deploy` without the digest-pinned "
                f"promotion config: `wrangler deploy{flags}`"
            )

    finalize_at = deploy.index("tools/promote_runner_image.py finalize")
    last_deploy = max(match.start() for match in re.finditer(r"wrangler deploy", deploy))
    if finalize_at < last_deploy:
        raise PolicyError(f"{where} deploy closes its evidence before deploying")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=str(DEFAULT_ROOT), help="repository root to check")
    args = parser.parse_args(argv)
    root = Path(args.root)
    try:
        check_builder(root)
        check_deploy(root)
    except PolicyError as error:
        print(f"image promotion policy: {error}", file=sys.stderr)
        return 1
    print("image promotion policy: releases publish attested digests and deploys pin them")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

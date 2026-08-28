#!/usr/bin/env python3
"""Validate repository agent routing and role contracts without external deps."""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
AGENTS_FILE = ROOT / "AGENTS.md"
AGENT_DIR = ROOT / ".claude" / "agents"

EXPECTED_ROLES = {
    "architect",
    "delivery-lead",
    "kernel-engineer",
    "product-owner",
    "qa-assurance",
    "ralph-loop",
    "reviewer",
    "routine-dev",
    "scout",
}
READ_ONLY_ROLES = {
    "architect",
    "delivery-lead",
    "product-owner",
    "qa-assurance",
    "ralph-loop",
    "reviewer",
    "scout",
}
IMPLEMENTATION_ROLES = {"kernel-engineer", "routine-dev"}
REQUIRED_HEADINGS = {
    "## Inputs",
    "## Source of truth",
    "## Authority and prohibited actions",
    "## Graph-first workflow",
    "## Outputs",
    "## Handoffs and escalation",
    "## Exit criteria",
}
GRAPH_TOOLS = {
    "mcp__codebase_memory_mcp__search_graph",
    "mcp__codebase_memory_mcp__trace_path",
}
STANDALONE_ROOTS = (
    "crates/wasm",
    "crates/cloud",
    "py",
    "containers/verify-runner",
)
REQUIRED_CI_JOBS = {"python-sdk", "wasm", "cloud", "verify-runner"}
ALLOWED_MODELS = {"claude-opus-5", "sonnet", "haiku", "inherit"}
FORBIDDEN_STALE_PHRASES = (
    "only `crates/wasm` and `py/` have CI jobs",
    "all 8 package manifests",
    "zero false negatives against physical and kinematic constraints",
)


def parse_frontmatter(path: Path) -> tuple[dict[str, str], str]:
    text = path.read_text(encoding="utf-8")
    if not text.startswith("---\n"):
        raise ValueError("missing opening frontmatter delimiter")
    try:
        raw_frontmatter, body = text[4:].split("\n---\n", 1)
    except ValueError as error:
        raise ValueError("missing closing frontmatter delimiter") from error

    values: dict[str, str] = {}
    for line in raw_frontmatter.splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if ":" not in line:
            raise ValueError(f"invalid frontmatter line: {line!r}")
        key, value = line.split(":", 1)
        values[key.strip()] = value.strip()
    return values, body


def main() -> int:
    errors: list[str] = []
    agents_text = AGENTS_FILE.read_text(encoding="utf-8")
    routed_roles = set(re.findall(r"^\| `([^`]+)` \|", agents_text, re.MULTILINE))
    files = {path.stem: path for path in AGENT_DIR.glob("*.md")}

    if routed_roles != EXPECTED_ROLES:
        errors.append(
            f"AGENTS.md role set differs: expected={sorted(EXPECTED_ROLES)} "
            f"actual={sorted(routed_roles)}"
        )
    if set(files) != EXPECTED_ROLES:
        errors.append(
            f"agent file set differs: expected={sorted(EXPECTED_ROLES)} "
            f"actual={sorted(files)}"
        )

    for role in sorted(EXPECTED_ROLES & set(files)):
        path = files[role]
        try:
            frontmatter, body = parse_frontmatter(path)
        except ValueError as error:
            errors.append(f"{path.relative_to(ROOT)}: {error}")
            continue

        if frontmatter.get("name") != role:
            errors.append(
                f"{path.relative_to(ROOT)}: frontmatter name "
                f"{frontmatter.get('name')!r} does not match filename"
            )
        for key in ("description", "tools", "model", "effort"):
            if not frontmatter.get(key):
                errors.append(f"{path.relative_to(ROOT)}: missing frontmatter {key!r}")
        if frontmatter.get("model") not in ALLOWED_MODELS:
            errors.append(
                f"{path.relative_to(ROOT)}: unsupported model "
                f"{frontmatter.get('model')!r}"
            )

        tools = {item.strip() for item in frontmatter.get("tools", "").split(",")}
        missing_graph_tools = GRAPH_TOOLS - tools
        if missing_graph_tools:
            errors.append(
                f"{path.relative_to(ROOT)}: missing graph tools "
                f"{sorted(missing_graph_tools)}"
            )
        if role in READ_ONLY_ROLES and tools & {"Edit", "Write"}:
            errors.append(f"{path.relative_to(ROOT)}: read-only role exposes Edit/Write")
        if role in IMPLEMENTATION_ROLES and not {"Edit", "Write"}.issubset(tools):
            errors.append(
                f"{path.relative_to(ROOT)}: implementation role must expose Edit and Write"
            )

        missing_headings = REQUIRED_HEADINGS - set(re.findall(r"^## .+$", body, re.MULTILINE))
        if missing_headings:
            errors.append(
                f"{path.relative_to(ROOT)}: missing contract headings "
                f"{sorted(missing_headings)}"
            )

        for phrase in FORBIDDEN_STALE_PHRASES:
            if phrase in body:
                errors.append(f"{path.relative_to(ROOT)}: contains stale phrase {phrase!r}")

    routine_text = files.get("routine-dev", Path()).read_text(encoding="utf-8") if files.get("routine-dev") else ""
    for relative_root in STANDALONE_ROOTS:
        root = ROOT / relative_root
        for filename in ("Cargo.toml", "Cargo.lock"):
            if not (root / filename).is_file():
                errors.append(f"{relative_root}: missing {filename}")
        if relative_root not in routine_text:
            errors.append(f"routine-dev does not own standalone root {relative_root}")
        if relative_root not in agents_text:
            errors.append(f"AGENTS.md does not mention standalone root {relative_root}")

    ci_text = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
    ci_jobs = set(re.findall(r"^  ([A-Za-z0-9_-]+):\s*$", ci_text, re.MULTILINE))
    missing_jobs = REQUIRED_CI_JOBS - ci_jobs
    if missing_jobs:
        errors.append(f"CI is missing required standalone jobs: {sorted(missing_jobs)}")

    combined_contracts = "\n".join(path.read_text(encoding="utf-8") for path in files.values())
    if "services/cloud" not in combined_contracts or "no dedicated CI job" not in combined_contracts:
        errors.append("agent contracts must record services/cloud as a local-only CI gap")

    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        print(f"agent contract validation failed with {len(errors)} error(s)", file=sys.stderr)
        return 1

    print(
        "agent contracts valid: "
        f"{len(EXPECTED_ROLES)} roles, {len(STANDALONE_ROOTS)} standalone Cargo roots, "
        f"{len(REQUIRED_CI_JOBS)} required CI jobs"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

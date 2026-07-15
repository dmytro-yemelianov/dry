# Design: Release engineering (Slice B)

**Date:** 2026-06-29
**Status:** Approved (batch directive "take B-E") — tracked in GitHub issues
**Branch:** `feat/release-engineering` (off main, after the A+D stack merged)
**Source docs:** `docs/08-production-transition.md` (§WS2), `docs/09-customer-readiness.md` (task #2).

## Goal

A repeatable, tagged release that produces installable artifacts so a clean machine can run the CLI,
Python and TypeScript front-ends **without building from source**, and CI can reproduce those artifacts
from a tag.

## Default decisions

- **Build + attach, publish gated.** Every release always builds artifacts and attaches them to the
  GitHub Release with sha256 checksums. Registry *publish* steps (PyPI, npm) are guarded by
  `if: ${{ secrets.X != '' }}` so the pipeline works today without registry tokens and starts publishing
  the moment secrets are added.
- **Hand-rolled workflow** using well-known actions (`softprops/action-gh-release`,
  `PyO3/maturin-action`, `actions/setup-node`), not a new umbrella tool — consistent with the repo's lean
  dependency posture.
- **Tag drives the release; versions are checked, not rewritten.** A `v{X.Y.Z}` tag must match the
  workspace `version`; a guard step fails the release on mismatch. Bumping versions is a deliberate commit
  before tagging (documented in `docs/12-releasing.md`).

## Artifacts

| Path | What |
|---|---|
| `.github/workflows/release.yml` | tag-triggered (`v*`) build/release pipeline |
| `CHANGELOG.md` | Keep a Changelog format; 0.2.0 entry covering the IR v0 spec/vectors (slice A) and the profile/report contract + verify severity behavior change (slice D) |
| `docs/12-releasing.md` | the release process + install-without-source instructions |
| `scripts/check-version.sh` | asserts a `vX.Y.Z` tag matches the workspace/py/ts versions (locally testable) |

## `release.yml` jobs

1. **guard** — require a private repository, parse the tag, and run `scripts/check-version.sh` (tag == Cargo workspace == pyproject == package.json). Fail fast on either mismatch.
2. **cli** — matrix: linux x86_64, macOS aarch64 + x86_64, windows x86_64. `cargo build --release -p dry-cli`, package as `.tar.gz`/`.zip`, emit `.sha256`.
3. **wheels** — `PyO3/maturin-action` building wheels (manylinux + macOS + windows) and an sdist for `py/`.
4. **ts** — `npm ci && npm run build` in `sdk/ts`, then `npm pack` to a tarball artifact.
5. **release** — create a private GitHub Release for the tag and attach all binaries + checksums + wheels + sdist + npm tarball. Public registry publishing is intentionally excluded.

## Local verifiability

- `release.yml` YAML parses; job/step structure validated.
- `scripts/check-version.sh` unit-exercised locally (matching + mismatching tags).
- `cargo build --release -p dry-cli` produces a runnable binary (smoke `dry --version`).
- `py/pyproject.toml` / `sdk/ts/package.json` metadata validated.
- **CI-only** (clearly noted): multi-platform binary/wheel builds and private GitHub Release assembly.

## Acceptance → 08·WS2 / 09 #2

- ✅ tagged release process (`release.yml` + `docs/12-releasing.md`)
- ✅ CLI artifacts for macOS/Linux/Windows; Python wheels (maturin); npm package
- ✅ checksums + changelog + release notes
- ✅ version/tag consistency guard so CI reproduces artifacts from a tag
- Install-without-source instructions documented for all three front-ends.

## Work breakdown (issues)

- Epic: Slice B — Release engineering.
- B1 `release.yml`; B2 `CHANGELOG.md`; B3 `docs/12-releasing.md`; B4 `scripts/check-version.sh` + version guard.

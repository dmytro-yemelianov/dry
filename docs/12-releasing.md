# Releasing Dry

Dry ships installable artifacts from one tagged release: the **CLI** (native binaries), the
**Python** package (wheels via maturin), the **TypeScript SDK** (npm package), the **AI MCP Server** (`@dry/mcp`),
the **WebAssembly Standalone Bundle**, and **The Dry Book & Specifications Archive**.
A release is produced entirely by CI from a `vX.Y.Z` tag — `.github/workflows/release.yml`.

---

## 1. Semantic Versioning Policy & Issue Lifecycle

Dry adheres strictly to **Semantic Versioning 2.0.0 (`MAJOR.MINOR.PATCH`)**:

```
  v MAJOR . MINOR . PATCH
      │       │       │
      │       │       └─► Bug fixes, security remediations, link & doc fixes (no API changes)
      │       └─────────► Strategic roadmap milestones, new CAM dialects, backward-compatible IR
      └─────────────────► Normative standard freeze, breaking dialect/API changes
```

### Issue & Defect Mapping
1. **GitHub Issues**: When a bug or defect is identified, create an issue with the appropriate label (`bug`, `security`, `kinematics`, `docs`).
2. **Milestone Assignment**: Assign every issue to its target resolution milestone (e.g. `v0.7.1` for immediate patches, `v0.8.0` for new capabilities).
3. **Conventional Commits**: Every PR and commit must follow the conventional commit format linking the issue:
   - `fix(verify): resolve out-of-bounds arc center check (#102)` $\rightarrow$ lands in `PATCH`
   - `feat(cam): add trochoidal pocket milling generator (#105)` $\rightarrow$ lands in `MINOR`
   - `feat(ir)!: migrate to L2 multi-frame toolpath representation` $\rightarrow$ lands in `MAJOR`
4. **Changelog Lifecycle**: Update [CHANGELOG.md](../CHANGELOG.md) under the `## [X.Y.Z]` header under `Added`, `Fixed`, `Changed`, `Deprecated`, or `Security`.

---

## 2. Cutting a release

1. **Bump versions** in lockstep (they must match the tag):
   - `Cargo.toml` → `[workspace.package] version`
   - `crates/wasm/Cargo.toml` → `[package] version`
   - `crates/cloud/Cargo.toml` → `[package] version`
   - `containers/verify-runner/Cargo.toml` → `[package] version`
   - `py/pyproject.toml` & `py/Cargo.toml` → `version`
   - `sdk/ts/package.json` & `sdk/ts/package-lock.json` → `version`
   - `sdk/mcp/package.json` → `version`
   - `web/package.json`, `services/cloud/package.json`, `tools/license-issuer/package.json` → `version`
   Verify locally: `bash scripts/check-version.sh vX.Y.Z`.
2. **Update `CHANGELOG.md`** — move `[Unreleased]` into a new `## [X.Y.Z] - <date>` section.
3. **Commit** the bump + changelog, open a PR, merge to `main`.
4. **Tag and push**:
   ```sh
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```
5. CI runs `release.yml`:
   - **guard** fails fast unless the tag is reachable from `main`, matches every published manifest
     and lockfile version, and has a changelog release heading;
   - **quality** reruns format/lint, all-feature Rust tests, conformance validators, proof mutation tests,
     and dependency audits on the tagged commit;
   - **cli** builds locked binaries for Linux x86_64, macOS aarch64 + x86_64, and Windows x86_64;
   - **wheels** / **sdist** build locked Python artifacts via maturin (manylinux + macOS + Windows);
   - **ts** / **mcp** audit dependencies, build, test, and pack the npm tarballs;
   - **wasm** builds the standalone WebAssembly distribution package;
   - **book** packages The Dry Book chapters and architectural specifications;
   - **release** creates the GitHub Release, attaches all artifacts, generates `SHA256SUMS`, merges CycloneDX 1.5 SBOM, and signs In-Toto SLSA Level 3 build provenance.

---

## 3. Distribution & Licensing Boundary

Artifacts are distributed through this repository's **public GitHub Releases** under the **Functional Source License, Version 1.1, MIT Future License (FSL-1.1-MIT)**. See [`LICENSE`](../LICENSE) and [`NOTICE`](../NOTICE).

---

## 4. Installing release artifacts

- **CLI** — download the `dry-<ver>-<target>.tar.gz` for your platform from the GitHub Release, verify it
  against `SHA256SUMS`, extract, and run `./dry --help`.
- **Python** — download the matching wheel from the release and run `pip install <wheel>`.
- **TypeScript** — download the npm tarball from the release and run `npm install <tarball>`.
- **AI MCP Server** — install `@dry/mcp` for Claude Desktop, Cursor, or Goose.
- **Verification Daemon** — pull the multi-arch container image `ghcr.io/dmytro-yemelianov/dry-verify-runner:latest`.

---

## 5. Compatibility & migration

Each release's notes state compatibility and migration risks. Dry IR follows the compatibility policy in
[`10-dry-ir-v0-spec.md`](10-dry-ir-v0-spec.md) §8; the profile/report contracts follow
[`11-profiles-and-reports.md`](11-profiles-and-reports.md). Behavioral changes (e.g. a verification rule's
default severity) are called out in the changelog and warrant at least a minor bump.

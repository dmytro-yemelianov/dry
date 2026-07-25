# Releasing Dry

Dry ships three installable front-ends from one tagged release: the **CLI** (native binaries), the
**Python** package (wheels via maturin), and the **TypeScript SDK** (npm package over the wasm engine).
A release is produced entirely by CI from a `vX.Y.Z` tag — `.github/workflows/release.yml`.

## Cutting a release

1. **Bump versions** in lockstep (they must match the tag):
   - `Cargo.toml` → `[workspace.package] version`
   - `py/pyproject.toml` → `[project] version`
   - `sdk/ts/package.json` → `version`
   Verify locally: `scripts/check-version.sh vX.Y.Z`.
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
   - **quality** reruns format/lint, all-feature Rust tests, conformance validators and dependency
     audits on the tagged commit, so publication does not depend on an earlier workflow run;
   - **cli** reproducibly builds locked binaries (including Moonraker upload) for Linux x86_64, macOS
     aarch64 + x86_64, and Windows x86_64, then smoke-tests runnable targets;
   - **wheels** / **sdist** build locked Python artifacts via maturin (manylinux + macOS + Windows), and
     each wheel is installed and imported before packaging succeeds;
   - **ts** audits dependencies, builds, tests and packs the npm tarball;
   - **release** creates the GitHub Release, attaches every artifact plus a `SHA256SUMS` checksum file,
     and auto-generates notes.

## Distribution boundary

Artifacts are distributed only through this repository's **private GitHub Releases**. The workflow
intentionally contains no npm or PyPI publishing jobs, credentials, or trusted-publisher setup because
the release artifacts contain proprietary IP. Do not add public registry publishing without a separate
security review and explicit authorization from the repository owner.

Python wheels, the source distribution and the npm tarball are release assets, not public registry
packages. Customers and internal users install them from an authenticated private release.

Product documentation is a separate **public** distribution surface. Its public build substitutes
non-executable examples and excludes the SDK, browser/WASM engine, gallery, packages, and release
downloads. Never deploy the internal `build:product` output to public hosting. The build and verification
requirements are in [`18-cloudflare-publishing.md`](18-cloudflare-publishing.md).

## Installing private release artifacts

- **CLI** — download the `dry-<ver>-<target>.tar.gz` for your platform from the private GitHub Release, verify it
  against `SHA256SUMS`, extract, and run `./dry --help`.
- **Python** — download the matching wheel from the private release and run `pip install <wheel>`.
- **TypeScript** — download the npm tarball from the private release and run `npm install <tarball>`.

## Compatibility & migration

Each release's notes state compatibility and migration risks. Dry IR follows the compatibility policy in
[`10-dry-ir-v0-spec.md`](10-dry-ir-v0-spec.md) §8; the profile/report contracts follow
[`11-profiles-and-reports.md`](11-profiles-and-reports.md). Behavioral changes (e.g. a verification rule's
default severity) are called out in the changelog and warrant at least a minor bump.

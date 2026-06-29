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
   - **guard** fails fast unless the tag matches every manifest version;
   - **cli** builds binaries for Linux x86_64, macOS aarch64 + x86_64, and Windows x86_64;
   - **wheels** / **sdist** build Python artifacts via maturin (manylinux + macOS + Windows);
   - **ts** builds and packs the npm tarball;
   - **release** creates the GitHub Release, attaches every artifact plus a `SHA256SUMS` checksum file,
     and auto-generates notes.

## Publishing to registries (optional)

Artifacts are always attached to the GitHub Release. Registry publishing is **gated on secrets**, so it
activates only once you add them:

- **PyPI** — add `PYPI_API_TOKEN`; the `pypi` job then publishes the wheels + sdist.
- **npm** — add `NPM_TOKEN`; the `ts` job then runs `npm publish --access public` for `@dry/sdk`.

## Installing without building from source

- **CLI** — download the `dry-<ver>-<target>.tar.gz` for your platform from the GitHub Release, verify it
  against `SHA256SUMS`, extract, and run `./dry --help`.
- **Python** — `pip install dry` (once published to PyPI), or `pip install <wheel>` from the release
  assets.
- **TypeScript** — `npm install @dry/sdk` (once published to npm), or `npm install <tarball>` from the
  release assets.

## Compatibility & migration

Each release's notes state compatibility and migration risks. Dry IR follows the compatibility policy in
[`10-dry-ir-v0-spec.md`](10-dry-ir-v0-spec.md) §8; the profile/report contracts follow
[`11-profiles-and-reports.md`](11-profiles-and-reports.md). Behavioral changes (e.g. a verification rule's
default severity) are called out in the changelog and warrant at least a minor bump.

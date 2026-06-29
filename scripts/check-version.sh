#!/usr/bin/env bash
# Assert that a release tag (vX.Y.Z) matches the version declared in every published manifest:
# the Cargo workspace, the Python package (py/pyproject.toml), and the TS SDK (sdk/ts/package.json).
# Used by the release guard job; runnable locally: `scripts/check-version.sh v0.2.0`.
set -euo pipefail

TAG="${1:-${GITHUB_REF_NAME:-}}"
if [ -z "$TAG" ]; then
  echo "usage: check-version.sh <vX.Y.Z>" >&2
  exit 2
fi
VER="${TAG#v}"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cargo_ver="$(awk '/^\[workspace.package\]/{f=1} f&&/^version[[:space:]]*=/{gsub(/[" ]/,"",$3); print $3; exit}' "$ROOT/Cargo.toml")"
py_ver="$(awk '/^\[project\]/{f=1} f&&/^version[[:space:]]*=/{gsub(/[" ]/,"",$3); print $3; exit}' "$ROOT/py/pyproject.toml")"
ts_ver="$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$ROOT/sdk/ts/package.json" | head -1)"

status=0
check() {
  local name="$1" got="$2"
  if [ "$got" != "$VER" ]; then
    echo "MISMATCH: $name is '$got' but the tag is '$TAG' (expected '$VER')" >&2
    status=1
  else
    echo "ok: $name = $got"
  fi
}

check "Cargo workspace" "$cargo_ver"
check "py/pyproject.toml" "$py_ver"
check "sdk/ts/package.json" "$ts_ver"

if [ "$status" -ne 0 ]; then
  echo "release tag does not match all manifest versions; bump versions before tagging (see docs/12-releasing.md)" >&2
  exit 1
fi
echo "all manifest versions match $TAG"

#!/usr/bin/env bash
# Assert that a release tag (vX.Y.Z) matches every shipped manifest plus the version and rolling
# four-year Change Date embedded in the DryMachina BUSL-1.1 parameters.
# Used by the release guard job; runnable locally: `scripts/check-version.sh v0.2.0`.
set -euo pipefail

TAG="${1:-${GITHUB_REF_NAME:-}}"
if [ -z "$TAG" ]; then
  echo "usage: check-version.sh <vX.Y.Z>" >&2
  exit 2
fi
if [[ ! "$TAG" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "invalid release tag '$TAG'; expected vMAJOR.MINOR.PATCH[-PRERELEASE]" >&2
  exit 2
fi
VER="${TAG#v}"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

json_version() {
  sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$1" | head -1
}

cargo_ver="$(awk '/^\[workspace.package\]/{f=1} f&&/^version[[:space:]]*=/{gsub(/[" ]/,"",$3); print $3; exit}' "$ROOT/Cargo.toml")"
py_ver="$(awk '/^\[project\]/{f=1} f&&/^version[[:space:]]*=/{gsub(/[" ]/,"",$3); print $3; exit}' "$ROOT/py/pyproject.toml")"
py_cargo_ver="$(awk '/^\[package\]/{f=1} f&&/^version[[:space:]]*=/{gsub(/[" ]/,"",$3); print $3; exit}' "$ROOT/py/Cargo.toml")"
wasm_cargo_ver="$(awk '/^\[package\]/{f=1} f&&/^version[[:space:]]*=/{gsub(/[" ]/,"",$3); print $3; exit}' "$ROOT/crates/wasm/Cargo.toml")"
cloud_cargo_ver="$(awk '/^\[package\]/{f=1} f&&/^version[[:space:]]*=/{gsub(/[" ]/,"",$3); print $3; exit}' "$ROOT/crates/cloud/Cargo.toml")"
verify_runner_cargo_ver="$(awk '/^\[package\]/{f=1} f&&/^version[[:space:]]*=/{gsub(/[" ]/,"",$3); print $3; exit}' "$ROOT/containers/verify-runner/Cargo.toml")"
ts_ver="$(json_version "$ROOT/sdk/ts/package.json")"
ts_lock_ver="$(json_version "$ROOT/sdk/ts/package-lock.json")"
mcp_ver="$(json_version "$ROOT/sdk/mcp/package.json")"
mcp_lock_ver="$(json_version "$ROOT/sdk/mcp/package-lock.json")"
web_ver="$(json_version "$ROOT/web/package.json")"
web_lock_ver="$(json_version "$ROOT/web/package-lock.json")"
service_ver="$(json_version "$ROOT/services/cloud/package.json")"
service_lock_ver="$(json_version "$ROOT/services/cloud/package-lock.json")"
issuer_ver="$(json_version "$ROOT/tools/license-issuer/package.json")"
issuer_lock_ver="$(json_version "$ROOT/tools/license-issuer/package-lock.json")"
deploy_ver="$(json_version "$ROOT/deploy/cloudflare/package.json")"
deploy_lock_ver="$(json_version "$ROOT/deploy/cloudflare/package-lock.json")"
license_ver="$(sed -n 's/^Licensed Work: DryMachina version \([^,]*\),.*/\1/p' "$ROOT/LICENSE" | head -1)"
license_change_date="$(sed -n 's/^Change Date: //p' "$ROOT/LICENSE" | head -1)"

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
check "py/Cargo.toml" "$py_cargo_ver"
check "crates/wasm/Cargo.toml" "$wasm_cargo_ver"
check "crates/cloud/Cargo.toml" "$cloud_cargo_ver"
check "containers/verify-runner/Cargo.toml" "$verify_runner_cargo_ver"
check "sdk/ts/package.json" "$ts_ver"
check "sdk/ts/package-lock.json" "$ts_lock_ver"
check "sdk/mcp/package.json" "$mcp_ver"
check "sdk/mcp/package-lock.json" "$mcp_lock_ver"
check "web/package.json" "$web_ver"
check "web/package-lock.json" "$web_lock_ver"
check "services/cloud/package.json" "$service_ver"
check "services/cloud/package-lock.json" "$service_lock_ver"
check "tools/license-issuer/package.json" "$issuer_ver"
check "tools/license-issuer/package-lock.json" "$issuer_lock_ver"
check "deploy/cloudflare/package.json" "$deploy_ver"
check "deploy/cloudflare/package-lock.json" "$deploy_lock_ver"
check "LICENSE Licensed Work" "$license_ver"

if ! grep -Fq "## [$VER]" "$ROOT/CHANGELOG.md"; then
  echo "MISMATCH: CHANGELOG.md has no release heading '## [$VER]'" >&2
  status=1
else
  echo "ok: CHANGELOG.md contains [$VER]"
fi

release_date="$(sed -n "s/^## \[$VER\] - //p" "$ROOT/CHANGELOG.md" | head -1)"
if [ -z "$release_date" ]; then
  echo "MISMATCH: CHANGELOG.md has no dated release heading for '$VER'" >&2
  status=1
else
  expected_change_date="$(python3 - "$release_date" <<'PY'
import datetime
import sys

released = datetime.date.fromisoformat(sys.argv[1])
print(released.replace(year=released.year + 4).isoformat())
PY
)"
  if [ "$license_change_date" != "$expected_change_date" ]; then
    echo "MISMATCH: LICENSE Change Date is '$license_change_date' but $VER released on $release_date; expected '$expected_change_date'" >&2
    status=1
  else
    echo "ok: LICENSE Change Date = $license_change_date (fourth anniversary)"
  fi
fi

if [ "$status" -ne 0 ]; then
  echo "release tag does not match all manifest versions; bump versions before tagging (see docs/12-releasing.md)" >&2
  exit 1
fi
echo "all manifest versions match $TAG"

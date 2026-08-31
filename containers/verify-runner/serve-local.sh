#!/usr/bin/env bash
# Runs the verify runner locally against a stub profile registry, so `POST /verify` can be exercised
# without a deployed registry and without a Cloudflare account.
#
# This works because of the deliberate escape hatch in `validate_registry_url`: the registry base URL
# must be `https://` EXCEPT when the host is `127.0.0.1` or `localhost`, where plain `http://` is
# accepted too. A throwaway stub therefore needs no TLS. Every other host still must be https, so the
# production SSRF rule is not weakened to get this.
#
# The stub registry is just static files: the runner fetches
# `{registry}/v1/profiles/{pack}/{version}/{profile}` and expects the resolved profile JSON there. All
# six profiles from `conformance/profile-matrix` are served under pack `dry-matrix`, version `1.0.0`.
#
# Ctrl-C stops both processes. The runner's port is fixed at 8080 (`main.rs` binds it literally, and
# the Dockerfile healthcheck and the Worker's `defaultPort` both assume it); the stub registry's port
# is overridable with REGISTRY_PORT.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

RUNNER_PORT=8080 # not configurable: see the note above
REGISTRY_PORT="${REGISTRY_PORT:-8099}"
REGISTRY_DIR="$(mktemp -d "${TMPDIR:-/tmp}/dry-stub-registry.XXXXXX")"
REGISTRY_PID=""

cleanup() {
  [ -n "$REGISTRY_PID" ] && kill "$REGISTRY_PID" 2>/dev/null || true
  rm -rf "$REGISTRY_DIR"
}
trap cleanup INT TERM EXIT

for port in "$RUNNER_PORT" "$REGISTRY_PORT"; do
  if lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
    echo "port $port is already in use; stop that process (or set REGISTRY_PORT for $REGISTRY_PORT)" >&2
    exit 1
  fi
done

PROFILE_DIR="$REGISTRY_DIR/v1/profiles/dry-matrix/1.0.0"
mkdir -p "$PROFILE_DIR"
profiles=()
for dir in conformance/profile-matrix/*/; do
  [ -f "$dir/profile.json" ] || continue
  name="$(basename "$dir")"
  cp "$dir/profile.json" "$PROFILE_DIR/$name"
  profiles+=("$name")
done
if [ "${#profiles[@]}" -eq 0 ]; then
  echo "no profiles found under conformance/profile-matrix" >&2
  exit 1
fi

echo "== building the runner =="
cargo build --release --manifest-path containers/verify-runner/Cargo.toml

# `--directory` rather than a `(cd … && …)` subshell on purpose: with the subshell, `$!` is the
# subshell's pid, so `cleanup` killed *it* and left the http.server orphaned still holding the port —
# and the next run of this script then failed its own port check.
python3 -m http.server "$REGISTRY_PORT" --bind 127.0.0.1 --directory "$REGISTRY_DIR" >/dev/null 2>&1 &
REGISTRY_PID=$!

cat <<EOF

== ready ==
runner    http://127.0.0.1:$RUNNER_PORT      /healthz  /verify  /metrics
registry  http://127.0.0.1:$REGISTRY_PORT    ${#profiles[@]} profiles: ${profiles[*]}

  curl -X POST --data-binary @examples/part.gcode \\
    'http://127.0.0.1:$RUNNER_PORT/verify?pack=dry-matrix&version=1.0.0&profile=${profiles[0]}&registry=http://127.0.0.1:$REGISTRY_PORT'

The report is byte-identical to the CLI for the same input:

  dry import-gcode examples/part.gcode --profile conformance/profile-matrix/${profiles[0]}/profile.json -o /tmp/ir.json
  dry verify /tmp/ir.json --profile conformance/profile-matrix/${profiles[0]}/profile.json --json

...with one caveat worth knowing before you diff them: the CLI stamps its licence mode into the
report and this runner has no licence configured, so run the CLI unlicensed (e.g. HOME pointed at an
empty directory) or the two will differ in the licence block alone.

EOF

ALLOWED_REGISTRY_HOST=127.0.0.1 \
RUST_LOG="${RUST_LOG:-verify_runner=info,tower_http=info}" \
  ./containers/verify-runner/target/release/dry-verify-runner

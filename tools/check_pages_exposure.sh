#!/usr/bin/env bash
# Report which stored deployments of a Cloudflare Pages project are publicly reachable.
#
# Replacing a Pages project's production deployment does NOT withdraw the ones behind
# it: every deployment keeps its own permanent <hash>.<project>.pages.dev URL, and
# each still serves the full site it was built from, including any Pages Functions.
# Taking a site "offline" by deploying a maintenance page therefore leaves every
# previous build — and its live API endpoints — on the public internet.
#
# This script enumerates those URLs and probes each one. A deployment counts as
# CLOSED only if it redirects to a Cloudflare Access login or answers 403; anything
# that returns a page is reported OPEN, with the status of an extra path so an
# exposed API surface is visible rather than implied.
#
#   tools/check_pages_exposure.sh                      # check drymachina
#   tools/check_pages_exposure.sh dry-public-docs      # check another project
#   tools/check_pages_exposure.sh drymachina --path /api/machines
#
# Requires `wrangler` authenticated to the account owning the project (npx is used
# if wrangler is not on PATH). Read-only: it never deploys, deletes, or reconfigures.
#
# Note that a 200 does not by itself prove a real file was served — a project whose
# bundle has no 404.html falls back to index.html with a 200. That fallback is still
# a publicly reachable page, which is what this script measures.
#
# Exit: 0 no deployment publicly reachable, 1 one or more reachable, 2 usage or
# environment error.
set -uo pipefail

PROJECT="drymachina"
PROBE="/api/mcp"
args=()
while [ $# -gt 0 ]; do
  case "$1" in
    --path) PROBE="${2:-}"; [ -z "$PROBE" ] && { echo "--path needs a value" >&2; exit 2; }; shift 2;;
    -h|--help) sed -n '2,27p' "$0" | sed 's/^# \{0,1\}//'; exit 0;;
    -*) echo "unknown option: $1" >&2; exit 2;;
    *) args+=("$1"); shift;;
  esac
done
[ "${#args[@]}" -gt 0 ] && PROJECT="${args[0]}"

if command -v wrangler >/dev/null 2>&1; then WRANGLER=(wrangler); else WRANGLER=(npx --yes wrangler); fi

listing="$("${WRANGLER[@]}" pages deployment list --project-name "$PROJECT" 2>/dev/null)"
if [ -z "$listing" ]; then
  echo "could not list deployments for '$PROJECT' — is wrangler authenticated to the right account?" >&2
  exit 2
fi

urls="$(printf '%s\n' "$listing" | grep -oE "https://[a-z0-9]+\.${PROJECT}\.pages\.dev" | sort -u)"
if [ -z "$urls" ]; then
  echo "no deployment URLs found for '$PROJECT'" >&2
  exit 2
fi

total=0; open=0; closed=0
while IFS= read -r u; do
  [ -z "$u" ] && continue
  total=$((total + 1))
  headers="$(curl -sSI "$u/" --max-time 20 2>/dev/null)"
  code="$(printf '%s' "$headers" | awk 'toupper($1) ~ /^HTTP/ {print $2}' | tail -1)"
  loc="$(printf '%s' "$headers" | grep -i '^location:' | tr -d '\r' | head -1)"
  case "$loc" in
    *cloudflareaccess.com*)
      closed=$((closed + 1)); printf 'CLOSED  %s  (%s -> Access login)\n' "$u" "${code:-?}"; continue;;
  esac
  if [ "${code:-}" = "403" ]; then
    closed=$((closed + 1)); printf 'CLOSED  %s  (403)\n' "$u"; continue
  fi
  probe="$(curl -sS -o /dev/null -w '%{http_code}' "$u$PROBE" --max-time 20 2>/dev/null)"
  open=$((open + 1))
  printf 'OPEN    %s  /=%s  %s=%s\n' "$u" "${code:-?}" "$PROBE" "${probe:-?}"
done <<< "$urls"

echo '-----'
printf '%s: %d deployments — %d closed, %d publicly reachable\n' "$PROJECT" "$total" "$closed" "$open"
if [ "$open" -eq 0 ]; then
  echo "no deployment of '$PROJECT' is publicly reachable"
  exit 0
fi
cat <<MSG

$open deployment(s) still serve this project to anyone holding the URL. To close them,
either delete the deployments (this also removes them as rollback targets) or put a
Cloudflare Access policy over '*.$PROJECT.pages.dev', which closes the hash URLs while
leaving '$PROJECT.pages.dev' itself public. See docs/18-cloudflare-publishing.md.
MSG
exit 1

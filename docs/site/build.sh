#!/usr/bin/env bash
# Build the live-docs site: (1) build the web-target wasm engine, (2) copy it into the site's public
# assets so the browser can load it at /pkg/, (3) build the VitePress site. Pass "wasm-only" to stop
# after the copy (used by `npm run wasm` before dev/test/smoke).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"

bash "$ROOT/web/build.sh" web "$ROOT/web/pkg"
mkdir -p "$HERE/public/pkg"
cp "$ROOT/web/pkg/dry_wasm.js" "$ROOT/web/pkg/dry_wasm_bg.wasm" "$HERE/public/pkg/"
echo "copied web wasm -> $HERE/public/pkg/"

[ "${1:-}" = "wasm-only" ] && exit 0

(
  cd "$HERE"
  ./node_modules/.bin/vitepress build
  node scripts/stage-gallery.mjs
  node scripts/check-built-links.mjs
)
echo "built docs site -> $HERE/.vitepress/dist"

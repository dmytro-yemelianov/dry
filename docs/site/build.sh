#!/usr/bin/env bash
# Build either the public documentation boundary (default) or the internal product site.
# Public builds never compile, copy, or bundle Dry's WASM/SDK implementation.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
MODE="${1:-public}"

if [ "$MODE" = "wasm-only" ] || [ "$MODE" = "product" ]; then
  bash "$ROOT/web/build.sh" web "$ROOT/web/pkg"
  mkdir -p "$HERE/public/pkg"
  cp "$ROOT/web/pkg/dry_wasm.js" "$ROOT/web/pkg/dry_wasm_bg.wasm" "$HERE/public/pkg/"
  echo "copied product wasm -> $HERE/public/pkg/"
fi

[ "$MODE" = "wasm-only" ] && exit 0

(
  cd "$HERE"
  if [ "$MODE" = "public" ]; then
    node scripts/stage-public-assets.mjs
    DRY_DOCS_MODE=public ./node_modules/.bin/vitepress build
    node scripts/check-public-boundary.mjs
  elif [ "$MODE" = "product" ]; then
    DRY_DOCS_MODE=product ./node_modules/.bin/vitepress build
    node scripts/stage-gallery.mjs
  else
    echo "unknown docs build mode: $MODE (expected public, product, or wasm-only)" >&2
    exit 2
  fi
  node scripts/check-built-links.mjs
)
echo "built $MODE docs site -> $HERE/.vitepress/dist"

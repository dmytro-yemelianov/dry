#!/usr/bin/env bash
# Build and stage the full Dry Machina site bundle for Cloudflare Pages.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
OUT="$ROOT/dist-site"

rm -rf "$OUT"
mkdir -p "$OUT/web" "$OUT/docs"

# 1. Build the web WASM bundle
bash "$ROOT/web/build.sh" web "$ROOT/web/pkg"

# 2. Copy root portal files
cp "$ROOT/index.html" "$OUT/index.html"
cp "$ROOT/README.md" "$OUT/README.md"
cp -r "$ROOT/docs/"* "$OUT/docs/"

# 3. Copy web application assets
cp -r "$ROOT/web/"* "$OUT/web/"

# 4. Write Cloudflare Headers & Routes configuration
cat << 'EOF' > "$OUT/_headers"
/*
  X-Content-Type-Options: nosniff
  X-Frame-Options: SAMEORIGIN
  Referrer-Policy: strict-origin-when-cross-origin

/*.html
  Cache-Control: no-cache, no-store, must-revalidate

/web/*.html
  Cache-Control: no-cache, no-store, must-revalidate

/web/*.js
  Cache-Control: no-cache, no-store, must-revalidate

/web/pkg/*.wasm
  Content-Type: application/wasm
  Cache-Control: public, max-age=31536000, immutable
EOF

echo "✅ Dry Machina site bundle built into $OUT"

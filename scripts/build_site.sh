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

# 2. Build the React Studio Vite application
npm run build --prefix "$ROOT/web"

# 3. Copy root portal files
cp "$ROOT/index.html" "$OUT/index.html"
cp "$ROOT/README.md" "$OUT/README.md"
cp -r "$ROOT/docs/"* "$OUT/docs/"

# 4. Copy static HTML portals and data
cp "$ROOT/web/machines.html" "$OUT/web/machines.html"
cp "$ROOT/web/machines.json" "$OUT/web/machines.json"
cp "$ROOT/web/docs.html" "$OUT/web/docs.html"
cp "$ROOT/web/auth.html" "$OUT/web/auth.html"
cp "$ROOT/web/legal.html" "$OUT/web/legal.html"
cp "$ROOT/web/privacy.html" "$OUT/web/privacy.html"
cp "$ROOT/web/cleanroom.html" "$OUT/web/cleanroom.html"
cp "$ROOT/web/opportunities.html" "$OUT/web/opportunities.html"
cp "$ROOT/web/architecture.html" "$OUT/web/architecture.html"

# 5. Copy Vite compiled studio (index.html, assets/ with content hashes)
cp -r "$ROOT/web/dist/"* "$OUT/web/"

# 6. Write Cloudflare Headers & Cache Policy
cat << 'EOF' > "$OUT/_headers"
/*
  X-Content-Type-Options: nosniff
  X-Frame-Options: SAMEORIGIN
  Referrer-Policy: strict-origin-when-cross-origin

/*.html
  Cache-Control: no-cache, no-store, must-revalidate

/web/*.html
  Cache-Control: no-cache, no-store, must-revalidate

/web/assets/*
  Cache-Control: public, max-age=31536000, immutable

/web/assets/*.wasm
  Content-Type: application/wasm
  Cache-Control: public, max-age=31536000, immutable
EOF

echo "✅ Dry Machina site bundle built into $OUT"

#!/usr/bin/env bash
# Build and stage the full Dry Machina site bundle for Cloudflare Pages.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
OUT="$ROOT/dist-site"

rm -rf "$OUT"
mkdir -p "$OUT/web" "$OUT/docs" "$OUT/assets"

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
cp "$ROOT/web/api.html" "$OUT/web/api.html"
cp "$ROOT/web/auth.html" "$OUT/web/auth.html"
cp "$ROOT/web/legal.html" "$OUT/web/legal.html"
cp "$ROOT/web/privacy.html" "$OUT/web/privacy.html"
cp "$ROOT/web/cleanroom.html" "$OUT/web/cleanroom.html"
cp "$ROOT/web/opportunities.html" "$OUT/web/opportunities.html"
cp "$ROOT/web/architecture.html" "$OUT/web/architecture.html"

# 5. Copy Vite compiled studio (index.html, assets/ with content hashes)
cp -r "$ROOT/web/dist/"* "$OUT/web/"
# Also mirror assets to root /assets/ for absolute resilience
cp -r "$ROOT/web/dist/assets/"* "$OUT/assets/"

# 6. Write Cloudflare Redirects
cat << 'EOF' > "$OUT/_redirects"
/web    /web/   301
/api    /web/api.html 301
EOF

# 7. Write Cloudflare Headers & Cache Policy with explicit MIME types
cat << 'EOF' > "$OUT/_headers"
/*
  X-Content-Type-Options: nosniff
  X-Frame-Options: SAMEORIGIN
  Referrer-Policy: strict-origin-when-cross-origin

/*.html
  Cache-Control: no-cache, no-store, must-revalidate

/web/*.html
  Cache-Control: no-cache, no-store, must-revalidate

/web/assets/*.css
  Content-Type: text/css; charset=utf-8
  Cache-Control: public, max-age=31536000, immutable

/assets/*.css
  Content-Type: text/css; charset=utf-8
  Cache-Control: public, max-age=31536000, immutable

/web/assets/*.js
  Content-Type: text/javascript; charset=utf-8
  Cache-Control: public, max-age=31536000, immutable

/assets/*.js
  Content-Type: text/javascript; charset=utf-8
  Cache-Control: public, max-age=31536000, immutable

/web/assets/*.wasm
  Content-Type: application/wasm
  Cache-Control: public, max-age=31536000, immutable

/assets/*.wasm
  Content-Type: application/wasm
  Cache-Control: public, max-age=31536000, immutable
EOF

echo "✅ Dry Machina site bundle built into $OUT"

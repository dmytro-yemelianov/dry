#!/usr/bin/env bash
# Build and stage the full Dry Machina site bundle for Cloudflare Pages.
#
# This script stages STATIC assets only. The public API endpoints (/api/verify,
# /api/macros, /api/mcp, /api/machines) are Cloudflare Pages Functions living in
# functions/ at the repo root, and are deliberately NOT copied into dist-site/ --
# Wrangler discovers a functions/ directory relative to the CURRENT WORKING
# DIRECTORY, not relative to the uploaded directory. The deploy must therefore be
# run from the repo root or the site ships with no API endpoints, silently and
# with a successful-looking upload. See docs/18-cloudflare-publishing.md.
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

# Pages Functions are picked up from ./functions relative to the deploy CWD.
# This script cannot know that CWD -- the deploy is a separate command -- so it
# only checks that functions/ exists and prints the correct invocation below.
if [ ! -d "$ROOT/functions" ]; then
  echo "⚠️  $ROOT/functions is missing — a deploy from here would publish NO /api/* endpoints."
fi

cat <<EOM

Deploy from the REPO ROOT ($ROOT) so functions/ is bundled:

  cd "$ROOT"
  npx wrangler pages deploy dist-site --project-name drymachina --branch main

Then verify the endpoints actually shipped (a 200 alone is not proof — dist-site
has no 404.html, so unmatched paths fall back to index.html with a 200):

  curl -s https://drymachina.com/api/machines | head -c 80

EOM

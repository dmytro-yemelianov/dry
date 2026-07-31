#!/usr/bin/env bash
# Build the Dry wasm engine for the browser demo. Produces web/pkg/ (target=web, ES module
# loaded directly by web/index.html). Requires: rustup target wasm32-unknown-unknown, and the
# wasm-bindgen CLI pinned to the crate version (0.2.123).
#
# The node smoke test (CI) builds the same crate with --target nodejs into a scratch dir; see
# .github/workflows/ci.yml. The engine is identical — only the JS glue differs by target.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
TARGET="${1:-web}"          # web | nodejs
OUT="${2:-$HERE/pkg}"

# Materialize the browser-only module from the committed Dry L1 fixtures. The generated file is
# ignored by git and staged into the public docs artifact by an explicit allow list.
node "$HERE/generate-fullcontrol-gallery.mjs"

# dry-wasm is excluded from the core workspace (kept binding-free), so it builds into its own
# target dir under crates/wasm/.
cargo build --release --manifest-path "$ROOT/crates/wasm/Cargo.toml" \
  --target wasm32-unknown-unknown
wasm-bindgen "$ROOT/crates/wasm/target/wasm32-unknown-unknown/release/dry_wasm.wasm" \
  --target "$TARGET" --out-dir "$OUT" --no-typescript

if [ "$TARGET" = "nodejs" ]; then
  # wasm-bindgen's --target nodejs glue is CommonJS. web/package.json declares
  # "type": "module" (needed for the browser TPMS delegation, web/tpms-engine.js), which would
  # otherwise make Node parse this generated glue as ESM whenever $OUT lives under web/. Scope
  # the CommonJS declaration to just the output directory so it wins regardless of $OUT.
  printf '{"type":"commonjs"}\n' > "$OUT/package.json"
fi

echo "built $TARGET -> $OUT"

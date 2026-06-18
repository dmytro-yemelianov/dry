#!/usr/bin/env bash
# Build the TypeScript SDK: compile the Dry wasm engine (nodejs target) into ./wasm, then tsc.
# The SDK is a thin front-end — all toolpath logic lives in the wasm engine (the same Rust core the
# CLI and the Python SDK use). Requires the wasm-bindgen CLI pinned to the crate version (0.2.123).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"

bash "$ROOT/web/build.sh" nodejs "$HERE/wasm"
# mark the wasm dir CommonJS so the .js glue loads regardless of any ancestor package.json "type".
printf '{"type":"commonjs"}\n' > "$HERE/wasm/package.json"
npx tsc -p "$HERE/tsconfig.json"
echo "built @dry/sdk -> $HERE/dist"

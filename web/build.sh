#!/usr/bin/env bash
# Build the Dry wasm engine for the browser demo. Produces web/pkg/ (target=web, ES module
# loaded directly by web/index.html). Requires: rustup target wasm32-unknown-unknown, and the
# wasm-bindgen CLI pinned to the crate version (0.2.123).
#
# The node smoke test (CI) builds the same crate with --target nodejs into a scratch dir; see
# .github/workflows/ci.yml. The engine is identical — only the JS glue differs by target.
set -euo pipefail

# This bundle is a release artifact: it must not inherit ambient compiler instrumentation.
# Two independent channels reach an inner cargo build, and neither belongs in a shipped bundle:
# 1. Flag env vars: `cargo-llvm-cov` exports `RUSTFLAGS="-C instrument-coverage --cfg=coverage"`
#    for the whole process tree it runs (this script is reached that way —
#    `crates/core/tests/wasm_native_math.rs` shells out to it from an instrumented test run).
# 2. The rustc-wrapper channel: cargo-llvm-cov also sets `RUSTC_WRAPPER`/`CARGO_BUILD_RUSTC_WRAPPER`
#    to its own shim, which appends the same flags inside every rustc invocation — unsetting only
#    RUSTFLAGS does not stop it (that was PR #282's one-layer-short first attempt).
# The wasm32-unknown-unknown target ships without the profiler runtime, so `-C
# instrument-coverage` cannot even compile there (`error[E0463]: can't find crate for
# profiler_builtins`) — with the wrapper channel still open, the coverage job died identically.
# Sanitizer or coverage flags have no business in a shipped bundle anyway. CI jobs that genuinely
# need custom flags pass them to their own cargo invocation, not through this script.
unset RUSTFLAGS RUSTDOCFLAGS CARGO_ENCODED_RUSTFLAGS
unset RUSTC_WRAPPER CARGO_BUILD_RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER

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

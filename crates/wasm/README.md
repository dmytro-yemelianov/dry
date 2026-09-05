# `dry-wasm` — WebAssembly Compiler Engine

[![License: BUSL-1.1](https://img.shields.io/badge/License-BUSL--1.1-blue.svg)](../../LICENSE)

`wasm-bindgen` bindings that compile the entire `dry-core` Rust engine to high-performance WebAssembly (`wasm32-unknown-unknown`) for in-browser and Node.js execution.

---

## 1. Features

- **100% Client-Side Execution**: Zero network latency and zero server costs.
- **In-Browser G-Code Verification**: `verify_gcode_to_report_wasm(gcode, contracts_json)`.
- **Parametric CAM Generators**: TPMS lattices, CNC pocketing, and lathe turning directly in the browser.
- **Bitwise Parity**: Produces byte-identical output to native Rust binaries.

---

## 2. Building

```bash
# Build wasm package for Web / Bundlers
wasm-pack build --target web --out-dir pkg

# Build wasm package for Node.js
wasm-pack build --target nodejs --out-dir pkg-node
```

---

## License

Licensed under the **Business Source License 1.1 (BUSL-1.1)**. The DryMachina v0.10.0
terms convert to MIT on 2030-09-05; see [LICENSE](../../LICENSE).

# The Guided Tour

Dry is a toolpath compiler: a design is a program that lowers through a typed IR
(design to path to motion to target) which the engine simulates, verifies, optimizes, and emits.

The public site documents each workflow and ships the browser gallery with the same Rust/WASM engine
used by the CLI and SDKs. Open **Gallery** in the top navigation to edit examples, inspect the 3D
toolpath, simulate playback, verify output, and export G-code or Dry IR.

Reference pages: [TypeScript SDK](../reference/generated/typescript-sdk), [Python SDK](../reference/generated/python-sdk),
[CLI](../reference/generated/cli), [IR](../reference/generated/ir), [examples matrix](../reference/generated/examples).

1. [Author a path](./author) - the fluent `Design` API
2. [Lower to the Dry IR](./lower) - the typed L2 motion segments
3. [Simulate](./simulate) - time, distance, material, peak flow
4. [Verify](./verify) - machine-safety contracts
5. [Optimize](./optimize) - standard vs kinematics-aware balanced
6. [Generative](./generative) - TPMS and lattice generators

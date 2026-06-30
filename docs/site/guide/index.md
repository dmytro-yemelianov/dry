# The Guided Tour

Dry is a toolpath compiler: a design is a program that lowers through a typed IR
(design to path to motion to target) which the engine simulates, verifies, optimizes, and emits.

Every code block below is live. Edit it and the canvas, g-code, IR, metrics, and verify panes
rerun against the same Rust/wasm engine the CLI and the Python/TypeScript SDKs use.

1. [Author a path](./author) - the fluent `Design` API
2. [Lower to the Dry IR](./lower) - the typed L2 motion segments
3. [Simulate](./simulate) - time, distance, material, peak flow
4. [Verify](./verify) - machine-safety contracts
5. [Optimize](./optimize) - standard vs kinematics-aware balanced
6. [Generative](./generative) - TPMS and lattice generators

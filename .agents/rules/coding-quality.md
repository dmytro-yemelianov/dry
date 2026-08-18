# Coding Quality & Invariants: dry

1. **Correctness & Mathematical Invariants**:
   - `crates/core` is the mathematical heart of `dry`. All floating-point operations must be guarded against NaN, infinity, division-by-zero, and out-of-domain trigonometrics.
   - Any modification to core numerics must verify against the contracts in `proofs/` (`claims.toml`, boundary TOMLs) and schemas in `spec/`.
   - Never weaken numerical precision or relax error tolerances to bypass test failures.

2. **Cross-Target Binding Parity**:
   - `crates/wasm`, `crates/cloud`, `py/`, `sdk/ts`, and `containers/verify-runner` expose core DSL functionality but build outside the primary workspace.
   - When core AST, IR, or resolver APIs change, verify that binding surfaces maintain behavioral parity.

3. **Dependency Discipline**:
   - `crates/core` is deliberately lightweight and self-contained. Do not add heavy external crates to `crates/core` without explicit design approval.

4. **Testing Gate**:
   - `cargo test -p dry-core` and `cargo test -p dry-cli` must pass with zero failures and zero warnings.
   - Conformance vectors (`python tools/validate_vectors.py conformance/vectors`) must remain valid.

5. **Sandbox & Agent Protocol**:
   - All code generation is executed in isolated worktree branches (`Workspace: 'branch'`).
   - Reviewers and auditors operate strictly in read-only mode (`enable_write_tools: false`).

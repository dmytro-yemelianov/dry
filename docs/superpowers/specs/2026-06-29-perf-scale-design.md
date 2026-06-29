# Design: Performance & scale gates (Slice C)

**Date:** 2026-06-29
**Status:** Approved (batch directive "take B-E") — tracked in GitHub issues
**Branch:** `feat/perf-scale-gates` (stacked on `feat/release-engineering`)
**Source docs:** `docs/08-production-transition.md` (§WS4), `docs/09-customer-readiness.md` (task #5).

## Goal

Measure scale behavior and lock in the **bounded-memory** claim with a deterministic gate, so a large
print can be simulated/verified/emitted through `DRY1` without unbounded memory growth, and the JSON/`DRY0`
materialization is documented (not mis-sold as streaming).

## Approach

- **Deterministic bounded-memory gate** (the real regression protection): a counting global allocator
  (`crates/core/tests/memory_scale.rs`) measures peak heap above a baseline while streaming a `DRY1`
  archive vs. materializing it, across two sizes. Streaming must stay within ~1.5× as N doubles;
  materialization must grow ≥1.7×. Deterministic → safe on noisy CI runners (runs in `cargo test`).
- **Criterion benchmarks** (`crates/core/benches/engine_codec.rs`) over the codecs and passes for local
  profiling. Wall-clock is **not** a hard CI gate (runner noise); CI only compiles the benches
  (`cargo bench --no-run`) as a bit-rot gate.
- **Honest memory-model doc** (`docs/13-performance-and-scale.md`): only `DRY1` + the `*_stream` passes are
  bounded; `DRY0`/JSON materialize.

## Artifacts

| Path | What |
|---|---|
| `crates/core/tests/memory_scale.rs` | counting-allocator bounded-memory gate |
| `crates/core/benches/engine_codec.rs` | criterion benches (codecs + passes); `criterion` dev-dep + `[[bench]]` |
| `docs/13-performance-and-scale.md` | memory model, benchmarks, regression-gate policy |
| `.github/workflows/ci.yml` | new `bench` job (compile gate) |

## Acceptance → 08·WS4 / 09 #5

- ✅ benchmarks for JSON/`DRY0`/`DRY1`/emit/verify/simulate/trace
- ✅ a large print streams through `DRY1` without unbounded memory (proven by the scale gate)
- ✅ JSON/`DRY0` materialization documented, not presented as bounded streaming
- ✅ regressions fail before release (deterministic gate in CI; bench bit-rot gate)

## Work breakdown (issues)

- Epic: Slice C — Performance & scale gates.
- C1 bounded-memory gate; C2 criterion benches + `bench` CI job; C3 `docs/13` memory-model doc.

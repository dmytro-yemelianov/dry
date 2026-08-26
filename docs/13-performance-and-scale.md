# Performance and scale

This documents Dry's **memory model** (which operations are bounded-memory and which materialize the
whole toolpath), the **benchmarks**, and the **regression gates**. It operationalizes
`docs/08-production-transition.md` §WS4.

## Memory model — streaming vs. materializing

The honest claim Dry makes: **only the `DRY1` chunked archive read through the streaming APIs is
bounded-memory.** JSON and `DRY0` materialize the full toolpath.

| Path | Decode behavior | Working set |
|---|---|---|
| `DRY1` + `decode_any_streaming` → `simulate_stream`/`verify_stream`/`emit_stream` | decodes one compressed **block** (default 512 segments) at a time | **bounded** — independent of segment count |
| `DRY0` (`from_bytes`) | must inflate the **entire** columnar body before yielding segments | O(N) |
| JSON (`from_json`) | parses the full document into `Vec<Segment>` | O(N) |
| Any `*_stream` pass over an in-memory `Toolpath` | the `Vec<Segment>` already holds all N | O(N) (the toolpath itself) |

So "streaming" is a property of the **`DRY1` reader + the `*_stream` passes**, not of JSON or `DRY0`. A
caller who needs bounded memory on a large print must read `DRY1` and use the streaming passes; reading
JSON or `DRY0`, or calling the non-streaming `simulate(&tp)` on a materialized toolpath, is O(N) by
construction. This is intentional and is **not** to be presented as bounded-memory streaming.

Both binary readers reject declared sizes before allocation or DEFLATE expansion. The default
`DecodeLimits` cap total input and segment counts, the full `DRY0` body, each `DRY1` block, metadata,
strings and per-segment control points. `decode_with_limits`, `decode_any_streaming_with_limits` and
`Toolpath::from_bytes_with_limits` let an embedding application choose a smaller or larger explicit
budget. The defaults are a denial-of-service boundary, not a claim that a materialized `DRY0` read uses
constant memory.

### Proof (the scale gate)

`crates/core/tests/memory_scale.rs` installs a counting global allocator and measures the peak heap
**above a baseline** while (a) streaming a `DRY1` archive through `simulate_stream`, and (b) materializing
the same toolpath. Doubling the segment count must keep the streaming working set within ~1.5×, while the
materialized peak grows ≥1.7×. Representative run (10k → 20k segments):

```
stream:      ~0.46 MB -> ~0.46 MB   (flat — bounded)
materialize: ~8.6 MB  -> ~17 MB     (linear in N)
```

The test fails if a streaming path ever starts buffering proportionally to N. It runs in `cargo test`.

## Benchmarks

Criterion benchmarks cover the three codecs and the passes over a 5,000-segment toolpath, split across
the three layer crates the codec/simulate/emit, verify, and trace passes now live in:
`crates/kernel/benches/engine_codec.rs` (`encode_json`, `encode_dry0`, `encode_dry1`, `decode_json`,
`decode_dry0`, `decode_dry1`, `simulate`, `emit`), `crates/verify/benches/verify_pass.rs` (`verify`),
and `crates/trace/benches/trace_pass.rs` (`trace`):

```sh
cargo bench -p kmet-kernel --bench engine_codec
cargo bench -p kmet-verify --bench verify_pass
cargo bench -p kmet-trace --bench trace_pass
# or a quick pass, e.g.:
cargo bench -p kmet-kernel --bench engine_codec -- --measurement-time 1 --sample-size 10
```

Indicative timings (5,000 segments; machine-dependent, for relative comparison only):

| Operation | ~time |
|---|---|
| `encode_json` | ~1.0 ms |
| `encode_dry0` | ~1.6 ms |
| `encode_dry1` | ~3.6 ms |
| `decode_json` | ~1.4 ms |
| `decode_dry0` | ~0.4 ms |

`DRY1` encode is slower than `DRY0` (per-block DEFLATE framing) but is the path that decodes
bounded-memory; `DRY0` columnar decode is the fastest full-materialization read.

## Regression gates

- **Bounded-memory gate (hard, deterministic):** `memory_scale.rs` runs in `cargo test --all` and fails
  on any streaming regression. Deterministic — not wall-clock — so it is safe on shared CI runners.
- **Bench bit-rot gate:** the CI `bench` job runs
  `cargo bench -p kmet-kernel -p kmet-verify -p kmet-trace --no-run`, so all three benchmarks always
  compile against the current API. (`-p dry-core --no-run` would still exit 0 here and gate nothing —
  the facade crate has no bench target of its own.)
- **Wall-clock benchmarks** are for local profiling and trend tracking, not a hard CI pass/fail — shared
  runners are too noisy for reliable absolute thresholds. Compare runs locally with criterion's baseline
  feature (`--save-baseline` / `--baseline`).

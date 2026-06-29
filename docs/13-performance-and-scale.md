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

Criterion benchmarks cover the three codecs and the passes over a 5,000-segment toolpath
(`crates/core/benches/engine_codec.rs`):

```sh
cargo bench -p dry-core --bench engine_codec
# or a quick pass:
cargo bench -p dry-core --bench engine_codec -- --measurement-time 1 --sample-size 10
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
- **Bench bit-rot gate:** the CI `bench` job runs `cargo bench -p dry-core --no-run`, so the benchmarks
  always compile against the current API.
- **Wall-clock benchmarks** are for local profiling and trend tracking, not a hard CI pass/fail — shared
  runners are too noisy for reliable absolute thresholds. Compare runs locally with criterion's baseline
  feature (`--save-baseline` / `--baseline`).

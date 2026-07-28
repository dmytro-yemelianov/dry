# Task 0 Findings — dry-core verify inside a Cloudflare Worker (feasibility spike)

**Date:** 2026-07-28
**Status:** Binding interface for Tasks 1–11 of `docs/superpowers/plans/2026-07-28-dry-cloud-mvp.md`
per that plan's Task-0 framing note. Where a later task's assumption conflicts with this doc, this
doc wins — note the adaptation in that task's report.
**Scaffold:** `crates/cloud/` (`Cargo.toml`, `src/lib.rs`, `wrangler.toml`) — excluded from the root
workspace exactly like `crates/wasm` (see root `Cargo.toml`). Never deployed; every command below ran
against `wrangler dev` (local) only.

## Verdict up front

- **NO-GO at the plan's originally-assumed 50 MB upload cap.** A synthetic 50 MB gcode file reliably
  crashes the worker (wasm trap, reproduced 3/3 runs) before it can even return a timing response. Root
  cause is **memory, not CPU time**: `dry-core`'s gcode import path (the same one the CLI's
  `review-gcode` uses) allocates roughly **43–50× the input's byte size** in peak process memory while
  building the parsed `Toolpath` + source-line map, and Cloudflare's Workers isolate memory ceiling is a
  **fixed, non-configurable 128 MB** (Free and Paid plans alike — see §3). CPU time was never the
  binding constraint; it was comfortably inside budget at every size that didn't crash outright.
- **GO for queue-consumer verify at a much smaller cap.** The `#[event(queue)]` handler compiles and
  links cleanly for wasm32 in this crate (§2), and the same per-invocation CPU/memory limits apply to
  queue consumers as to fetch handlers (cited in §3), so nothing about "it's a queue consumer" changes
  the calculus — the constraint is dry-core's memory profile, independent of trigger type.
- **Recommended MVP upload cap: ~1 MB** (down from the plan's 50 MB — see §4 for the arithmetic). This
  is a product-level decision the owner should explicitly sign off on: a 50× cap reduction changes what
  "verify my print" means for the MVP. Three paths forward, not mutually exclusive:
  1. Ship the MVP with the ~1 MB cap (covers small/simple prints; not most sliced multi-hour files).
  2. Invest in a lower-memory import path before raising the cap (streaming import without the
     source-line map that review mode currently always builds — see §4 note).
  3. Route anything above the Worker-safe cap to a **Container fallback** (Cloudflare Containers: a
     normal Linux container runtime, GB-scale memory, no V8-isolate 128 MB ceiling) running the same
     `dry-core` crate compiled as a native binary instead of wasm32. Shape: size-route at ingest — the
     `POST /v1/jobs/verify` handler checks `Content-Length` and either queues the existing Worker/Queue
     job (≤ cap) or dispatches to a Container job (> cap), sharing the same `dry-core` verify call and
     `Report` JSON shape either way.

---

## 1. Pinned versions

| Component | Version | Notes |
|---|---|---|
| `worker` crate | **0.8.5** | `Cargo.toml`: `worker = { version = "0.8", features = ["queue"] }`; resolved/locked at 0.8.5 |
| `worker-build` | **0.8.5** | Upgraded from the pre-installed 0.8.3 (`cargo install worker-build --version 0.8.5 --force`) to match the `worker` crate's minor version |
| `wasm-bindgen` (transitive) | 0.2.126 | Pulled in by `worker`/`web-sys`; not a direct dependency of `dry-cloud` |
| Rust toolchain | 1.88.0 | `rustc 1.88.0 (6b00bc388 2025-06-23)`; `wasm32-unknown-unknown` target already installed |
| `wrangler` | 4.112.0 | Installed + authenticated; update to 4.114.0 available but not applied (spike ran against the installed version) |
| `dry-core` | 0.4.0 (path dep) | Unmodified — compiles for `wasm32-unknown-unknown` with zero changes |

`crates/cloud/Cargo.lock` is committed alongside the crate, mirroring `crates/wasm/Cargo.lock`
(both are workspace-excluded, single-crate-root Cargo projects with their own lockfiles).

---

## 2. Build pipeline — a real blocker, worked around

`worker-build --release` (the plan's assumed build command) **fails outright** on this exact version
combination:

```
error: failed to generate catch wrappers

Caused by:
    externref table required for catch wrappers
Error: Running the wasm-bindgen CLI
```

Root cause (read from `worker-build` 0.8.5's own source, `src/main.rs`): the default "module target"
build path passes `--experimental-reset-state-function --force-enable-abort-handler` to the
auto-downloaded `wasm-bindgen` CLI (0.2.126), and that combination requires an externref table the
compiled `dry_cloud.wasm` doesn't have. Reproduced with a from-scratch `cargo clean` + explicit
`RUSTFLAGS="-C target-feature=+reference-types"` (no change) — this is a genuine version-compatibility
bug in the toolchain, not a project misconfiguration.

**Workaround (two genuine attempts, second one worked):** `worker-build --release --no-panic-recovery`.
This flag makes `worker-build` skip the new module-target path entirely and use the legacy
bundler-target codegen (`main_legacy::process`), which builds cleanly and produces an equivalent
`build/index.js` entrypoint (verified: the built `build/worker/shim.mjs` correctly exports a
`WorkerEntrypoint` subclass with working `fetch`/`queue` methods — confirmed by curling it, §3/§4).
**This flag is now load-bearing in `crates/cloud/wrangler.toml`'s `[build] command` — Tasks 4–7 must
keep it** (or re-verify the module-target path against whatever `worker`/`worker-build`/`wasm-bindgen`
versions are current when they run; this may be a transient upstream bug that gets fixed).

Trade-off of `--no-panic-recovery`: Rust panics still lower to a wasm trap (`unreachable`) that kills
the whole isolate ungracefully (see the 50 MB crash in §4) rather than being caught and converted to a
clean JS error — the alternative `--panic-unwind` flag needs a nightly toolchain + rebuilt std, out of
scope for this spike. Task 6 should wrap the actual `verify()` call in `std::panic::catch_unwind` inside
the queue consumer (already specified in the plan) — that guards against *panics*, but not against the
kind of wasm-level allocation trap seen here, which happens below the point `catch_unwind` can intercept
in a `panic=abort` build.

---

## 3. Cloudflare limits (cited from current docs, fetched 2026-07-28)

From <https://developers.cloudflare.com/workers/platform/limits/>:

| Limit | Workers Free | Workers Paid |
|---|---|---|
| CPU time per HTTP request | 10 ms | **5 min (default: 30 seconds)**, configurable via `limits.cpu_ms` up to `300000` |
| CPU time per Cron Trigger | 10 ms | 30 s (< 1 hour interval) / 15 min (≥ 1 hour interval) |
| **Memory per isolate** | **128 MB** | **128 MB** — "including the JavaScript heap and WebAssembly allocations. This limit is per-isolate, not per-invocation. A single isolate can handle many concurrent requests." Not configurable on either plan. |
| Duration (wall clock), HTTP request | No limit (client-connection-bound) | No limit |

Request body size (governed by **Cloudflare account plan**, not Workers plan — enforced at the edge
before the Worker even runs, returns `413` if exceeded): Free/Pro 100 MB, Business 200 MB, Enterprise
500 MB (default, negotiable higher). This is *not* the binding constraint for dry-cloud (§4 shows the
isolate memory ceiling bites far earlier).

From <https://developers.cloudflare.com/queues/platform/limits/>:

| Limit | Value |
|---|---|
| Message size | 128 KB (1 KB = 1000 B; ~100 B of that is internal metadata) |
| Maximum consumer batch size | 100 messages |
| Consumer duration (wall clock) | 15 minutes |
| Consumer CPU time | "Configurable to 5 minutes" — "Queue consumer Workers are Worker scripts, and share the same per invocation CPU limits as any Workers do." Same `limits.cpu_ms` knob, same 30 s default. |

The 128 KB message-size ceiling **confirms** (doesn't just permit) the plan's Task 6 design: the queue
message can only ever be `{job_id}`, never the raw gcode — the file has to go through R2. Good — no
change needed there.

From <https://developers.cloudflare.com/r2/platform/limits/>:

| Limit | Value |
|---|---|
| Object size | 5 TiB per object |
| Maximum single-request/single-part upload size | 5 GiB |
| Maximum multipart upload total size | 4.995 TiB, up to 10,000 parts |

At any upload cap under discussion here (1 MB through even the plan's original 50 MB), **a single R2
`put()` is sufficient — multipart is unnecessary complexity for this product at this scale.** Multipart
only starts to matter for uploads in the hundreds-of-MB-to-GB range, nowhere near this MVP.

---

## 4. Measurements

### 4.1 Synthetic fixtures

Base fixture: `examples/sliced-sample.gcode` (real, tiny — 528 B, two `;LAYER:` blocks with a Marlin
header/footer). Generator script repeats the one-layer body with an incrementing `;LAYER:N` index and Z
height (keeps header/footer intact) to reach target sizes:

```
python3 gen_synthetic_gcode.py 1  synth-1mb.gcode    # 1,048,610 B / 66,331 lines
python3 gen_synthetic_gcode.py 10 synth-10mb.gcode   # 10,485,922 B / 655,810 lines
python3 gen_synthetic_gcode.py 50 synth-50mb.gcode   # 52,428,976 B / 3,250,292 lines
```

Sanity check — the CLI reviews all three cleanly before any wasm measurement:

```
$ dry review-gcode synth-1mb.gcode
  segments:  48232 (48230 moves with length)
  verify:    4 finding(s), 0 error(s)   # all 4 are unmodeled-gcode warnings (M140/M190/G28), expected
```

### 4.2 Native baseline (no process/CLI overhead — direct `dry-core` calls, `Instant`-timed)

A standalone `native-bench` binary (in the spike's scratch dir, not committed — a thin `main.rs` making
the exact same `import_gcode_reader_with_map` → `simulate` → `verify` calls as `crates/cloud/src/lib.rs`
and the CLI's `review-gcode`) gives a clean native comparison point, steady-state (median of 3 runs,
first run discarded as allocator/cache warmup):

| Input | parse_ms | verify_ms | total_ms | ms/MB (total) |
|---|---|---|---|---|
| 1 MB | ~17–21 | ~4–5 | ~21–26 | ~23 |
| 10 MB | ~150–156 | ~42–46 | ~188–202 | ~19.5 |
| 50 MB | ~742–758 | ~238–255 | ~996–997 | ~19.9 |

Native throughput is stable at **~20 ms/MB**, linear in input size, as expected for a single-pass
parser.

`cargo build --release -p dry-cli` + `/usr/bin/time -p dry review-gcode <file>` (process-level,
includes CLI startup/I/O) tracks the same shape: 1 MB ≈ 20–30 ms, 10 MB ≈ 210–220 ms, 50 MB ≈
1070–1320 ms.

### 4.3 Worker (wasm) — `wrangler dev`, curl from the host

`wrangler dev --port 8799` (no `--local` flag exists in wrangler 4.112.0 — local is the default mode
now; confirmed via `wrangler dev --help`). `compatibility_date` had to be pinned to **2026-07-21**, one
week behind the actual date — `wrangler dev` refused to start otherwise: *"This Worker requires
compatibility date '2026-07-28', but the newest date supported by this server binary is
'2026-07-21'"* (the installed wrangler 4.112.0 bundles a fixed workerd binary; a newer wrangler would
raise this ceiling).

| Input | worker `parse_ms` | worker `verify_ms` | worker `total_ms`\* | curl wall-clock |
|---|---|---|---|---|
| 1 MB | 51–62 | 7–9 | 1069–1077 | 1.071–1.080 s |
| 10 MB | 260–296 | 53–54 | 1337–1380 | 1.350–1.391 s |
| 50 MB | **crash — see 4.4** | — | — | 1.83–2.48 s (all three attempts returned HTTP 500) |

\*`total_ms` is measured from *before* `req.bytes().await` (see `crates/cloud/src/lib.rs`), so it
legitimately includes body-read time — that's why it's ~1000 ms larger than `parse_ms + verify_ms`. It
tracks curl's wall-clock closely (both dominated by the same body-read cost), which is reassuring:
**no evidence of the "Workers freezes Date() across awaits" effect producing a misleading number
here** — the one await point (the body read) is exactly where both clocks agree elapsed real time.

The dominant ~1000 ms is flat across 1 MB → 10 MB (1075 ms → ~1370 ms, only +~300 ms for +9 MB), i.e.
it is **not driven by dry-core's engine work** (parse_ms+verify_ms is 60–350 ms of that) — it looks like
a fixed `wrangler dev`/Miniflare-local body-relay overhead (Node-based local dev server plumbing), and
is very unlikely to be representative of a real deployed Worker's body-read latency. Flagging as a
**local-dev-only artifact**, not a production number.

**Wasm-vs-native compute multiplier** (parse_ms+verify_ms only, excluding body-read): 1 MB ≈
63 ms/MB vs. native ≈ 23 ms/MB → **~2.7×**; 10 MB ≈ 33 ms/MB vs. native ≈ 19.5 ms/MB → **~1.7×**. The
multiplier shrinks as size grows (fixed JS/wasm-bindgen call overhead amortizes), converging toward
~1.7×. Comfortably inside even the *default* 30 s CPU budget, let alone the 300,000 ms (5 min) paid-plan
maximum — **CPU time was never at risk at any size tested.**

### 4.4 The 50 MB crash — and why it's a memory problem, not a CPU-time problem

All three 50 MB attempts failed identically:

```
[wrangler:info] POST /spike/verify 500 Internal Server Error (1827–2483ms)
✘ [ERROR] Uncaught RuntimeError: unreachable
      at wasm://wasm/...:wasm-function[950]...
✘ [ERROR] Uncaught Error: The Workers runtime canceled this request because it detected that
  your Worker's code had hung and would never generate a response.
```

The "hung" message is workerd's generic fallback when a request's promise never settles — it is
**not** evidence of an actual infinite loop. The real signal is `RuntimeError: unreachable`, which is
what a Rust panic/abort lowers to in a `panic=abort` wasm build (the default; see §2's
`--no-panic-recovery` trade-off) — i.e., **the worker crashed**, most plausibly from `wasm memory.grow`
failing under the isolate's memory ceiling and Rust's allocator calling `handle_alloc_error` → abort.

To confirm the memory hypothesis independently of `wrangler dev`'s exact local memory allowance
(which may not match production's 128 MB precisely), `native-bench` was re-run under
`/usr/bin/time -l` (macOS; reports peak/"maximum resident set size"):

| Input | peak RSS | multiplier (RSS / input bytes) |
|---|---|---|
| 1 MB | 52.0 MB | **49.6×** |
| 10 MB | 468.1 MB | **44.6×** |
| 50 MB | 2,259.0 MB (2.15 GiB) | **43.1×** |

The multiplier is stable (converging toward ~43× as input grows — fit between the 10 MB and 50 MB
points gives an asymptotic slope of **42.7 bytes of memory per byte of input**), i.e. this is a
consistent property of `import_gcode_reader_with_map`'s allocation pattern (the parsed-line vector, the
per-segment `Toolpath` structures, and the source-line map it builds for `ReviewReport` findings — the
same `_with_map` variant the CLI's `review-gcode` uses, which Task 6's byte-identical-report requirement
means the cloud path needs too), not an artifact of file size or of this spike's harness.

**50 MB × ~43× ≈ 2.15 GB of memory demand against a hard 128 MB ceiling explains the crash completely** —
no CPU-time theory was needed. A back-of-envelope CPU-time-only estimate (the kind a paper feasibility
analysis might have produced without running real code) would have said 50 MB is comfortably fine
(≈2–3 s of wasm compute against a 30–300 s budget) — **that estimate would have been wrong**, and only
running the actual worker surfaced the real constraint. This is the central finding of the spike.

The 10 MB case (~470 MB equivalent native RSS) *succeeded* locally, which is worth flagging on its own:
**`wrangler dev`/Miniflare does not appear to enforce the 128 MB isolate ceiling as strictly locally as
production's real V8 isolates do** (10 MB's ~470 MB footprint is already >3.5× over 128 MB, yet it
returned 200 OK locally). **Do not treat "it worked under `wrangler dev`" as proof a given file size is
production-safe** — cross-check against the documented 128 MB ceiling using the native-RSS-multiplier
method used here (or, better, get real telemetry once something is actually deployed).

One more compounding factor for the safety margin: the 128 MB ceiling is **per isolate, not
per-request** — "a single isolate can handle many concurrent requests," so two concurrent verify calls
on a hot isolate share the same 128 MB budget. The cap math below does not additionally discount for
concurrency; it is a single-request ceiling.

---

## 5. Upload cap arithmetic

- Hard ceiling: 128 MiB (isolate memory, both plans, not configurable).
- Reserve ~20 MiB for the wasm module + JS/V8 baseline + wasm-bindgen glue + the raw body being held at
  least twice (once by the JS-side request/stream machinery, once copied into the `Vec<u8>` via
  `req.bytes()`) before parsing even starts.
- Usable budget for the parsed-IR blowup: 128 − 20 = **108 MiB**.
- Using the measured multiplier (43×–50× depending on file size; using the more conservative/larger
  small-file multiplier of ~50× since small caps are exactly the regime under discussion): 108 / 50 ≈
  **2.16 MiB max feasible input, zero safety margin**.
- Required 2× safety margin per the spike's deliverable bar: 2.16 / 2 ≈ **~1.1 MiB**.

**Recommended MVP upload cap: 1 MB.** This is ~50× smaller than the plan's assumed 50 MB — flagging
explicitly for owner sign-off rather than quietly absorbing it, since "50 MB" appears in the plan's
Global Constraints (line 21) and Task 6's interface contract. Task 6 already defers the literal number
to this FINDINGS doc ("cap per FINDINGS"), so implementing at 1 MB does not itself block Task 6 — but
the *product* implication (verify only works for small/simple gcode files, not most real sliced
multi-hour prints, until §4's mitigation paths are pursued) is a decision the owner should make
knowingly.

---

## 6. API deviations from the plan's workers-rs sketches

The plan's Global Constraints (line 24) sketch: `#[event(fetch)]` + `Router`, `#[event(queue)]` +
`MessageBatch<T>`, `ctx.env.d1("DB")`, `env.queue("...")`, `ctx.kv("...")`, `ctx.secret("...")`/
`ctx.var("...")`; `worker-build --release`.

Verified against `worker` 0.8.5 source (`~/.cargo/registry/src/.../worker-0.8.5/src/router.rs`,
`env.rs`):

| Sketch | Actual (0.8.5) | Verdict |
|---|---|---|
| `#[event(fetch)]` + `Router` | Present, unchanged. `Router<'a, D>`, `.post_async(pattern, handler)`, `.run(req, env)`. | Matches. |
| `#[event(queue)]` + `MessageBatch<T>` | Present, gated behind the `queue` cargo feature (`worker = { features = ["queue"] }`) — the plan doesn't mention the feature flag explicitly but Task 4's file list does note it. `MessageBatch<T>::messages() -> Result<Vec<Message<T>>>`, `Message<T>::body() -> &T`, `.ack_all()`. | Matches; feature flag confirmed required. |
| `ctx.env.d1("DB")` | `RouteContext<D>` has its **own** `d1`/`kv`/`secret`/`var`/`bucket`/`durable_object`/`service`/`rate_limiter` methods that just delegate to `self.env.*` — `ctx.d1("DB")` is the idiomatic form; `ctx.env.d1("DB")` also compiles (the `env` field is `pub`). | Cosmetic only — both work. |
| `env.queue("...")` | `Queue` access is on `Env` directly (`env.queue(binding)`), **not** mirrored on `RouteContext` (no `ctx.queue(...)` shorthand exists, unlike `d1`/`kv`/`secret`/`var`). A fetch handler that needs to enqueue a job must go through `ctx.env.queue(...)`. | Minor: note for Task 6's `POST /v1/jobs/verify`, which does need to enqueue. |
| `worker-build --release` | **Fails** on this toolchain (§2) — needs `--no-panic-recovery`. | **Real deviation, load-bearing for Tasks 4–7's `wrangler.toml`.** |
| (not in plan) `wrangler.toml` `main` field | Verified against the actual `cloudflare/workers-rs` GitHub template (`templates/hello-world/wrangler.toml`, fetched 2026-07-28): `main = "build/index.js"` (a small re-export shim `worker-build` generates that points at `build/worker/shim.mjs`), not e.g. `build/worker/shim.mjs` directly. | New info, not a deviation — just wasn't specified anywhere before. |
| (not in plan) `compatibility_date` ceiling | Installed wrangler bundles a fixed workerd; dates beyond its ceiling are rejected outright at `wrangler dev` startup. | Operational note for whoever runs `wrangler dev` next — check the ceiling each time. |
| (not in plan) `[build] command` parse warning | `wrangler dev` printed `Unexpected fields found in top-level field: "command"` even though `[build].command` is the documented/templated syntax. Did not block the build or dev server (worker-build still ran, `wrangler:info Ready on http://localhost:8799` followed). Likely a wrangler-version-specific schema-validation quirk; re-verify against whatever wrangler version Task 4 runs. | Cosmetic (observed, not resolved). |
| `worker::Error` conversions | No `From<GcodeImportError>`/`From<dry_core::verify::...>` impls exist (obviously — `dry-core` has zero cloud awareness by design). Errors are mapped by hand: `Response::error(format!("import failed: {e}"), 422)`. | Expected, not really a "deviation" — just confirms there's no free lunch here for Task 6's error handling. |

---

## 7. Reproduction

```sh
# from crates/cloud/
cargo check --target wasm32-unknown-unknown         # sanity: dry-core + dry-cloud compile for wasm32
worker-build --release --no-panic-recovery          # the only build invocation that works (§2)
wrangler dev --port 8799                            # compatibility_date pinned in wrangler.toml (§4.3)

# from another shell, against the three synthetic fixtures (§4.1):
curl -X POST http://localhost:8799/spike/verify --data-binary @synth-1mb.gcode -w '\n%{time_total}s\n'
curl -X POST http://localhost:8799/spike/verify --data-binary @synth-10mb.gcode -w '\n%{time_total}s\n'
curl -X POST http://localhost:8799/spike/verify --data-binary @synth-50mb.gcode -w '\n%{time_total}s\n'  # crashes (§4.4)
```

The synthetic-gcode generator and the native-bench harness used for §4.2/§4.4 are throwaway spike
tooling (not committed — they exist only to produce the numbers in this doc); regenerate from the
description in §4.1/§4.2 if the measurements ever need to be reproduced or re-run against a newer
toolchain.

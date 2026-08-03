# Trace analytics + batch review — design (P3.5)

**Date:** 2026-08-03 · **Task:** P3.5 (`docs/04-tasks.md`) · **Branch:** `feat/trace-analytics`
**Acceptance (P3.5, remainder):** *Parquet/Arrow export, before/after diffing, layer/raster linkage and
higher-level statistical features.*
**Governing constraints:** `dry-core` is dependency-free; the committed trace goldens under
`conformance/reports/` are byte-identical drift gates (`crates/core/tests/report_goldens.rs`);
`spec/dry-reports-v1.schema.json` is `additionalProperties: false` everywhere.

## 0. Decisions at a glance

1. **Feature set** — time-weighted, phase-split segment statistics (count/time/distance/volume,
   time-weighted mean, and `min`/`p50`/`p95`/`max` of feedrate and flow); exact order statistics over
   per-window peaks; per-layer aggregates with the same field set as a window; a flow-outlier window
   list thresholded at `k ×` the *published* window-peak p50. One pass over segments, two O(W) passes
   over the window vector, no new dependency (§3).
2. **Where it lives** — `TraceSummary` gains one optional field, `analytics: Option<TraceAnalytics>`,
   and its already-declared-but-never-populated `layers` vector finally gets filled — both only when
   the caller asks, via a new `trace_summary_with_analytics(…)`. Existing entry points and every
   committed golden stay byte-identical (§4). CLI: `dry trace-gcode --analytics` (§5).
3. **Batch review** — a new `dry review-batch FILES…` subcommand (not a variadic `review-gcode`),
   emitting a `ReviewBatch` envelope that nests unmodified `ReviewReport`s. Exit `0` all clean, `1` any
   file gates, `2` any file could not be inspected — `2` wins over `1` (§6). Schema: nine additive
   `$defs` across the slice (four of them for the batch), two new goldens and two
   `validate_reports.py` rows; no existing `$def` or golden changes (§7).
4. **Parquet/Arrow — deferred**, explicitly. CSV + JSON are this slice's export boundary, and this
   slice *wires up* the CSV writer that currently has no call sites (§8).
5. **Not in scope** — before/after trace diffing (§9.1, including why it must come after layer
   linkage, not with it), layer *rasters*, streaming analytics, and any new verify rule. The outlier
   flag is an observation, never a gate (§9).

## 1. Problem

P3.5's remainder is four items. Against the code as it stands, three of them are not "extensions" —
they are declared surfaces with nothing behind them:

- **`TraceSummary.layers` is never written.** It is declared (`crates/core/src/trace.rs:27`),
  initialised empty (`trace.rs:116`), exported (`crates/core/src/lib.rs:107`) and has a published
  schema `$def` with seven required properties
  (`spec/dry-reports-v1.schema.json:256-277`) — and no code anywhere assigns to it. All seven committed
  `trace.json` goldens carry `"layers": []`, as does the trace nested in
  `conformance/reports/explain/explain.json`. The "layer linkage" P3.5 lists as remaining is not
  partially done; it is a type and a schema with no producer.
- **`TraceSummary::to_csv` has no call sites at all** (`trace.rs:121`) — not in the CLI, not in a test,
  not in a binding. Its doc comment advertises it as the on-ramp for "tabular analysis / Parquet
  export". This is the same shape as `nonplanar.rs` before P5.1 deleted it (functions whose only
  callers were their own unit tests, and `to_csv` does not even have those). It is either wired up or
  removed; this slice wires it up, because CSV is the honest export boundary (§8).
- **There is no batch runner.** `docs/09-customer-readiness.md` §Dry Review Service names batch review
  as the *first* core job of the product and "batch runner" as the *first* production gap.
  `review-gcode` takes exactly one `file: String` (`crates/cli/src/main.rs:353-354`) and exits 1 when
  `error_count > 0` (`main.rs:1505-1509`). A farm reviewing 500 files today gets 500 processes, 500
  JSON documents and no aggregate — and any unreadable file kills the run, because import failure goes
  through `die` → `exit(2)` (`main.rs:678-681`).

The fourth item, statistical features, is a genuine extension: `TraceSummary` and `TraceWindow` carry
sums and two maxima. A max is the least informative order statistic there is — one 40 mm³/s spike and a
whole print reads as flow-limited. What a reviewer actually asks ("is this file *normally* near the
limit, or once?", "which layer is the bottleneck?") is a percentile and a per-layer roll-up question.

## 2. Non-goals

- **No Parquet, no Arrow, no new dependency anywhere** (§8). CSV/JSON only.
- **No before/after trace diffing** (§9.1) — it needs the layer linkage as its alignment key, so it
  belongs in the slice *after* this one.
- **No layer rasters.** `docs/05-product-directions.md` §Trace storage lists "optional screenshots or
  layer rasters" beside layer linkage; core renders no images and grows no image dependency. The
  "raster" half of P3.5's phrase is out of scope and should be struck from the task line rather than
  left implying work in flight.
- **No new verify rule, no new gate.** A flow-outlier window is an *observation* with no severity, no
  rule id, and no effect on any exit code. Turning "window flow > k × median" into a finding is a
  verify-catalog change (`docs/11` §2, `RuleId`, the rule-coverage assertion in `report_goldens.rs`)
  and needs its own justification — a statistical outlier is not a safety violation.
- **No streaming analytics.** `trace_summary` takes a materialised `&Toolpath` and builds an O(time)
  window vector; per `docs/13-performance-and-scale.md` it is O(N) by construction and is not one of
  the bounded-memory paths. Analytics does not change that class, and must not be presented as
  available on `simulate_stream`/`verify_stream` (§4.4).
- **No change to `ReviewReport`, `Metrics`, `Contracts` or any verify surface.** The batch envelope
  *nests* today's `ReviewReport` unchanged (§6.2) — which is what keeps `crates/cloud` and
  `containers/verify-runner` out of this slice's blast radius (§12).
- **No parallelism.** Sequential file processing; a work-stealing pool would mean a new dependency and
  a nondeterministic finding order (§6.5).

## 3. The statistical feature set

Design rules the set is chosen against, in priority order: (a) exact and deterministic — no sampling,
no approximation, no transcendental, so native/wasm bit-identity follows from the same argument as the
existing metric fold; (b) computable from quantities the trace pass *already* derives; (c) each number
answers a question a print-farm reviewer actually asks; (d) nothing speculative — if no caller can
state what a number is for, it is not in the set.

### 3.1 One percentile definition, used everywhere

```
Given samples (vᵢ, wᵢ) with wᵢ ≥ 0 and W = Σwᵢ > 0, sorted ascending by v with a STABLE sort:
    quantile(p) = the first vᵢ whose cumulative Σ_{j≤i} wⱼ ≥ p·W        for p ∈ (0, 1]
```

This is the nearest-rank (inverse-CDF, lower) definition. Consequences, all deliberate:

- **The result is always a value that actually occurred.** No interpolation, so a threshold quoted from
  it (§3.4) refers to a real observed flow rate, not an average of two.
- **It differs from `forensics`' `median`** (`crates/core/src/forensics.rs:414`), which averages the two
  middle values on an even count. Two functions named "median" with different definitions in one crate
  is a trap, so this slice does not add a second `median` helper: `p50` *is* the median under this
  definition, and the divergence is documented here and in the doc comment rather than hidden.
- **Weights unify the two populations.** Segment statistics use `wᵢ = motion seconds` (time-weighted:
  "for 95% of print time, flow was at or below X" — the question a hotend limit answers). Window
  statistics use `wᵢ = 1` (windows are equal-duration by construction, except a partial last one).
  One code path, one proof obligation.
- **Determinism.** The sort is `sort_by(|a, b| a.value.total_cmp(&b.value))` — std's *stable* sort, no
  new dependency. Stability is load-bearing, not decorative: with an unstable sort, equal-valued
  samples permute, the partial sums inside a tie group differ in their last bits, and a later group's
  `cumulative ≥ p·W` comparison can flip. With a stable sort the whole computation is a deterministic
  function of segment order alone. `total_cmp` also gives non-finite values a defined position instead
  of the partial-order hole `partial_cmp` leaves.
- **Non-finite samples are excluded and counted**, never silently folded into a NaN percentile.
  `trace` runs no verifier, `Contracts` has a `finite` rule that nothing forces here, and
  `conformance/reports/non_finite/` exists precisely because such toolpaths are reachable. Each stats
  block carries `nonfinite_samples: usize`. Existing totals keep today's behaviour unchanged.
- **`W == 0` yields `None`, not zero.** A phase with no motion has no percentiles; reporting `0.0`
  would be indistinguishable from a real stall.

### 3.2 Phase-split segment statistics

```rust
/// Exact order statistics under the nearest-rank definition (see the module docs).
pub struct Percentiles { pub min: f64, pub p50: f64, pub p95: f64, pub max: f64 }

/// Time-weighted statistics over the moving segments of one phase (print or travel).
/// Dwell time is excluded: a dwell has no feedrate and no flow.
pub struct PhaseStats {
    pub segments: usize,          // segments in this phase with motion_s > 0
    pub time_s: f64,
    pub distance_mm: f64,
    pub volume_mm3: f64,
    /// Σ(v·t)/Σt. `None` when `time_s == 0`.
    pub mean_feedrate_mm_min: Option<f64>,
    pub mean_flow_mm3_s: Option<f64>,
    /// Time-weighted percentiles. `None` when no finite-valued sample carries weight.
    pub feedrate_mm_min: Option<Percentiles>,
    pub flow_mm3_s: Option<Percentiles>,
    pub nonfinite_samples: usize,
}
```

Travel-phase *flow* is reported even though it should be identically zero — a non-zero value is exactly
the smell the `travel-extrudes` rule names, and having the trace corroborate a verify rule from an
independent computation is worth four bytes of JSON.

**Cross-check invariant (testable, exact):** when every sample is finite,
`max(print.feedrate.max, travel.feedrate.max) == summary.max_feedrate_mm_min`, and likewise for flow —
because `trace_summary` already updates those maxima only on segments with `motion_s > 0`
(`trace.rs:320-333`), which is precisely this ledger's population. The precondition matters: `f64::max`
ignores NaN, so with a NaN present the summary max and the filtered ledger legitimately disagree.

### 3.3 Window peak order statistics

```rust
/// Order statistics over per-window peaks, unweighted, over windows with duration_s > 0.
pub window_flow_mm3_s: Option<Percentiles>,
pub window_feedrate_mm_min: Option<Percentiles>,
/// How many windows those statistics were computed over — a p50 over three windows is not a
/// trend, and the number that says so is published rather than inferred.
pub windows_considered: usize,
pub segments_considered: usize,
```

Named for what they are: percentiles of *per-window peaks*, not of instantaneous flow. Zero-duration
windows — created by `add_zero_duration_segment` (`trace.rs:224-239`) so a zero-length segment still
gets a segment/source-line anchor — carry no motion and are excluded; including them would drag every
percentile toward zero. `windows_considered`/`segments_considered` exist for the same reason
`VerifyReport` gained `segments_inspected`: a statistic computed over almost nothing must not be
byte-indistinguishable from one computed over a whole print.

### 3.4 Flow-outlier windows

```rust
pub struct WindowOutliers {
    /// The multiplier applied to the reference. Echoed from the options.
    pub k: f64,
    /// `k × window_flow_mm3_s.p50`. `None` when there is no reference (no window with motion).
    pub threshold_mm3_s: Option<f64>,
    /// Ascending indices of windows whose `max_flow_mm3_s` is strictly greater than the threshold.
    pub window_indices: Vec<usize>,
}
```

The reference is the **already-published** `window_flow_mm3_s.p50` — not a separately computed median.
So the threshold is reproducible by any consumer from two numbers in the same document, there is no
second median definition to drift, and the O(W) median pass happens once. Strict `>` so `k = 1` on a
constant-flow file flags nothing.

Flow only, deliberately: flow is the channel with a physical ceiling (hotend melt rate,
`material.max_volumetric_flow_mm3_s`), so an outlier there has a mechanism. Feedrate outliers are
mostly travels, which are supposed to be fast. `window_indices` is bounded by `windows.len()`, which
the document already serialises in full.

### 3.5 Layer linkage and per-layer aggregates

`LayerTraceLinkage` gains the same field set a `TraceWindow` has, minus the time bounds — *a layer is a
window whose boundary is Z instead of time*, so it gets the same fields, units and semantics:

```rust
pub struct LayerTraceLinkage {
    // existing, unchanged and in place (declaration order = JSON key order):
    pub layer_index: usize,        // sequence order, 0-based
    pub z_mm: f64,                 // the layer's extruding Z
    pub segment_start: usize,      // inclusive
    pub segment_end: usize,        // EXCLUSIVE, matching TraceWindow::segment_end
    pub print_time_s: f64,
    pub travel_time_s: f64,
    pub extruded_volume_mm3: f64,
    // appended by this slice, in TraceWindow's own order:
    pub dwell_time_s: f64,
    pub extruding_distance_mm: f64,
    pub travel_distance_mm: f64,
    pub filament_mm: f64,
    pub max_feedrate_mm_min: f64,
    pub max_flow_mm3_s: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_line_start: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_line_end: Option<usize>,
}
```

Appending rather than interleaving keeps the existing fields' relative order, so the JSON key order of
the shared prefix matches what the schema already documents.

Adding required fields here is safe *because* nothing ever produced a value: no golden, no fixture and
no binding contains a `layers` entry, so no committed document becomes schema-invalid (§7).

**Layer assignment rule.** A layer *break* is detected at the first **extruding** segment whose Z
differs from the current layer's Z by more than `layer_z_epsilon_mm` (default `1e-6`, the same epsilon
`forensics` uses to dedup its Z set, `forensics.rs:412`). Z is `end[2].or(start[2])`, matching
`forensics.rs:406`. The new layer's `segment_start` is then walked **back** over the immediately
preceding run of non-extruding segments, i.e. to `last_extruding_index + 1`, so the Z lift and the
approach travel belong to the layer they are entering — which is where the slicer's own annotation puts
them (`examples/sliced-sample.gcode` emits `;LAYER:1` *before* `G1 Z0.4`). The back-walk is one tracked
index, so this is still one pass.

Keying breaks on **extruding** Z specifically is what makes the rule robust: keying on any segment's Z
would let a single Z-hop travel shatter a layer into three. A hop's travels sit between extrusions at
the same Z, so the back-walk only ever absorbs the travels adjacent to a real break.

Two boundary decisions make the linkage a *partition*, which is what makes it testable:

- Layer 0 starts at segment **0**, not at the first extruding move, so a heat-up/prime prologue is
  attributed rather than orphaned.
- The last layer ends at `segment_count`, so a wipe/park epilogue is attributed too.

Therefore: `layers[0].segment_start == 0`, `layers[i].segment_end == layers[i+1].segment_start`,
`layers.last().segment_end == segment_count`, and `Σ layer.print_time_s == summary.print_time_s`. The
sum invariant is asserted to `1e-9` **relative**, not bit-exactly: grouping changes the summation order
and f64 addition is not associative. A toolpath with no extruding segment produces `layers: []` and
`layer_stats: None` — there is no Z to key on, and inventing one layer covering everything would be a
lie about a travel-only file.

**Divergence from forensics, stated rather than reconciled:** `ForensicsReport.layers.layer_count`
counts *distinct* Z levels (sorted + deduped, `forensics.rs:411-412`); trace layers are *passes* in
execution order, so a re-visited Z (an ironing pass, a non-monotonic vase) produces a second entry.
`trace.layers.len() >= forensics.layers.layer_count`, with equality when Z is non-decreasing and each
level is a single contiguous run. The two numbers answer different questions and both keep their names.

**Trace does not recompute layer height.** `forensics` owns `layers.layer_height_mm` (median Z delta,
with an `Estimate` confidence tag). Publishing a second `layer_height_mm` from a different population
(passes, not levels) would put two differently-defined numbers with one name in the same bundle —
`ExplainReports` carries trace *and* forensics side by side (`explain.rs`), so they would appear in one
document. Consumers wanting layer height read the forensics field.

```rust
pub struct LayerStats {
    pub layer_count: usize,
    /// Order statistics over per-layer values (unweighted; one sample per layer).
    pub print_time_s: Percentiles,
    pub extruded_volume_mm3: Percentiles,
    /// Layer with the greatest `print_time_s`; ties resolve to the lowest index.
    pub slowest_layer_index: usize,
}
```

### 3.6 Cost, and what "one pass" means honestly

- **Segments:** one pass. `timing()` (`trace.rs:199-214`) already computes `motion_s` and
  `flow_mm3_s`; the analytics pass consumes those values in the loop that already exists — no second
  traversal, no recomputation.
- **Percentile ledger:** one scratch `Vec` of `{feedrate, flow, seconds, travel}` sized by moving
  segment count (32 bytes/segment), sorted twice (once per metric) and dropped before returning.
  `trace_summary` already requires a materialised `&Toolpath` — at ~200+ bytes per `Segment` the scratch
  is well under a tenth of what the caller is already holding, and it does not change the memory class
  (§4.4). This is the one allocation in the design proportional to N, and it is called out rather than
  buried.
- **Windows:** two extra O(W) passes — one to build the peak ledger and take p50, one to flag
  outliers. The outlier flag is *inherently* second-pass: it references a median of all windows. The
  design says "one pass over segments, two cheap passes over the window vector" and does not claim
  single-pass over both.
- **Layers:** accumulated in the same segment loop; the layer vector is O(layer count).

## 4. Where it lives

### 4.1 The core surface

```rust
// crates/core/src/trace.rs
pub struct TraceSummary {
    …                                   // every existing field, unchanged and in place
    pub windows: Vec<TraceWindow>,
    pub layers: Vec<LayerTraceLinkage>, // now actually populated — when analytics are requested
    /// Higher-level statistics, present only when the caller asked for them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analytics: Option<TraceAnalytics>,
}

pub struct TraceAnalytics {
    pub print: PhaseStats,
    pub travel: PhaseStats,
    pub window_flow_mm3_s: Option<Percentiles>,
    pub window_feedrate_mm_min: Option<Percentiles>,
    pub flow_outliers: WindowOutliers,
    pub layer_stats: Option<LayerStats>,
    /// travel motion time / total motion time, in [0,1]. `None` when nothing moved.
    pub travel_time_ratio: Option<f64>,
    pub windows_considered: usize,
    pub segments_considered: usize,
}

pub struct TraceAnalyticsOptions {
    /// Multiple of the window-peak p50 above which a window is flagged. Default 2.0.
    pub flow_outlier_k: f64,
    /// Z tolerance (mm) for "same layer". Default 1e-6.
    pub layer_z_epsilon_mm: f64,
}
impl Default for TraceAnalyticsOptions { … }

/// Summarize a toolpath into fixed-duration windows, also computing layer linkage and analytics.
pub fn trace_summary_with_analytics(
    tp: &Toolpath,
    window_s: f64,
    source_lines: &[Option<usize>],
    options: &TraceAnalyticsOptions,
) -> Result<TraceSummary, TraceError>;
```

**Extend `TraceSummary`, do not add a parallel struct.** A sibling `TraceStatistics` would mean two
documents to correlate, a second report kind in the schema and validator, a second CLI output, and a
`TraceReport` that carries one but not the other — while every number here is *about* the windows and
layers already in `TraceSummary`, and `flow_outliers.window_indices` is meaningless without them. One
document, one optional field.

`trace_summary` and `trace_summary_with_sources` keep their exact signatures and behaviour; all three
delegate to a private core taking `Option<&TraceAnalyticsOptions>`. `TraceError` is reused for the two
new validations (§10) rather than growing a second error type.

### 4.2 Golden byte-identity (a requirement, and how it is met)

`crates/core/tests/report_goldens.rs` writes goldens with `UPDATE_REPORTS=1` and otherwise asserts the
committed bytes; seven `trace.json` goldens plus the nested trace in `explain.json` are in force.

- `analytics: None` on every existing path + `skip_serializing_if = "Option::is_none"` ⇒ the key is
  absent, not `null`.
- `layers` stays `[]` on every existing path. This is the one mildly awkward consequence of the
  requirement: `layers` is a required, always-serialised field that remains empty unless analytics are
  requested, even though nothing about layer linkage is expensive. The alternative — populate it
  unconditionally — rewrites every trace golden and the explain bundle, i.e. exactly the byte drift the
  brief forbids. Chosen: one switch turns on layers *and* analytics together; `docs/11` §3.3 states
  that `layers` is empty without it, so the emptiness is documented behaviour rather than a bug
  report waiting to happen.
- Adding fields to `LayerTraceLinkage` cannot drift anything, because no golden holds an instance.
- `TraceWindow` is **not** modified. An early sketch put a `flow_outlier: bool` on each window; that
  needs an `is_false` skip helper, spreads one statistic across W objects, and edits the most
  widely-validated `$def` in the schema. The outlier list lives in `WindowOutliers` instead.

New golden coverage is *added* rather than existing goldens changed (§7).

### 4.3 CSV: two relations, stable columns

`to_csv()` keeps its current header and column set byte-for-byte (it is now reachable, §5, so its
output becomes a contract). A second method covers the second grain:

```rust
impl TraceSummary {
    pub fn to_csv(&self) -> String;         // one row per window — unchanged
    pub fn layers_to_csv(&self) -> String;  // one row per layer
}
```

Analytics are *aggregate* and stay JSON-only: appending an aggregate to a per-window table would either
denormalise it across every row or make the column set vary with a flag, and a tabular consumer's first
requirement is a stable schema. Two relations at two grains is also exactly what a future Parquet
export writes as two tables (§8).

### 4.4 Streaming boundary

`docs/13-performance-and-scale.md` is explicit that only `DRY1` + the `*_stream` passes are
bounded-memory, and that any pass over a materialised `Toolpath` is O(N). Trace is in the second class
already; analytics adds an O(N) scratch buffer inside it. Neither `docs/13`'s table nor its
bounded-memory claim changes, and no analytics API is added to the streaming passes. Percentiles over a
stream would need either a full ledger (unbounded) or a sketch (approximate, and a new dependency) —
both out of scope, and the doc should not hint otherwise.

## 5. CLI — `trace-gcode`

```
dry trace-gcode FILE [--profile P] [--filament-diameter D] [--line-width W] [--layer-height H]
                     [--window-s 5] [--analytics] [--flow-outlier-k 2.0]
                     [--format json|csv|layers-csv]
```

- Default invocation is **byte-identical to today**: `--format json`, no analytics, same
  `TraceReport` pretty-printed to stdout, exit 0.
- `--analytics` runs `trace_summary_with_analytics`, so the JSON gains `trace.layers` and
  `trace.analytics`.
- `--flow-outlier-k` sets `TraceAnalyticsOptions::flow_outlier_k`; supplying it without `--analytics`
  is a usage error (exit 2) rather than a silently ignored flag. `layer_z_epsilon_mm` gets no flag —
  no caller has a reason to move it, and it is echoed nowhere, so it stays a library option.
- `--format csv` prints `to_csv()`; `--format layers-csv` prints `layers_to_csv()` and **implies**
  `--analytics`, since the analytics pass is the only producer of rows. That implication is the one
  place a format flag changes what is computed, and it beats printing a bare header.
- No `--out`: shell redirection covers it, and `trace-gcode` has never had one.
- Exit code stays 0 on success regardless of outliers. Trace is descriptive; gates are `verify` /
  `review-gcode` / `review-batch`.

## 6. Batch review — `dry review-batch`

### 6.1 A new subcommand, not a variadic `review-gcode`

Making `review-gcode`'s positional variadic forces one of three bad outcomes: the `--json` shape varies
with argument count (hostile to scripts, and unvalidatable by `validate_reports.py`, which maps one
filename to one `$def`); or single-file `--json` starts emitting a batch envelope, breaking the
published `ReviewReport` contract (`docs/11` §3.2) and its golden; or the batch aggregate is dropped,
which is the entire point. A separate command also keeps batch-specific exit semantics (§6.4) out of a
command whose exit rule is documented and depended on.

The name fits the CLI as it is: `compare FILE_A FILE_B` and `explain FILE` both take g-code without a
`-gcode` suffix (`main.rs:449, 499`), so the suffix is a convention of the *import-shaped* commands,
not a rule.

```
dry review-batch [FILES]... [--files-from FILE|-] [--profile P]
                 [--filament-diameter D] [--line-width W] [--layer-height H]
                 [--json] [--out FILE]
```

**Deliberately no per-flag contract overrides.** `review-gcode` carries twelve of them
(`ContractOverrides`, `main.rs:2445-2458`); a fleet gates
against a *profile*, which is what profiles are for and what `docs/09` lists as the adjacent gap
("profile fleet management"). One-off limit tweaks stay `review-gcode`'s job. If a pilot needs them,
they arrive as a shared `#[command(flatten)]` args struct — which would reorder `review-gcode`'s
`--help` and therefore churn `docs/site/reference/generated`, so it is a separate change, not a
drive-by (§13).

`--files-from` reads newline-separated paths (`-` = stdin), because a 50k-file farm exceeds `ARG_MAX`
and the aggregate is exactly what stops you from using `xargs`. Paths are processed in order: positional
first, then `--files-from`. No dedup — a path listed twice is reviewed twice and appears twice; the
output order is the input order, always.

### 6.2 Envelope

Built in `crates/core/src/report.rs` (pure, golden-testable, reusable by the bindings); the CLI does
the I/O and hands core the per-file outcomes.

```rust
pub enum BatchStatus { Passed, Failed, Errored }   // #[serde(rename_all = "lowercase")]

pub struct BatchFileResult {
    pub file: String,
    pub status: BatchStatus,
    /// Present iff the file was inspected.
    #[serde(skip_serializing_if = "Option::is_none")] pub review: Option<ReviewReport>,
    /// Present iff it was not. Exactly one of the two is `Some`.
    #[serde(skip_serializing_if = "Option::is_none")] pub error: Option<String>,
}

/// Per-rule roll-up across the batch, ascending by rule id.
pub struct RuleTally { pub rule: String, pub errors: usize, pub warnings: usize, pub files: usize }

pub struct ReviewBatch {
    pub files_total: usize,
    pub files_passed: usize,
    pub files_failed: usize,
    pub files_errored: usize,
    /// Profile label, once for the batch (every file is reviewed against the same one).
    pub profile: Option<String>,
    pub findings_by_rule: Vec<RuleTally>,
    pub results: Vec<BatchFileResult>,
    #[serde(skip_serializing_if = "Option::is_none")] pub license: Option<LicenseStamp>,
}

impl ReviewBatch { pub fn build(profile: Option<String>, results: Vec<BatchFileResult>) -> Self; }
```

- **`ReviewReport` is nested unmodified.** No per-file bespoke shape, no duplicated metric fields, and
  `ReviewReport`'s schema, golden and binding consumers are untouched (§12).
- **`passed` = inspected and `error_count == 0`.** Warnings do not fail a file — the same rule
  `review-gcode`'s exit code already uses (`main.rs:1505`).
- **The license is stamped once, on the envelope**; nested `ReviewReport.license` stays `None` and is
  skipped. `license_notice` prints once per run, not once per file.
- **`findings_by_rule` is derived from a `BTreeMap<String, …>`**, so its order is the rule id's order —
  deterministic, and diffable between runs.
- Each file's report goes through `ReviewReport::build(…)` **and** `add_unmodeled_gcode(&imported)`,
  matching `review-gcode` exactly (`main.rs:1444-1452`); a batch that quietly skipped the
  unmodeled-g-code finding would gate differently from the single-file command on the same file.

### 6.3 Memory

Toolpaths are imported, reviewed and dropped one at a time; only the `ReviewReport`s accumulate
(metrics + findings, no geometry). Peak working set is one toolpath plus N small reports — so a
500-file batch is bounded by its largest file, not by their sum. Worth stating because it is the
property that makes the command usable on a farm, and because a naive "import all, then review" would
have neither.

### 6.4 Exit codes

| Code | Meaning |
|---|---|
| `0` | every file was inspected and every file passed |
| `1` | every file was inspected and **at least one** has an `error`-severity finding |
| `2` | **at least one** file could not be inspected (unreadable / unimportable), or a usage error |

`2` outranks `1`. An incomplete batch is neither a pass nor a trustworthy gate: the correct message to
CI is "do not trust this verdict", which is a different fact from "this file is unsafe". And crucially,
an unreadable file **does not abort the run** — it becomes an `errored` result and the other 499 files
are still reviewed. That is the opposite of today's `die` → `exit(2)` behaviour in `review-gcode`, and
it is the whole reason a batch runner exists. `2` for usage errors keeps the CLI's existing convention
(`main.rs:678-681`).

### 6.5 Human output

```
review-batch: 3 file(s), profile voron24-abs
  PASS   a.gcode     1204 segments, no findings
  FAIL   b.gcode      980 segments, 2 error(s), 1 warning(s)
  ERROR  c.gcode     cannot import: unsupported word at line 12
  --
  3 file(s): 1 passed, 1 failed, 1 errored
  by rule: bead 1 warning(s) in 1 file(s); max-flow 2 error(s) in 1 file(s)
```

Sequential, one line per file as it completes, so a long batch shows progress. No parallelism: a pool
means a new dependency and either a nondeterministic result order or a reordering buffer. A farm that
does not need the aggregate can still `xargs -P` over `review-gcode`.

## 7. Schema and validator coverage

`spec/dry-reports-v1.schema.json` is `additionalProperties: false` on every object, so the schema edits
are **mandatory in the same commit** — an analytics-carrying document is invalid until they land.

Additive `$defs`: `Percentiles`, `PhaseStats`, `WindowOutliers`, `LayerStats`, `TraceAnalytics`,
`BatchStatus`, `RuleTally`, `BatchFileResult`, `ReviewBatch`.
Additive properties: `TraceSummary.analytics` (**not** in `required`); the new
`LayerTraceLinkage` fields (in `required` — safe because no document has ever contained an instance,
§4.2, so nothing previously valid becomes invalid); `source_line_start`/`source_line_end` optional
there, mirroring `TraceWindow`.
Unchanged: `TraceWindow`, `TraceReport`, `ReviewReport`, `Metrics`, `Contracts`, `VerifyReport`,
`CompareDelta`, `ExplainBundle`. The schema's top-level `description`, which enumerates the report kinds
and the `$def` each output validates against, gains `ReviewBatch` (`review-batch`).

A schema that describes nothing committed is unvalidated, so the slice adds two goldens and two
`REPORT_KINDS` rows in `tools/validate_reports.py` — `"trace-analytics.json": "TraceReport"` and
`"review-batch.json": "ReviewBatch"`:

| Golden | `$def` | Built from |
|---|---|---|
| `conformance/reports/trace_analytics/trace-analytics.json` | `TraceReport` | `examples/sliced-sample.gcode` at `window_s = 1.0` — see the fixture note below |
| `conformance/reports/review_batch/review-batch.json` | `ReviewBatch` | three of the existing seeded cases: one clean, one with error findings, plus a hand-constructed `errored` entry so that arm of the shape is covered too |

**Fixture note (checked, and it eliminates the obvious choice).** The explain golden's
`examples/sliced-prusa-sample.gcode` (`report_goldens.rs:888-895`) is **not** usable: it emits no `Z`
word at all — only a `;Z:0.2` comment — so `forensics.layers.layer_count` is `0` in
`conformance/reports/forensics-prusa/forensics.json` and the file has no layer structure for the
linkage to find. `examples/sliced-sample.gcode` (the Cura sample behind
`conformance/reports/forensics/forensics.json`) has two real layers, `Z0.2`/`Z0.4`, layer height 0.2 —
so it is the fixture. `window_s = 1.0` rather than the CLI default 5.0: the file runs ≈10 s, so 5.0 s
windows give two windows and a degenerate percentile population, while 1.0 s gives ~10 windows over 2
layers — non-trivial and still a small golden.

Both are generated and drift-gated by `report_goldens.rs` under `UPDATE_REPORTS=1`, like every other
golden. `validate_reports.py` needs no structural change — it already walks case directories and
validates whichever known filenames are present.

`docs/11-profiles-and-reports.md`: §3.3 extended (analytics, `layers` empty without the flag, the
`layers`-vs-`forensics` divergence, both CSV relations); new §3.9 for `review-batch` including the exit
table. `docs/15-cli-cookbook.md`: recipes for both. `docs/16-support-matrix.md`: rows for trace
analytics and batch review. `docs/04-tasks.md` P3.5: struck items, and the remainder narrowed to
Parquet/Arrow + diffing (rasters removed per §2).

## 8. Parquet/Arrow — deferred, and what it would look like

**Deferred, explicitly.** `arrow` + `parquet` pull dozens of transitive crates; `dry-core` is
dependency-free by policy and the CLI should not grow that tree for an export nothing has asked for by
name. `arrow` also does not build cleanly for `wasm32`, so a core feature flag would fracture the
binding surface — the very thing the workspace's exclusion list exists to prevent.

When it is wanted, it follows `crates/llm`'s pattern exactly: `crates/llm` exists solely to keep the
one network dependency (`ureq`) behind a crate boundary, and the CLI reaches it through an *optional*
dependency plus a non-default feature (`llm = ["dep:dry-llm"]`, `crates/cli/Cargo.toml`). So: a new
workspace member `crates/trace-export` depending on `dry-core` + `arrow`/`parquet`, exposing
`write_windows_parquet(&TraceSummary, W)` and `write_layers_parquet(&TraceSummary, W)` over the two
relations §4.3 defines; the CLI gains `parquet = ["dep:dry-trace-export"]`, off by default, adding
`--format parquet` only when compiled in. Core stays dependency-free, the default `cargo build` tree is
unchanged, and the column sets are already fixed by the CSV writers — which is the point of shipping
CSV first rather than sketching a columnar schema in the abstract. **CSV and JSON are this slice's
export boundary.**

## 9. Out of scope

### 9.1 Before/after trace diffing — and why it comes next, not now

`compare` already consumes both files' full `ExplainReports` (`compare.rs:89`), of which `TraceReport`
is a member, and already surfaces four trace-derived numbers (total/print/travel time and
`max_flow_mm3_s`). So trace diffing slots in as additional `CompareDelta` fields — no new command, no
new report kind:

- **Aggregate deltas** are easy: `ScalarDelta` over `analytics.print.flow_mm3_s.p95`,
  `travel_time_ratio`, `layer_stats.layer_count`, outlier count.
- **Per-window diffing is meaningless** and this is the substantive reason to defer. Windows are
  time-indexed from t=0; two files with different total times have no window correspondence — window 40
  of a 12-minute print and window 40 of a 15-minute print are different parts of the object. The only
  honest alignment key is **layer Z**, which requires the layer linkage this slice builds. A per-layer
  before/after table (time, volume, peak flow per matched Z, plus layers present on one side only) is
  the shape worth having, and it is only expressible once §3.5 exists.

That ordering — linkage first, diff second — is the reason this slice stops here rather than shipping
half a diff against an alignment key that does not yet exist.

### 9.2 Also out

Layer rasters (§2); streaming analytics (§4.4); any new verify rule or gate (§2); histogram/sketch
percentiles; `ReviewReport` field additions; parallel batch execution (§6.5).

## 10. Error handling

- `TraceAnalyticsOptions` validation reuses `TraceError` (`trace.rs:71-90`) and runs before any work:
  `flow_outlier_k` must be finite and `> 0`; `layer_z_epsilon_mm` finite and `>= 0`. Same shape as
  `validate_window` (`trace.rs:190-197`).
- No panics on user input, and no `unwrap` on a percentile: every accessor returns `Option` where the
  population can be empty (§3.1).
- Non-finite sample values are excluded and counted, never propagated into a percentile (§3.1).
- `review-batch` per-file failures are captured as `BatchFileResult::error` strings — the message text
  from the existing import/read error — and never abort the batch (§6.4). Only a usage error
  (no files, `--flow-outlier-k` misuse, unreadable `--files-from`) goes through `die`.

## 11. Testing

Every item below runs in `cargo test -p dry-core` / `cargo test -p dry-cli` unless marked.

1. **Percentile definition (unit).** Hand-computed cases against §3.1: single sample (all four
   statistics equal); two samples of equal weight (`p50` = lower); weight-dominated case where the
   time-weighted `p50` differs from the count-weighted one (proves the weighting is real); `p95` on 20
   samples; `W == 0` → `None`; a tie group with distinct weights ordered two ways → identical output
   (the stability argument, §3.1).
2. **Non-finite exclusion (unit).** A toolpath with one NaN and one `+inf` feedrate: percentiles are
   finite, `nonfinite_samples == 2`, and the documented cross-check invariant (§3.2) is *not* asserted
   for that case.
3. **Phase/summary cross-check (unit).**
   `max(print.feedrate_mm_min.max, travel.feedrate_mm_min.max) == summary.max_feedrate_mm_min`
   exactly, and likewise for `flow_mm3_s` against `summary.max_flow_mm3_s`, on an all-finite fixture.
4. **Layer partition invariants (unit).** `segment_start == 0`, contiguity, `segment_end ==
   segment_count`, `Σ print_time` within `1e-9` relative of the summary total, on: a monotonic 3-layer
   path; a path with a re-visited Z (two entries at the same Z, proving passes-not-levels); a
   travel-only path (`layers == []`, `layer_stats == None`); a single-segment path. Plus two rule-shape
   tests: a mid-layer Z-hop travel must **not** split the layer, and the lift/approach travel before a
   break must land in the layer being entered (the back-walk, §3.5).
5. **Forensics divergence (integration).** On `examples/sliced-sample.gcode` (two monotonic layers,
   `forensics.layers.layer_count == 2`), `trace.layers.len() == 2` — equality, since Z is
   non-decreasing and each level is one contiguous run; plus a synthetic re-visited-Z toolpath where
   `trace.layers.len() > forensics.layers.layer_count`. Pins the documented relationship in both
   directions rather than leaving it as prose.
6. **Outlier threshold (unit).** Constant-flow file at `k = 1` flags nothing (strict `>`); one 5×
   window at `k = 2` flags exactly that index; `threshold_mm3_s == k * window_flow_mm3_s.p50` exactly
   (reproducibility from the published numbers, §3.4).
7. **Golden byte-identity (the requirement).** `cargo test -p dry-core` with **no** `UPDATE_REPORTS`
   must leave all seven existing `trace.json` goldens and `explain.json` unchanged. Plus an explicit
   test that `trace_summary_with_sources(...)` and `trace_summary_with_analytics(...)` serialise
   identically once `analytics` and `layers` are stripped — i.e. the analytics pass changes no
   pre-existing number.
8. **New goldens** (§7), generated and drift-gated; `python tools/validate_reports.py .` green,
   confirming the schema describes them with no `dry-core` in the loop.
9. **Batch aggregation (unit, core).** `ReviewBatch::build` counts; `findings_by_rule` ordering and
   per-rule `files` counting (a rule twice in one file counts one file, two errors); the
   `review.is_some() != error.is_some()` invariant.
10. **CLI (`crates/cli/tests/cli.rs`).** `trace-gcode` default output unchanged; `--analytics` adds
    both keys; `--format csv` header matches `to_csv`; `--format layers-csv` emits rows; a bare
    `--flow-outlier-k` exits 2. `review-batch` on three fixtures (clean / gating / missing file) →
    exit 2 with all three inspected-or-reported; drop the missing file → exit 1; clean-only → exit 0;
    `--files-from -` equivalence with positionals.
11. **Docs drift gates.** `crates/cli/src/main.rs` and `docs/15-cli-cookbook.md` are hashed inputs to
    `docs/site/scripts/gen-reference.mjs`, so `docs/site/reference/generated` must be regenerated and
    `check-reference-coverage.mjs` re-run. This has failed a slice before (2026-07-30) — it is part of
    the task, not an afterthought.

## 12. Contracts, proofs and binding parity

**Proofs — nothing is added and nothing is invalidated, and this slice must not imply coverage.**
`proofs/claims.toml`'s metric-fold claims (`FM1…SIMULATE…`, e.g. the exact-rational fold at
`claims.toml:1342`) are about `simulate` over L2 segment traces, not about `trace_summary`; trace has no
Lean model and this slice does not give it one. No claim's `theorem`, `relation` or `exclusions` change.

**No new numeric-boundary inventory**, deliberately, and the reason is structural rather than
convenience: `proofs/numeric-boundaries.schema.json` closes `model` to four enumerated semantic models,
and every `boundary` entry requires a `numeric_profile_id` matching `FM1.NUMERIC.PROFILE.*` plus
`claim_ids` into `claims.toml`. Publishing an inventory for a reporting layer would mean inventing a
numeric profile and claim ids that nothing proves — worse than not publishing one. What this slice does
instead: the two tolerances are **self-describing in the output** (`k` and `threshold_mm3_s` are
serialised; the layer epsilon is a documented constant shared with `forensics`), and the arithmetic is
`+`, `*`, `/` and comparison only — no transcendental — so cross-SDK bit-identity rests on the same
argument as the existing folds. If a reviewer wants an inventory, that is a schema change to
`numeric-boundaries.schema.json` plus a numeric profile, and it should be its own decision.

**Spec clauses.** `proofs/spec-claim-links.toml`'s `DRY.REPORT.METRICS_V1` points at `docs/11` §3
("report schemas define the metric summaries and located evidence exposed to independent consumers").
Extending §3.3 and adding §3.9 keeps that clause true and needs no edit to the clause text; the
additive schema `$defs` are what keep it true.

**Binding parity — checked, and the answer is "nothing, provided §6.2 holds".** No binding touches
trace: `trace_summary`/`TraceSummary` appear nowhere in `crates/wasm`, `crates/cloud`, `py/`, `sdk/ts`
or `containers/verify-runner`, so the trace half needs no out-of-workspace re-verification. No binding
constructs a `ReviewReport` either — the two that replicate a review-shaped pipeline stop at
`simulate` + `verify` and serialise a `Report`: `crates/cloud/src/lib.rs:57-59` (ad-hoc timing JSON;
note its module doc at lines 8-9 still *claims* `ReviewReport::build`, which the code does not call —
a stale comment, not a dependency) and `containers/verify-runner/src/lib.rs:359-380`, whose output is
byte-identical to `dry verify --json` including the license stamp.

Consequence: this slice, as specified, has **no binding-visible behaviour change** — which is exactly
why §6.2 nests `ReviewReport` unmodified and §2 rules out touching `Report`/`Metrics`/`VerifyReport`.
If the implementing slice ends up changing any of those three after all, `crates/cloud` and
`containers/verify-runner` build outside the workspace with their own locks and **must be built and
tested from their own directories before the slice is called done** — a workspace `cargo test` cannot
see them, and `verify_report_is_byte_identical_to_the_real_cli`
(`containers/verify-runner/tests/handler.rs:351`), which shells out to the real CLI for its ground
truth, is the first thing a `Report` change breaks.

## 13. Alternatives considered

- **A separate `TraceStatistics` struct / report kind** — rejected (§4.1): two documents to correlate,
  a second schema kind and validator row, and `window_indices` is meaningless away from `windows`.
- **Populate `layers` unconditionally** — rejected: rewrites seven trace goldens plus the explain
  bundle, which the brief forbids. The cost is an always-serialised field that is empty by default,
  documented in `docs/11` §3.3.
- **`flow_outlier: bool` per `TraceWindow`** — rejected (§4.2): needs a skip helper, smears one
  statistic across W objects, and edits the most-validated `$def` in the schema.
- **Variadic `review-gcode`** — rejected (§6.1): shape-by-arity, or a broken published contract, or no
  aggregate. Also drags batch exit semantics into a command whose exit rule is already documented.
- **Interpolated percentiles (linear between ranks)** — rejected (§3.1): the value is quoted as a
  threshold, and an interpolated one never occurred in the file.
- **Reuse `forensics`' `median` helper** — rejected: it averages the two middle values, so it would
  disagree with `p50` computed two files away. `p50` under one definition serves both, and no second
  helper is added.
- **Histogram/sketch percentiles for bounded memory** — rejected for this slice: approximate answers to
  an exact question, and either a new dependency or a bin-edge policy to defend. The scratch ledger is
  a fraction of the toolpath the caller already holds (§3.6).
- **`arrow`/`parquet` in core behind a feature flag** — rejected (§8): breaks the dependency-free
  policy and does not build for `wasm32`; the `crates/llm` boundary pattern is the answer when it is
  wanted.
- **Per-flag contract overrides on `review-batch`** — rejected for now (§6.1): a fleet gates on a
  profile, and sharing the flag block with `review-gcode` reorders its `--help` and churns the
  generated reference. Follow-up if a pilot needs it.
- **Parallel batch execution** — rejected (§6.5): new dependency, nondeterministic ordering.

## 14. Follow-ups

- **Trace diffing** in `compare`, aligned on layer Z (§9.1) — the natural next slice, unblocked by
  §3.5.
- **Parquet/Arrow** via `crates/trace-export` + an optional CLI feature (§8).
- **Batch review at the service edge** — `docs/09` also lists dashboard/API, profile fleet management
  and audit logs as gaps; `ReviewBatch` is the wire shape those would carry, which is why it lives in
  core rather than the CLI.
- **Shared contract-flag args struct** for `review-gcode` / `review-batch` / `explain`, once someone
  is willing to absorb the generated-reference churn.
- **Strike "raster" from P3.5** in `docs/04-tasks.md` (§2) rather than leaving it implying work in
  flight.

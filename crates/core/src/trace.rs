//! Windowed motion/time-series summaries over Dry IR.
//!
//! # Order statistics
//!
//! [`trace_summary_with_analytics`] publishes percentiles, and every one of them — over segments,
//! over window peaks, over layers — uses one definition:
//!
//! ```text
//! Given samples (vᵢ, wᵢ) with wᵢ > 0 and W = Σwᵢ > 0, sorted ascending by v with a STABLE sort:
//!     quantile(p) = the first vᵢ whose cumulative Σ_{j≤i} wⱼ ≥ p·W        for p ∈ (0, 1]
//! ```
//!
//! This is the nearest-rank (inverse-CDF, lower) definition, and the consequences are deliberate:
//!
//! - **The result is always a value that actually occurred.** There is no interpolation, so a
//!   threshold quoted from a percentile (see [`WindowOutliers`]) refers to a real observed rate
//!   rather than an average of two.
//! - **It differs from the `median` in [`crate::forensics`]**, which averages the two middle values
//!   on an even count. Rather than add a second `median` helper with a third meaning, `p50` *is* the
//!   median under this definition and the divergence is stated here.
//! - **Weights unify the two populations.** Segment statistics weight by motion seconds ("for 95% of
//!   print time, flow was at or below X" — the question a hotend limit answers); window and layer
//!   statistics weight every sample by 1, because windows are equal-duration by construction.
//! - **It is deterministic.** The sort is `f64::total_cmp` under std's *stable* sort, so equal-valued
//!   samples never permute, the partial sums inside a tie group are fixed, and the whole computation
//!   is a function of segment order alone. The arithmetic is `+`, `*`, `/` and comparison only.
//! - **Non-finite sample values are excluded and counted** (`nonfinite_samples`), never folded into
//!   a NaN percentile. Totals and time-weighted means keep the engine's existing behaviour and do
//!   propagate them.
//! - **An empty population yields `None`, not zero** — a phase with no motion has no percentiles,
//!   and `0.0` would be indistinguishable from a real stall.

use crate::engine::segment_motion_time;
use crate::ir::{Segment, Toolpath};
use serde::{Deserialize, Serialize};

/// Z tolerance (mm) below which two extruding moves are the same layer — the same epsilon
/// `crates/core/src/forensics.rs` uses to dedup its Z set, so the two layer notions key on the same
/// tolerance even though they count different things (see [`LayerTraceLinkage`]).
const LAYER_Z_EPSILON_MM: f64 = 1e-6;

/// A compact time-series summary of a toolpath.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceSummary {
    /// Requested fixed window duration in seconds.
    pub window_s: f64,
    /// Total number of segments in the source toolpath.
    pub segment_count: usize,
    /// Number of segments that actually move.
    pub moving_segment_count: usize,
    pub total_time_s: f64,
    pub print_time_s: f64,
    pub travel_time_s: f64,
    pub dwell_time_s: f64,
    pub extruding_distance_mm: f64,
    pub travel_distance_mm: f64,
    pub extruded_volume_mm3: f64,
    pub filament_mm: f64,
    pub max_feedrate_mm_min: f64,
    pub max_flow_mm3_s: f64,
    pub windows: Vec<TraceWindow>,
    /// Per-layer segment ranges and aggregates. Populated only by
    /// [`trace_summary_with_analytics`]; the plain entry points leave it empty.
    pub layers: Vec<LayerTraceLinkage>,
    /// Higher-level statistics, present only when the caller asked for them via
    /// [`trace_summary_with_analytics`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analytics: Option<TraceAnalytics>,
}

/// Linkage between Z-height layers and trace segment ranges: a layer is a window whose boundary is Z
/// instead of time, so it carries the same field set a [`TraceWindow`] does minus the time bounds.
///
/// A layer here is a *pass* in execution order, not a distinct Z level: a re-visited Z (an ironing
/// pass, a non-monotonic vase) produces a second entry. `ForensicsReport.layers.layer_count` counts
/// distinct levels instead, so `trace.layers.len() >= forensics.layers.layer_count`, with equality
/// when Z is non-decreasing and each level is a single contiguous run. The two numbers answer
/// different questions and both keep their names. Trace does not publish a layer *height*: the
/// forensics report owns that estimate, and a second one from a different population would put two
/// differently-defined numbers under one name in the same `explain` bundle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerTraceLinkage {
    /// Sequence order, 0-based.
    pub layer_index: usize,
    /// The layer's extruding Z.
    pub z_mm: f64,
    /// Inclusive first segment of the layer.
    pub segment_start: usize,
    /// Exclusive segment end, matching [`TraceWindow::segment_end`].
    pub segment_end: usize,
    pub print_time_s: f64,
    pub travel_time_s: f64,
    pub extruded_volume_mm3: f64,
    pub dwell_time_s: f64,
    pub extruding_distance_mm: f64,
    pub travel_distance_mm: f64,
    pub filament_mm: f64,
    pub max_feedrate_mm_min: f64,
    pub max_flow_mm3_s: f64,
    /// First original source line in the layer, when the trace came from imported G-code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_line_start: Option<usize>,
    /// Last original source line in the layer, when the trace came from imported G-code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_line_end: Option<usize>,
}

/// Exact order statistics under the nearest-rank definition described in the [module docs](self).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Percentiles {
    pub min: f64,
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
}

/// Time-weighted statistics over the moving segments of one phase (print or travel).
///
/// Dwell time is excluded: a dwell has no feedrate and no flow. Travel-phase *flow* is reported even
/// though it should be identically zero — a non-zero value is exactly the smell the `travel-extrudes`
/// rule names, and having the trace corroborate a verify rule from an independent computation is
/// worth four bytes of JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseStats {
    /// Segments in this phase with `motion_s > 0`.
    pub segments: usize,
    pub time_s: f64,
    pub distance_mm: f64,
    pub volume_mm3: f64,
    /// `Σ(v·t)/Σt` over every segment in the phase. `None` when `time_s == 0`. Unlike the
    /// percentiles, this does *not* filter non-finite samples — it is a total, and totals keep the
    /// engine's existing behaviour.
    pub mean_feedrate_mm_min: Option<f64>,
    pub mean_flow_mm3_s: Option<f64>,
    /// Time-weighted percentiles. `None` when no finite-valued sample carries weight.
    pub feedrate_mm_min: Option<Percentiles>,
    pub flow_mm3_s: Option<Percentiles>,
    /// Segments in the phase whose feedrate or flow was non-finite, and therefore excluded from the
    /// percentiles above.
    pub nonfinite_samples: usize,
}

/// Windows whose peak flow stands out against the batch, as an observation — never a gate.
///
/// The reference is the **already-published** [`TraceAnalytics::window_flow_mm3_s`] `p50`, not a
/// separately computed median, so the threshold is reproducible by any consumer from two numbers in
/// the same document and there is no second median definition to drift. Flow only, deliberately: it
/// is the channel with a physical ceiling (hotend melt rate), so an outlier there has a mechanism,
/// whereas feedrate outliers are mostly travels, which are supposed to be fast.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowOutliers {
    /// The multiplier applied to the reference. Echoed from the options.
    pub k: f64,
    /// `k × window_flow_mm3_s.p50`. `None` when there is no reference (no window with motion).
    pub threshold_mm3_s: Option<f64>,
    /// Ascending indices of windows with motion whose `max_flow_mm3_s` is strictly greater than the
    /// threshold. Strict `>` so `k = 1` on a constant-flow file flags nothing.
    pub window_indices: Vec<usize>,
}

/// Order statistics over the per-layer aggregates of [`TraceSummary::layers`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerStats {
    pub layer_count: usize,
    /// Order statistics over per-layer values (unweighted; one sample per layer).
    pub print_time_s: Percentiles,
    pub extruded_volume_mm3: Percentiles,
    /// Layer with the greatest `print_time_s`; ties resolve to the lowest index.
    pub slowest_layer_index: usize,
}

/// Higher-level statistics over a [`TraceSummary`] — see the [module docs](self) for the percentile
/// definition every field here shares.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceAnalytics {
    pub print: PhaseStats,
    pub travel: PhaseStats,
    /// Order statistics over per-window *peak* flow (not over instantaneous flow), unweighted, over
    /// windows with `duration_s > 0`. Zero-duration windows carry no motion — they exist only so a
    /// zero-length segment still gets a segment/source-line anchor — and including them would drag
    /// every percentile toward zero.
    pub window_flow_mm3_s: Option<Percentiles>,
    /// Order statistics over per-window peak feedrate, same population as `window_flow_mm3_s`.
    pub window_feedrate_mm_min: Option<Percentiles>,
    pub flow_outliers: WindowOutliers,
    /// `None` when there is no layer to aggregate (see [`TraceSummary::layers`]).
    pub layer_stats: Option<LayerStats>,
    /// travel motion time / total motion time, in `[0,1]`. `None` when nothing moved.
    pub travel_time_ratio: Option<f64>,
    /// How many windows the window statistics were computed over — a p50 over three windows is not a
    /// trend, and the number that says so is published rather than inferred.
    pub windows_considered: usize,
    /// How many segments the phase statistics were computed over (the moving segments).
    pub segments_considered: usize,
}

/// Tolerances for [`trace_summary_with_analytics`].
#[derive(Debug, Clone, PartialEq)]
pub struct TraceAnalyticsOptions {
    /// Multiple of the window-peak p50 above which a window is flagged. Default `2.0`.
    pub flow_outlier_k: f64,
    /// Z tolerance (mm) for "same layer". Default `1e-6`.
    pub layer_z_epsilon_mm: f64,
}

impl Default for TraceAnalyticsOptions {
    fn default() -> Self {
        TraceAnalyticsOptions {
            flow_outlier_k: 2.0,
            layer_z_epsilon_mm: LAYER_Z_EPSILON_MM,
        }
    }
}

/// One fixed-duration trace window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceWindow {
    pub index: usize,
    pub start_time_s: f64,
    pub end_time_s: f64,
    /// First segment index touching this window.
    pub segment_start: Option<usize>,
    /// Exclusive segment end index touching this window.
    pub segment_end: Option<usize>,
    /// First original source line touching this window, when the trace came from imported G-code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_line_start: Option<usize>,
    /// Last original source line touching this window, when the trace came from imported G-code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_line_end: Option<usize>,
    pub duration_s: f64,
    pub print_time_s: f64,
    pub travel_time_s: f64,
    pub dwell_time_s: f64,
    pub extruding_distance_mm: f64,
    pub travel_distance_mm: f64,
    pub extruded_volume_mm3: f64,
    pub filament_mm: f64,
    pub max_feedrate_mm_min: f64,
    pub max_flow_mm3_s: f64,
}

/// A trace configuration error.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceError {
    message: String,
}

impl TraceError {
    fn new(message: impl Into<String>) -> Self {
        TraceError {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for TraceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TraceError {}

#[derive(Debug, Clone, Copy)]
struct SegmentTiming {
    motion_s: f64,
    dwell_s: f64,
    flow_mm3_s: f64,
}

impl TraceSummary {
    fn new(window_s: f64, segment_count: usize) -> Self {
        TraceSummary {
            window_s,
            segment_count,
            moving_segment_count: 0,
            total_time_s: 0.0,
            print_time_s: 0.0,
            travel_time_s: 0.0,
            dwell_time_s: 0.0,
            extruding_distance_mm: 0.0,
            travel_distance_mm: 0.0,
            extruded_volume_mm3: 0.0,
            filament_mm: 0.0,
            max_feedrate_mm_min: 0.0,
            max_flow_mm3_s: 0.0,
            windows: Vec::new(),
            layers: Vec::new(),
            analytics: None,
        }
    }

    /// Formats the windowed trace time-series as CSV records for tabular analysis / Parquet export.
    pub fn to_csv(&self) -> String {
        let mut out = String::from(
            "window_index,start_time_s,end_time_s,print_time_s,travel_time_s,dwell_time_s,extruding_distance_mm,travel_distance_mm,extruded_volume_mm3,max_feedrate_mm_min,max_flow_mm3_s\n",
        );
        for w in &self.windows {
            out.push_str(&format!(
                "{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}\n",
                w.index,
                w.start_time_s,
                w.end_time_s,
                w.print_time_s,
                w.travel_time_s,
                w.dwell_time_s,
                w.extruding_distance_mm,
                w.travel_distance_mm,
                w.extruded_volume_mm3,
                w.max_feedrate_mm_min,
                w.max_flow_mm3_s
            ));
        }
        out
    }

    /// Formats the per-layer aggregates as CSV records — the second relation of the trace, at the
    /// grain [`LayerTraceLinkage`] defines. Empty apart from its header unless the summary came from
    /// [`trace_summary_with_analytics`].
    ///
    /// The source-line range is deliberately not a column: the column set must not vary with whether
    /// the caller supplied a source map, and a tabular consumer's first requirement is a stable
    /// schema. Aggregate analytics stay JSON-only for the same reason.
    pub fn layers_to_csv(&self) -> String {
        let mut out = String::from(
            "layer_index,z_mm,segment_start,segment_end,print_time_s,travel_time_s,dwell_time_s,extruding_distance_mm,travel_distance_mm,extruded_volume_mm3,filament_mm,max_feedrate_mm_min,max_flow_mm3_s\n",
        );
        for l in &self.layers {
            out.push_str(&format!(
                "{},{:.6},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}\n",
                l.layer_index,
                l.z_mm,
                l.segment_start,
                l.segment_end,
                l.print_time_s,
                l.travel_time_s,
                l.dwell_time_s,
                l.extruding_distance_mm,
                l.travel_distance_mm,
                l.extruded_volume_mm3,
                l.filament_mm,
                l.max_feedrate_mm_min,
                l.max_flow_mm3_s
            ));
        }
        out
    }
}

impl TraceWindow {
    fn new(index: usize, window_s: f64) -> Self {
        TraceWindow {
            index,
            start_time_s: index as f64 * window_s,
            end_time_s: (index + 1) as f64 * window_s,
            segment_start: None,
            segment_end: None,
            source_line_start: None,
            source_line_end: None,
            duration_s: 0.0,
            print_time_s: 0.0,
            travel_time_s: 0.0,
            dwell_time_s: 0.0,
            extruding_distance_mm: 0.0,
            travel_distance_mm: 0.0,
            extruded_volume_mm3: 0.0,
            filament_mm: 0.0,
            max_feedrate_mm_min: 0.0,
            max_flow_mm3_s: 0.0,
        }
    }

    fn touch_segment(&mut self, segment: usize, source_line: Option<usize>) {
        self.segment_start = Some(
            self.segment_start
                .map_or(segment, |current| current.min(segment)),
        );
        self.segment_end = Some(
            self.segment_end
                .map_or(segment + 1, |current| current.max(segment + 1)),
        );
        if let Some(source_line) = source_line {
            self.source_line_start = Some(
                self.source_line_start
                    .map_or(source_line, |current| current.min(source_line)),
            );
            self.source_line_end = Some(
                self.source_line_end
                    .map_or(source_line, |current| current.max(source_line)),
            );
        }
    }
}

fn validate_window(window_s: f64) -> Result<(), TraceError> {
    if !window_s.is_finite() || window_s <= 0.0 {
        return Err(TraceError::new(
            "trace window must be a positive finite number of seconds",
        ));
    }
    Ok(())
}

fn validate_analytics_options(options: &TraceAnalyticsOptions) -> Result<(), TraceError> {
    if !options.flow_outlier_k.is_finite() || options.flow_outlier_k <= 0.0 {
        return Err(TraceError::new(
            "trace flow-outlier k must be a positive finite multiplier",
        ));
    }
    if !options.layer_z_epsilon_mm.is_finite() || options.layer_z_epsilon_mm < 0.0 {
        return Err(TraceError::new(
            "trace layer Z epsilon must be a non-negative finite number of millimetres",
        ));
    }
    Ok(())
}

/// One `(value, weight)` percentile sample.
#[derive(Debug, Clone, Copy)]
struct WeightedSample {
    value: f64,
    weight: f64,
}

/// Order statistics over `(value, weight)` samples — the definition in the [module docs](self).
///
/// The caller filters non-finite values and non-positive weights out first (and counts the non-finite
/// ones), so every sample reaching here carries real weight.
fn percentiles(samples: &mut [WeightedSample]) -> Option<Percentiles> {
    let total: f64 = samples.iter().map(|s| s.weight).sum();
    // A population with no weight — or one whose weight is not a usable number — has no percentiles.
    if samples.is_empty() || !(total.is_finite() && total > 0.0) {
        return None;
    }
    // Stability is load-bearing, not decorative: with an unstable sort, equal-valued samples permute,
    // the partial sums inside a tie group differ in their last bits, and a later group's
    // `cumulative >= p*total` comparison can flip. `total_cmp` also gives every f64 a defined
    // position instead of the partial-order hole `partial_cmp` leaves.
    samples.sort_by(|a, b| a.value.total_cmp(&b.value));
    Some(Percentiles {
        min: samples[0].value,
        p50: quantile(samples, total, 0.5),
        p95: quantile(samples, total, 0.95),
        max: samples[samples.len() - 1].value,
    })
}

/// The first sample value whose cumulative weight reaches `p × total`, over samples already sorted
/// ascending by value.
fn quantile(sorted: &[WeightedSample], total: f64, p: f64) -> f64 {
    let target = p * total;
    let mut cumulative = 0.0;
    for sample in sorted {
        cumulative += sample.weight;
        if cumulative >= target {
            return sample.value;
        }
    }
    // Only reachable when rounding leaves the running sum just short of `target` at `p = 1`; the
    // largest sample is the answer by definition.
    sorted[sorted.len() - 1].value
}

/// Order statistics over an unweighted population (one sample per window / per layer).
fn percentiles_unweighted(values: impl IntoIterator<Item = f64>) -> Option<Percentiles> {
    let mut samples: Vec<WeightedSample> = values
        .into_iter()
        .filter(|value| value.is_finite())
        .map(|value| WeightedSample { value, weight: 1.0 })
        .collect();
    percentiles(&mut samples)
}

/// The scratch ledger for one phase (print or travel): running totals plus the percentile samples.
#[derive(Debug, Default)]
struct PhaseAccum {
    segments: usize,
    time_s: f64,
    distance_mm: f64,
    volume_mm3: f64,
    feedrate_time: f64,
    flow_time: f64,
    nonfinite_samples: usize,
    feedrate: Vec<WeightedSample>,
    flow: Vec<WeightedSample>,
}

impl PhaseAccum {
    fn add(&mut self, feedrate: f64, flow: f64, seconds: f64, distance_mm: f64, volume_mm3: f64) {
        self.segments += 1;
        self.time_s += seconds;
        self.distance_mm += distance_mm;
        self.volume_mm3 += volume_mm3;
        self.feedrate_time += feedrate * seconds;
        self.flow_time += flow * seconds;
        if !feedrate.is_finite() || !flow.is_finite() {
            self.nonfinite_samples += 1;
        }
        if feedrate.is_finite() {
            self.feedrate.push(WeightedSample {
                value: feedrate,
                weight: seconds,
            });
        }
        if flow.is_finite() {
            self.flow.push(WeightedSample {
                value: flow,
                weight: seconds,
            });
        }
    }

    fn finish(mut self) -> PhaseStats {
        let mean = |sum: f64, total: f64| if total > 0.0 { Some(sum / total) } else { None };
        PhaseStats {
            segments: self.segments,
            time_s: self.time_s,
            distance_mm: self.distance_mm,
            volume_mm3: self.volume_mm3,
            mean_feedrate_mm_min: mean(self.feedrate_time, self.time_s),
            mean_flow_mm3_s: mean(self.flow_time, self.time_s),
            feedrate_mm_min: percentiles(&mut self.feedrate),
            flow_mm3_s: percentiles(&mut self.flow),
            nonfinite_samples: self.nonfinite_samples,
        }
    }
}

/// The running aggregate of one layer's segment range.
#[derive(Debug, Default)]
struct LayerAccum {
    print_time_s: f64,
    travel_time_s: f64,
    dwell_time_s: f64,
    extruding_distance_mm: f64,
    travel_distance_mm: f64,
    extruded_volume_mm3: f64,
    filament_mm: f64,
    max_feedrate_mm_min: f64,
    max_flow_mm3_s: f64,
    source_line_start: Option<usize>,
    source_line_end: Option<usize>,
}

impl LayerAccum {
    /// Accrue one segment, on exactly the terms [`trace_summary_with_sources`] accrues its own totals
    /// — which is what makes `Σ layer.print_time_s == summary.print_time_s` hold.
    fn add_segment(
        &mut self,
        segment: &Segment,
        timing: SegmentTiming,
        source_line: Option<usize>,
    ) {
        if let Some(line) = source_line {
            self.source_line_start = Some(
                self.source_line_start
                    .map_or(line, |current| current.min(line)),
            );
            self.source_line_end = Some(
                self.source_line_end
                    .map_or(line, |current| current.max(line)),
            );
        }
        if timing.motion_s > 0.0 {
            if segment.travel {
                self.travel_time_s += timing.motion_s;
                self.travel_distance_mm += segment.length.value();
            } else {
                self.print_time_s += timing.motion_s;
                self.extruding_distance_mm += segment.length.value();
            }
            self.extruded_volume_mm3 += segment.volume.value();
            self.filament_mm += segment.filament.value();
            self.max_feedrate_mm_min = self.max_feedrate_mm_min.max(segment.speed.value());
            self.max_flow_mm3_s = self.max_flow_mm3_s.max(timing.flow_mm3_s);
        }
        if timing.dwell_s > 0.0 {
            self.dwell_time_s += timing.dwell_s;
        }
    }

    fn merge(&mut self, other: LayerAccum) {
        self.print_time_s += other.print_time_s;
        self.travel_time_s += other.travel_time_s;
        self.dwell_time_s += other.dwell_time_s;
        self.extruding_distance_mm += other.extruding_distance_mm;
        self.travel_distance_mm += other.travel_distance_mm;
        self.extruded_volume_mm3 += other.extruded_volume_mm3;
        self.filament_mm += other.filament_mm;
        self.max_feedrate_mm_min = self.max_feedrate_mm_min.max(other.max_feedrate_mm_min);
        self.max_flow_mm3_s = self.max_flow_mm3_s.max(other.max_flow_mm3_s);
        self.source_line_start = match (self.source_line_start, other.source_line_start) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        self.source_line_end = match (self.source_line_end, other.source_line_end) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
    }

    fn finish(
        self,
        layer_index: usize,
        z_mm: f64,
        segment_start: usize,
        segment_end: usize,
    ) -> LayerTraceLinkage {
        LayerTraceLinkage {
            layer_index,
            z_mm,
            segment_start,
            segment_end,
            print_time_s: self.print_time_s,
            travel_time_s: self.travel_time_s,
            extruded_volume_mm3: self.extruded_volume_mm3,
            dwell_time_s: self.dwell_time_s,
            extruding_distance_mm: self.extruding_distance_mm,
            travel_distance_mm: self.travel_distance_mm,
            filament_mm: self.filament_mm,
            max_feedrate_mm_min: self.max_feedrate_mm_min,
            max_flow_mm3_s: self.max_flow_mm3_s,
            source_line_start: self.source_line_start,
            source_line_end: self.source_line_end,
        }
    }
}

/// The one-pass layer partitioner.
///
/// A layer *break* is detected at the first **extruding** segment whose Z differs from the current
/// layer's Z by more than `epsilon`. Keying on extruding Z specifically is what makes the rule robust:
/// keying on any segment's Z would let a single Z-hop travel shatter a layer into three, whereas a
/// hop's travels sit between extrusions at the same Z.
///
/// The new layer's `segment_start` is walked **back** over the immediately preceding run of
/// non-extruding segments (`pending`), so the Z lift and the approach travel belong to the layer they
/// are entering — which is where a slicer's own `;LAYER:` annotation puts them. Layer 0 starts at
/// segment 0 (a prime prologue is attributed, not orphaned) and the last layer ends at
/// `segment_count` (so is a wipe/park epilogue), which makes the linkage a partition.
#[derive(Debug)]
struct LayerPartition {
    epsilon: f64,
    layers: Vec<LayerTraceLinkage>,
    current: LayerAccum,
    /// The run of non-extruding segments since the last extruding one, held aside until the segment
    /// that follows it says which layer it belongs to.
    pending: LayerAccum,
    current_z: Option<f64>,
    current_start: usize,
    last_extruding: Option<usize>,
}

impl LayerPartition {
    fn new(epsilon: f64) -> Self {
        LayerPartition {
            epsilon,
            layers: Vec::new(),
            current: LayerAccum::default(),
            pending: LayerAccum::default(),
            current_z: None,
            current_start: 0,
            last_extruding: None,
        }
    }

    fn observe(
        &mut self,
        index: usize,
        segment: &Segment,
        timing: SegmentTiming,
        source_line: Option<usize>,
    ) {
        let extruding = !segment.travel && segment.volume.value() > 0.0;
        if !extruding {
            self.pending.add_segment(segment, timing, source_line);
            return;
        }
        let z = segment.end[2].or(segment.start[2]).map(|z| z.value());
        match (z, self.current_z) {
            (Some(z), Some(current)) if (z - current).abs() > self.epsilon => {
                let end = self.last_extruding.map_or(index, |i| i + 1);
                let accum = std::mem::take(&mut self.current);
                let layer_index = self.layers.len();
                self.layers
                    .push(accum.finish(layer_index, current, self.current_start, end));
                self.current = std::mem::take(&mut self.pending);
                self.current_start = end;
                self.current_z = Some(z);
            }
            (Some(z), None) => {
                self.current_z = Some(z);
                self.current.merge(std::mem::take(&mut self.pending));
            }
            _ => self.current.merge(std::mem::take(&mut self.pending)),
        }
        self.current.add_segment(segment, timing, source_line);
        self.last_extruding = Some(index);
    }

    /// Close the last layer at `segment_count`, absorbing any epilogue.
    ///
    /// Yields no layers at all when no extruding segment carried a Z — there is nothing to key on, and
    /// inventing one layer covering everything would be a lie about a travel-only file.
    fn finish(mut self, segment_count: usize) -> Vec<LayerTraceLinkage> {
        if let Some(z) = self.current_z {
            self.current.merge(self.pending);
            let layer_index = self.layers.len();
            self.layers.push(self.current.finish(
                layer_index,
                z,
                self.current_start,
                segment_count,
            ));
        }
        self.layers
    }
}

/// The whole analytics pass's scratch state, accumulated inside the loop that already exists.
#[derive(Debug)]
struct AnalyticsAccum {
    print: PhaseAccum,
    travel: PhaseAccum,
    layers: LayerPartition,
}

impl AnalyticsAccum {
    fn new(options: &TraceAnalyticsOptions) -> Self {
        AnalyticsAccum {
            print: PhaseAccum::default(),
            travel: PhaseAccum::default(),
            layers: LayerPartition::new(options.layer_z_epsilon_mm),
        }
    }

    fn observe(
        &mut self,
        index: usize,
        segment: &Segment,
        timing: SegmentTiming,
        source_line: Option<usize>,
    ) {
        if timing.motion_s > 0.0 {
            let phase = if segment.travel {
                &mut self.travel
            } else {
                &mut self.print
            };
            phase.add(
                segment.speed.value(),
                timing.flow_mm3_s,
                timing.motion_s,
                segment.length.value(),
                segment.volume.value(),
            );
        }
        self.layers.observe(index, segment, timing, source_line);
    }
}

/// Order statistics over the per-layer aggregates. `None` when there is no layer, or when no layer
/// carries a finite print time and volume.
fn layer_stats(layers: &[LayerTraceLinkage]) -> Option<LayerStats> {
    if layers.is_empty() {
        return None;
    }
    let print_time_s = percentiles_unweighted(layers.iter().map(|l| l.print_time_s))?;
    let extruded_volume_mm3 = percentiles_unweighted(layers.iter().map(|l| l.extruded_volume_mm3))?;
    let mut slowest_layer_index = 0;
    for (index, layer) in layers.iter().enumerate() {
        // strict `>`, so ties resolve to the lowest index.
        if layer.print_time_s > layers[slowest_layer_index].print_time_s {
            slowest_layer_index = index;
        }
    }
    Some(LayerStats {
        layer_count: layers.len(),
        print_time_s,
        extruded_volume_mm3,
        slowest_layer_index,
    })
}

/// The two O(W) passes over the window vector — one to take the peak percentiles, one to flag the
/// outliers against the p50 the first pass published.
fn build_analytics(
    summary: &TraceSummary,
    print: PhaseStats,
    travel: PhaseStats,
    options: &TraceAnalyticsOptions,
) -> TraceAnalytics {
    let considered: Vec<&TraceWindow> = summary
        .windows
        .iter()
        .filter(|w| w.duration_s > 0.0)
        .collect();
    let window_flow_mm3_s = percentiles_unweighted(considered.iter().map(|w| w.max_flow_mm3_s));
    let window_feedrate_mm_min =
        percentiles_unweighted(considered.iter().map(|w| w.max_feedrate_mm_min));
    let threshold_mm3_s = window_flow_mm3_s.map(|p| options.flow_outlier_k * p.p50);
    // Flagged over the same population the reference came from: mixing populations would be the bug.
    let window_indices = match threshold_mm3_s {
        Some(threshold) => considered
            .iter()
            .filter(|w| w.max_flow_mm3_s > threshold)
            .map(|w| w.index)
            .collect(),
        None => Vec::new(),
    };
    let motion_s = print.time_s + travel.time_s;
    let travel_time_ratio = if motion_s > 0.0 {
        Some(travel.time_s / motion_s)
    } else {
        None
    };
    let segments_considered = print.segments + travel.segments;
    TraceAnalytics {
        print,
        travel,
        window_flow_mm3_s,
        window_feedrate_mm_min,
        flow_outliers: WindowOutliers {
            k: options.flow_outlier_k,
            threshold_mm3_s,
            window_indices,
        },
        layer_stats: layer_stats(&summary.layers),
        travel_time_ratio,
        windows_considered: considered.len(),
        segments_considered,
    }
}

fn timing(segment: &Segment) -> SegmentTiming {
    let motion_s = segment_motion_time(segment)
        .map(|time| time.value())
        .unwrap_or(0.0);
    let dwell_s = segment.dwell_s.unwrap_or(0.0).max(0.0);
    let flow_mm3_s = if motion_s > 0.0 {
        segment.volume.value() / motion_s
    } else {
        0.0
    };
    SegmentTiming {
        motion_s,
        dwell_s,
        flow_mm3_s,
    }
}

fn ensure_window(windows: &mut Vec<TraceWindow>, index: usize, window_s: f64) -> &mut TraceWindow {
    while windows.len() <= index {
        let next = windows.len();
        windows.push(TraceWindow::new(next, window_s));
    }
    &mut windows[index]
}

fn add_zero_duration_segment(
    summary: &mut TraceSummary,
    segment_index: usize,
    source_line: Option<usize>,
    cursor_s: f64,
) {
    let boundary = (cursor_s / summary.window_s).round();
    let on_boundary = cursor_s > 0.0 && (cursor_s - boundary * summary.window_s).abs() < 1e-12;
    let index = if on_boundary {
        (boundary as usize).saturating_sub(1)
    } else {
        (cursor_s / summary.window_s).floor() as usize
    };
    let window = ensure_window(&mut summary.windows, index, summary.window_s);
    window.touch_segment(segment_index, source_line);
}

fn add_motion_component(
    summary: &mut TraceSummary,
    segment_index: usize,
    source_line: Option<usize>,
    segment: &Segment,
    cursor_s: f64,
    duration_s: f64,
    flow_mm3_s: f64,
) {
    let end_s = cursor_s + duration_s;
    let mut t = cursor_s;
    while t < end_s - 1e-12 {
        let index = (t / summary.window_s).floor() as usize;
        let window_end = ((index + 1) as f64 * summary.window_s).min(end_s);
        let overlap_s = (window_end - t).max(0.0);
        let fraction = overlap_s / duration_s;
        let window = ensure_window(&mut summary.windows, index, summary.window_s);
        window.touch_segment(segment_index, source_line);
        window.duration_s += overlap_s;
        if segment.travel {
            window.travel_time_s += overlap_s;
            window.travel_distance_mm += segment.length.value() * fraction;
        } else {
            window.print_time_s += overlap_s;
            window.extruding_distance_mm += segment.length.value() * fraction;
        }
        window.extruded_volume_mm3 += segment.volume.value() * fraction;
        window.filament_mm += segment.filament.value() * fraction;
        window.max_feedrate_mm_min = window.max_feedrate_mm_min.max(segment.speed.value());
        window.max_flow_mm3_s = window.max_flow_mm3_s.max(flow_mm3_s);
        t = window_end;
    }
}

fn add_dwell_component(
    summary: &mut TraceSummary,
    segment_index: usize,
    source_line: Option<usize>,
    cursor_s: f64,
    duration_s: f64,
) {
    let end_s = cursor_s + duration_s;
    let mut t = cursor_s;
    while t < end_s - 1e-12 {
        let index = (t / summary.window_s).floor() as usize;
        let window_end = ((index + 1) as f64 * summary.window_s).min(end_s);
        let overlap_s = (window_end - t).max(0.0);
        let window = ensure_window(&mut summary.windows, index, summary.window_s);
        window.touch_segment(segment_index, source_line);
        window.duration_s += overlap_s;
        window.dwell_time_s += overlap_s;
        t = window_end;
    }
}

/// Summarize a toolpath into fixed-duration windows.
pub fn trace_summary(tp: &Toolpath, window_s: f64) -> Result<TraceSummary, TraceError> {
    trace_summary_with_sources(tp, window_s, &[])
}

/// Summarize a toolpath into fixed-duration windows, carrying optional source-line numbers per segment.
pub fn trace_summary_with_sources(
    tp: &Toolpath,
    window_s: f64,
    source_lines: &[Option<usize>],
) -> Result<TraceSummary, TraceError> {
    trace_summary_core(tp, window_s, source_lines, None)
}

/// Summarize a toolpath into fixed-duration windows, also computing the layer linkage
/// ([`TraceSummary::layers`]) and the higher-level statistics ([`TraceSummary::analytics`]).
///
/// Everything the other two entry points report is computed identically here — the analytics ride
/// along in the same segment pass and change no pre-existing number. The cost is one scratch ledger
/// proportional to the moving segment count (a fraction of the [`Toolpath`] the caller is already
/// holding) plus two O(W) passes over the window vector; the pass stays in the O(N) memory class the
/// materialised-toolpath entry points are already in, which is why there is no streaming counterpart.
///
/// Exact, deterministic, and free of transcendentals: see the [module docs](self) for the percentile
/// definition and the determinism argument.
pub fn trace_summary_with_analytics(
    tp: &Toolpath,
    window_s: f64,
    source_lines: &[Option<usize>],
    options: &TraceAnalyticsOptions,
) -> Result<TraceSummary, TraceError> {
    trace_summary_core(tp, window_s, source_lines, Some(options))
}

fn trace_summary_core(
    tp: &Toolpath,
    window_s: f64,
    source_lines: &[Option<usize>],
    options: Option<&TraceAnalyticsOptions>,
) -> Result<TraceSummary, TraceError> {
    validate_window(window_s)?;
    if let Some(options) = options {
        validate_analytics_options(options)?;
    }
    let mut analytics = options.map(AnalyticsAccum::new);
    let mut summary = TraceSummary::new(window_s, tp.segments.len());
    let mut cursor_s = 0.0;

    for (segment_index, segment) in tp.segments.iter().enumerate() {
        let source_line = source_lines.get(segment_index).copied().flatten();
        let timing = timing(segment);
        if let Some(accum) = analytics.as_mut() {
            accum.observe(segment_index, segment, timing, source_line);
        }
        let duration_s = timing.motion_s + timing.dwell_s;
        if duration_s == 0.0 {
            add_zero_duration_segment(&mut summary, segment_index, source_line, cursor_s);
            continue;
        }

        if timing.motion_s > 0.0 {
            summary.moving_segment_count += 1;
            summary.total_time_s += timing.motion_s;
            if segment.travel {
                summary.travel_time_s += timing.motion_s;
                summary.travel_distance_mm += segment.length.value();
            } else {
                summary.print_time_s += timing.motion_s;
                summary.extruding_distance_mm += segment.length.value();
            }
            summary.extruded_volume_mm3 += segment.volume.value();
            summary.filament_mm += segment.filament.value();
            summary.max_feedrate_mm_min = summary.max_feedrate_mm_min.max(segment.speed.value());
            summary.max_flow_mm3_s = summary.max_flow_mm3_s.max(timing.flow_mm3_s);
            add_motion_component(
                &mut summary,
                segment_index,
                source_line,
                segment,
                cursor_s,
                timing.motion_s,
                timing.flow_mm3_s,
            );
            cursor_s += timing.motion_s;
        }

        if timing.dwell_s > 0.0 {
            summary.total_time_s += timing.dwell_s;
            summary.dwell_time_s += timing.dwell_s;
            add_dwell_component(
                &mut summary,
                segment_index,
                source_line,
                cursor_s,
                timing.dwell_s,
            );
            cursor_s += timing.dwell_s;
        }
    }

    if let (Some(accum), Some(options)) = (analytics, options) {
        let print = accum.print.finish();
        let travel = accum.travel.finish();
        summary.layers = accum.layers.finish(summary.segment_count);
        summary.analytics = Some(build_analytics(&summary, print, travel, options));
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Segment, SegmentKind};
    use crate::units::{Feedrate, Length, Volume};

    fn segment(length: f64, speed: f64, travel: bool, volume: f64) -> Segment {
        Segment {
            start: [
                Some(Length::mm(0.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            end: [
                Some(Length::mm(length)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            travel,
            speed: Feedrate(speed),
            length: Length::mm(length),
            volume: Volume(volume),
            filament: Length::mm(volume / 2.4),
            width: Some(Length::mm(0.45)),
            height: Some(Length::mm(0.2)),
            kind: SegmentKind::Line,
            centre: None,
            clockwise: false,
            temperature: None,
            fan: None,
            flow: None,
            tool: None,
            power: None,
            dwell_s: None,
            manual_gcode: None,
            orientation: None,
            control_points: None,
        }
    }

    #[test]
    fn splits_long_segments_across_windows() {
        let tp = Toolpath {
            version: 0,
            meta: None,
            segments: vec![segment(100.0, 600.0, false, 12.0)],
        };
        let summary = trace_summary_with_sources(&tp, 5.0, &[Some(42)]).unwrap();
        assert_eq!(summary.windows.len(), 2);
        assert!((summary.total_time_s - 10.0).abs() < 1e-12);
        assert!((summary.extruded_volume_mm3 - 12.0).abs() < 1e-12);
        assert!((summary.windows[0].extruded_volume_mm3 - 6.0).abs() < 1e-12);
        assert_eq!(summary.windows[0].source_line_start, Some(42));
        assert_eq!(summary.windows[1].source_line_end, Some(42));
    }

    #[test]
    fn rejects_bad_window_duration() {
        let tp = Toolpath {
            version: 0,
            meta: None,
            segments: vec![],
        };
        assert!(trace_summary(&tp, 0.0).is_err());
    }

    // ---------------------------------------------------------------------------------------------
    // Analytics: the percentile definition (module docs), hand-computed.
    // ---------------------------------------------------------------------------------------------

    fn ws(pairs: &[(f64, f64)]) -> Vec<WeightedSample> {
        pairs
            .iter()
            .map(|&(value, weight)| WeightedSample { value, weight })
            .collect()
    }

    #[test]
    fn a_single_sample_is_all_four_order_statistics() {
        let p = percentiles(&mut ws(&[(5.0, 2.0)])).unwrap();
        assert_eq!((p.min, p.p50, p.p95, p.max), (5.0, 5.0, 5.0, 5.0));
    }

    #[test]
    fn p50_of_two_equal_weights_is_the_lower_value() {
        // Nearest-rank, lower: target = 0.5 * 2 = 1, and the first sample's cumulative weight is
        // already 1 — so the answer is a value that actually occurred, never their average.
        let p = percentiles(&mut ws(&[(3.0, 1.0), (1.0, 1.0)])).unwrap();
        assert_eq!((p.min, p.p50, p.p95, p.max), (1.0, 1.0, 3.0, 3.0));
    }

    #[test]
    fn weighting_moves_the_median_off_the_count_median() {
        // 1 s at 1, 1 s at 2, 8 s at 3. By count the median is 2; by time it is 3 — "for half the
        // print time, the value was at or below 3". The weighting is real, not decorative.
        let time_weighted = percentiles(&mut ws(&[(1.0, 1.0), (2.0, 1.0), (3.0, 8.0)])).unwrap();
        let by_count = percentiles(&mut ws(&[(1.0, 1.0), (2.0, 1.0), (3.0, 1.0)])).unwrap();
        assert_eq!(time_weighted.p50, 3.0);
        assert_eq!(by_count.p50, 2.0);
    }

    #[test]
    fn p95_of_twenty_unit_samples_is_the_nineteenth() {
        let mut samples = ws(&[]);
        for i in 1..=20 {
            samples.push(WeightedSample {
                value: i as f64,
                weight: 1.0,
            });
        }
        let p = percentiles(&mut samples).unwrap();
        assert_eq!((p.min, p.p50, p.p95, p.max), (1.0, 10.0, 19.0, 20.0));
    }

    #[test]
    fn a_population_with_no_weight_has_no_percentiles() {
        assert!(percentiles(&mut ws(&[])).is_none());
        // `0.0` would be indistinguishable from a real stall.
        assert!(percentiles(&mut ws(&[(1.0, 0.0)])).is_none());
    }

    #[test]
    fn tie_groups_are_order_independent() {
        // A stable sort keeps a tie group's input order, so the partial sums inside it are fixed and
        // the output is a function of the population alone.
        let a = percentiles(&mut ws(&[(1.0, 1.0), (1.0, 2.0), (1.0, 3.0), (2.0, 4.0)])).unwrap();
        let b = percentiles(&mut ws(&[(1.0, 3.0), (1.0, 1.0), (1.0, 2.0), (2.0, 4.0)])).unwrap();
        assert_eq!(a, b);
    }

    // ---------------------------------------------------------------------------------------------
    // Analytics: phases, layers, windows.
    // ---------------------------------------------------------------------------------------------

    /// `segment()` at an explicit Z.
    fn segment_at_z(length: f64, speed: f64, travel: bool, volume: f64, z: f64) -> Segment {
        let mut s = segment(length, speed, travel, volume);
        s.start[2] = Some(Length::mm(z));
        s.end[2] = Some(Length::mm(z));
        s
    }

    fn tp(segments: Vec<Segment>) -> Toolpath {
        Toolpath {
            version: 0,
            meta: None,
            segments,
        }
    }

    fn analytics_of(tp: &Toolpath, window_s: f64) -> TraceSummary {
        trace_summary_with_analytics(tp, window_s, &[], &TraceAnalyticsOptions::default()).unwrap()
    }

    #[test]
    fn rejects_bad_analytics_options() {
        let tp = tp(vec![segment(10.0, 600.0, false, 1.2)]);
        for bad in [
            TraceAnalyticsOptions {
                flow_outlier_k: 0.0,
                ..Default::default()
            },
            TraceAnalyticsOptions {
                flow_outlier_k: f64::NAN,
                ..Default::default()
            },
            TraceAnalyticsOptions {
                layer_z_epsilon_mm: -1.0,
                ..Default::default()
            },
            TraceAnalyticsOptions {
                layer_z_epsilon_mm: f64::INFINITY,
                ..Default::default()
            },
        ] {
            assert!(trace_summary_with_analytics(&tp, 5.0, &[], &bad).is_err());
        }
    }

    #[test]
    fn phase_maxima_cross_check_the_summary_maxima() {
        // 100 mm at 600 mm/min extruding (12 mm³ over 10 s → 1.2 mm³/s), then a fast travel.
        let summary = analytics_of(
            &tp(vec![
                segment(100.0, 600.0, false, 12.0),
                segment(50.0, 6000.0, true, 0.0),
                segment(20.0, 1200.0, false, 6.0),
            ]),
            5.0,
        );
        let a = summary.analytics.as_ref().unwrap();
        let print = a.print.feedrate_mm_min.unwrap();
        let travel = a.travel.feedrate_mm_min.unwrap();
        assert_eq!(
            print.max.max(travel.max),
            summary.max_feedrate_mm_min,
            "the ledger population is exactly the population the summary maxima fold over"
        );
        let print_flow = a.print.flow_mm3_s.unwrap();
        let travel_flow = a.travel.flow_mm3_s.unwrap();
        assert_eq!(print_flow.max.max(travel_flow.max), summary.max_flow_mm3_s);

        // A travel deposits nothing, so its flow ledger is identically zero — a non-zero value here is
        // the `travel-extrudes` smell, corroborated from an independent computation.
        assert_eq!((travel_flow.min, travel_flow.max), (0.0, 0.0));
        assert_eq!(a.print.segments, 2);
        assert_eq!(a.travel.segments, 1);
        assert_eq!(a.segments_considered, summary.moving_segment_count);

        // Σ(v·t)/Σt: 10 s at 600 and 1 s at 1200 → 654.5454…
        let mean = a.print.mean_feedrate_mm_min.unwrap();
        assert!((mean - (600.0 * 10.0 + 1200.0 * 1.0) / 11.0).abs() < 1e-12);
        // travel motion 0.5 s of 11.5 s total motion.
        assert!((a.travel_time_ratio.unwrap() - 0.5 / 11.5).abs() < 1e-12);
    }

    #[test]
    fn nonfinite_samples_are_excluded_and_counted() {
        // A non-finite *feedrate* can never reach the ledger: `segment_motion_time` refuses it, so the
        // segment has no motion time and is not in the phase population at all. A non-finite *flow*
        // can — volume/motion_s with a non-finite volume — so that is what this probe uses.
        let nonfinite_volume = |volume: f64| Segment {
            // set past the constructor: `Length::mm` refuses a non-finite value, and the feedstock
            // length is not what this probe is about.
            volume: Volume(volume),
            ..segment(100.0, 600.0, false, 12.0)
        };
        let summary = analytics_of(
            &tp(vec![
                segment(100.0, 600.0, false, 12.0),
                nonfinite_volume(f64::NAN),
                nonfinite_volume(f64::INFINITY),
            ]),
            5.0,
        );
        let print = &summary.analytics.as_ref().unwrap().print;
        assert_eq!(print.segments, 3);
        assert_eq!(print.nonfinite_samples, 2);
        let flow = print.flow_mm3_s.unwrap();
        assert!(flow.min.is_finite() && flow.max.is_finite());
        assert_eq!(
            (flow.min, flow.p50, flow.p95, flow.max),
            (1.2, 1.2, 1.2, 1.2)
        );
        // Feedrates are all finite and unaffected.
        assert!(print.feedrate_mm_min.unwrap().max.is_finite());
        // Totals and time-weighted means are *not* filtered — they keep the engine's behaviour, which
        // is why the §3.2 cross-check invariant is not asserted for a case like this one.
        assert!(!print.mean_flow_mm3_s.unwrap().is_finite());
    }

    /// Three monotonic layers, each entered over a lift travel.
    fn three_layer_path() -> Toolpath {
        tp(vec![
            segment_at_z(10.0, 600.0, false, 1.2, 0.2),
            segment_at_z(5.0, 6000.0, true, 0.0, 0.4),
            segment_at_z(10.0, 600.0, false, 1.2, 0.4),
            segment_at_z(5.0, 6000.0, true, 0.0, 0.6),
            segment_at_z(10.0, 600.0, false, 1.2, 0.6),
        ])
    }

    #[test]
    fn layers_partition_the_segment_range() {
        let summary = analytics_of(&three_layer_path(), 5.0);
        let layers = &summary.layers;
        assert_eq!(layers.len(), 3);
        assert_eq!(
            layers[0].segment_start, 0,
            "a prologue is attributed, not orphaned"
        );
        for pair in layers.windows(2) {
            assert_eq!(pair[0].segment_end, pair[1].segment_start);
        }
        assert_eq!(layers.last().unwrap().segment_end, summary.segment_count);
        for (index, layer) in layers.iter().enumerate() {
            assert_eq!(layer.layer_index, index);
        }
        assert_eq!(
            layers.iter().map(|l| l.z_mm).collect::<Vec<_>>(),
            vec![0.2, 0.4, 0.6]
        );

        // Grouping changes the summation order and f64 addition is not associative, so the sum
        // invariant is relative, not bit-exact.
        let print: f64 = layers.iter().map(|l| l.print_time_s).sum();
        assert!((print - summary.print_time_s).abs() <= 1e-9 * summary.print_time_s.abs());
        let travel: f64 = layers.iter().map(|l| l.travel_time_s).sum();
        assert!((travel - summary.travel_time_s).abs() <= 1e-9 * summary.travel_time_s.abs());
    }

    #[test]
    fn the_lift_before_a_break_lands_in_the_layer_it_enters() {
        let summary = analytics_of(&three_layer_path(), 5.0);
        // Segment 1 is the lift to Z0.4: it belongs to layer 1, not to the layer it left.
        assert_eq!(summary.layers[0].segment_end, 1);
        assert_eq!(summary.layers[1].segment_start, 1);
        assert_eq!(summary.layers[0].travel_time_s, 0.0);
        assert!(summary.layers[1].travel_time_s > 0.0);
    }

    #[test]
    fn a_mid_layer_z_hop_does_not_split_a_layer() {
        // Extrude, hop up, hop back down, extrude — all at the same extruding Z. Keying breaks on
        // extruding Z is what keeps a single hop from shattering one layer into three.
        let summary = analytics_of(
            &tp(vec![
                segment_at_z(10.0, 600.0, false, 1.2, 0.2),
                segment_at_z(1.0, 6000.0, true, 0.0, 0.6),
                segment_at_z(1.0, 6000.0, true, 0.0, 0.2),
                segment_at_z(10.0, 600.0, false, 1.2, 0.2),
            ]),
            5.0,
        );
        assert_eq!(summary.layers.len(), 1);
        assert_eq!(summary.layers[0].segment_start, 0);
        assert_eq!(summary.layers[0].segment_end, 4);
    }

    #[test]
    fn a_revisited_z_is_a_second_layer_because_layers_are_passes() {
        let summary = analytics_of(
            &tp(vec![
                segment_at_z(10.0, 600.0, false, 1.2, 0.2),
                segment_at_z(5.0, 6000.0, true, 0.0, 0.4),
                segment_at_z(10.0, 600.0, false, 1.2, 0.4),
                segment_at_z(5.0, 6000.0, true, 0.0, 0.2),
                segment_at_z(10.0, 600.0, false, 1.2, 0.2),
            ]),
            5.0,
        );
        assert_eq!(
            summary.layers.iter().map(|l| l.z_mm).collect::<Vec<_>>(),
            vec![0.2, 0.4, 0.2],
            "three passes over two distinct levels"
        );
    }

    #[test]
    fn a_travel_only_path_has_no_layers() {
        let summary = analytics_of(&tp(vec![segment(50.0, 6000.0, true, 0.0)]), 5.0);
        assert!(summary.layers.is_empty(), "there is no Z to key on");
        assert!(summary.analytics.as_ref().unwrap().layer_stats.is_none());
    }

    #[test]
    fn a_single_segment_path_is_one_layer() {
        let summary = analytics_of(&tp(vec![segment(10.0, 600.0, false, 1.2)]), 5.0);
        assert_eq!(summary.layers.len(), 1);
        assert_eq!(summary.layers[0].segment_start, 0);
        assert_eq!(summary.layers[0].segment_end, 1);
    }

    #[test]
    fn layer_stats_name_the_slowest_layer_and_resolve_ties_low() {
        // Layer 1 is twice the length, so twice the print time.
        let summary = analytics_of(
            &tp(vec![
                segment_at_z(10.0, 600.0, false, 1.2, 0.2),
                segment_at_z(20.0, 600.0, false, 2.4, 0.4),
                segment_at_z(10.0, 600.0, false, 1.2, 0.6),
            ]),
            5.0,
        );
        let stats = summary
            .analytics
            .as_ref()
            .unwrap()
            .layer_stats
            .clone()
            .unwrap();
        assert_eq!(stats.layer_count, 3);
        assert_eq!(stats.slowest_layer_index, 1);
        assert_eq!(stats.print_time_s.max, 2.0);
        assert_eq!(stats.print_time_s.p50, 1.0);

        // Three equal layers: the tie resolves to the lowest index.
        let flat = analytics_of(
            &tp(vec![
                segment_at_z(10.0, 600.0, false, 1.2, 0.2),
                segment_at_z(10.0, 600.0, false, 1.2, 0.4),
                segment_at_z(10.0, 600.0, false, 1.2, 0.6),
            ]),
            5.0,
        );
        let stats = flat
            .analytics
            .as_ref()
            .unwrap()
            .layer_stats
            .clone()
            .unwrap();
        assert_eq!(stats.slowest_layer_index, 0);
    }

    #[test]
    fn constant_flow_flags_no_outlier_at_k_one() {
        let summary = trace_summary_with_analytics(
            // 100 mm at 600 mm/min = 10 s at a constant 1.2 mm³/s → 10 one-second windows.
            &tp(vec![segment(100.0, 600.0, false, 12.0)]),
            1.0,
            &[],
            &TraceAnalyticsOptions {
                flow_outlier_k: 1.0,
                ..Default::default()
            },
        )
        .unwrap();
        let a = summary.analytics.as_ref().unwrap();
        assert_eq!(a.windows_considered, 10);
        assert!(
            a.flow_outliers.window_indices.is_empty(),
            "strict `>` so k = 1 on a constant-flow file flags nothing"
        );
        assert_eq!(a.flow_outliers.k, 1.0);
        assert_eq!(
            a.flow_outliers.threshold_mm3_s.unwrap(),
            1.0 * a.window_flow_mm3_s.unwrap().p50,
            "the threshold is reproducible from two numbers in the same document"
        );
    }

    #[test]
    fn a_five_times_window_is_flagged_at_k_two() {
        // Three 1 s windows at 1.2 mm³/s, then one at 6.0 mm³/s.
        let summary = analytics_of(
            &tp(vec![
                segment(30.0, 600.0, false, 3.6),
                segment(10.0, 600.0, false, 6.0),
            ]),
            1.0,
        );
        let a = summary.analytics.as_ref().unwrap();
        assert_eq!(a.windows_considered, 4);
        let peaks = a.window_flow_mm3_s.unwrap();
        assert_eq!(peaks.p50, 1.2);
        assert_eq!(a.flow_outliers.threshold_mm3_s.unwrap(), 2.0 * 1.2);
        assert_eq!(a.flow_outliers.window_indices, vec![3]);
    }

    #[test]
    fn zero_duration_windows_are_excluded_from_the_window_population() {
        let mut zero = segment(0.0, 600.0, true, 0.0);
        zero.end = zero.start;
        let summary = analytics_of(&tp(vec![segment(10.0, 600.0, false, 1.2), zero]), 5.0);
        let a = summary.analytics.as_ref().unwrap();
        assert_eq!(summary.windows.len(), 1);
        assert_eq!(a.windows_considered, 1);
    }

    #[test]
    fn the_analytics_pass_changes_no_pre_existing_number() {
        let tp = three_layer_path();
        let sources: Vec<Option<usize>> = (0..tp.segments.len()).map(|i| Some(i + 10)).collect();
        let plain = trace_summary_with_sources(&tp, 1.0, &sources).unwrap();
        let mut with =
            trace_summary_with_analytics(&tp, 1.0, &sources, &TraceAnalyticsOptions::default())
                .unwrap();
        assert!(with.analytics.is_some() && !with.layers.is_empty());
        with.analytics = None;
        with.layers.clear();
        assert_eq!(plain, with);
    }

    #[test]
    fn the_layers_csv_is_a_second_relation_at_the_layer_grain() {
        let summary = analytics_of(&three_layer_path(), 5.0);
        let csv = summary.layers_to_csv();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 4, "header plus one row per layer");
        assert!(lines[0].starts_with("layer_index,z_mm,segment_start,segment_end,"));
        assert_eq!(lines[0].split(',').count(), 13);
        for row in &lines[1..] {
            assert_eq!(row.split(',').count(), 13);
        }
        // Without analytics the relation is empty, header and all.
        let plain = trace_summary(&three_layer_path(), 5.0).unwrap();
        assert_eq!(plain.layers_to_csv(), format!("{}\n", lines[0]));
    }

    #[test]
    fn zero_duration_segment_on_boundary_does_not_create_empty_trailing_window() {
        let mut zero = segment(0.0, 600.0, true, 0.0);
        zero.end = zero.start;
        let tp = Toolpath {
            version: 0,
            meta: None,
            segments: vec![segment(100.0, 600.0, false, 12.0), zero],
        };

        let summary = trace_summary_with_sources(&tp, 5.0, &[Some(10), Some(11)]).unwrap();
        assert_eq!(summary.windows.len(), 2);
        assert_eq!(summary.windows[1].source_line_end, Some(11));
        assert_eq!(summary.windows[1].segment_end, Some(2));
    }
}

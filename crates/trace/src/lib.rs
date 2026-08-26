//! # drymachina-trace — DRYMACHINA layer 3: the analysis layer
//!
//! What the engine says *about* a toolpath rather than what it does to one: windowed motion/time
//! statistics ([`trace`]), the typed report envelopes the CLI and SDK emit ([`report`]), slicer
//! inference from imported G-code ([`forensics`]), a deterministic two-file diff ([`compare`]), the
//! LLM-ready explanation bundle ([`explain`]), the classify/execute half of the recommendation loop
//! ([`recommend`]), and the L2→L1 reversing pass ([`reverse`]).
//!
//! It reads layer 1 from [`drymachina_kernel`], layer 2 from [`drymachina_verify`] and the shared rule and
//! contract vocabulary from [`drymachina_contracts`]. **Nothing depends on this crate** except the
//! `dry-core` facade, which re-exports every module and name below unchanged — which is exactly why
//! it is the layer that can graduate to its own repository first (plan Task 8).
//!
//! Extracted verbatim from `dry-core` (plan Task 6), after which that crate holds no implementation
//! of its own.

#![forbid(unsafe_code)]

pub mod compare;
pub mod explain;
pub mod forensics;
pub mod recommend;
pub mod report;
pub mod reverse;
pub mod trace;

pub use compare::{
    compare_reports, render_markdown as render_compare_markdown, CompareDelta, FindingsDelta,
    ScalarDelta, SettingChange, StringChange, TimeDelta,
};
pub use explain::{build_explain_bundle, render_markdown, ExplainBundle, ExplainReports};
pub use forensics::{
    analyze as forensics_analyze, Confidence, DeclaredSettings, Estimate, FeatureStat,
    ForensicsReport, Hotspot, LayerModel, SeamHint, TravelStat, TravelStrategy,
};
pub use recommend::{
    apply_executable, classify, ActionKind, Classified, ContractField, ContractOverride,
    ExecutableAction, ExecutionResult, MetricSnapshot, Recommendation, Verdict,
};
pub use report::{
    BatchFileResult, BatchStatus, LicenseStamp, LocatedFinding, ReviewBatch, ReviewReport,
    RewriteReport, RewriteSpanResult, RuleTally, TraceReport,
};
pub use reverse::{reverse, ReverseError};
pub use trace::{
    trace_summary, trace_summary_with_analytics, trace_summary_with_sources, LayerStats,
    LayerTraceLinkage, Percentiles, PhaseStats, TraceAnalytics, TraceAnalyticsOptions, TraceError,
    TraceSummary, TraceWindow, WindowOutliers,
};

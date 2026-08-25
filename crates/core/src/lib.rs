//! # dry-core — the Dry IR + engine (foundations)
//!
//! The dependency-light core of Dry (no PyO3, no numpy), the seed of the architecture in
//! `docs/01-architecture.md`. At Phase 0 it carries the L2 motion dialect (`ir`) and the first engine
//! analysis (`simulate`), validated against the FullControl behavioural oracle (`docs/03-conformance.md`)
//! — clean-room: Dry reproduces FullControl's *outputs*, never its code (`docs/CLEANROOM.md`).
//!
//! Status: **P0** — `resolve` + `simulate` + Marlin `emit`, all gated byte-for-output against the
//! FullControl oracle, over a **unit-typed IR** ([`units`]: mixing units is a compile error). The binary
//! encoding and the lowering passes are the next P0/P1 increments (`docs/04-tasks.md`).
//!
//! **Layering.** Layer 1 — the IR, resolve, lowering, optimisation, generation and emission — now
//! lives in `kmet-kernel`, and every one of its modules and names is re-exported below unchanged, so
//! a `dry_core::Toolpath` or `dry_core::emit::emit` import resolves exactly as it always did. The
//! verifier and the analysis layer are still defined here; they follow in plan Tasks 5 and 6, after
//! which this crate is a facade over four (`docs/superpowers/plans`).

#![forbid(unsafe_code)]

pub mod compare;
pub mod explain;
pub mod forensics;
mod gated;
pub mod recommend;
pub mod report;
pub mod reverse;
pub mod trace;

pub mod verify;

// Layer 1, re-exported module-for-module from `kmet-kernel` so that `dry_core::<module>::<item>`
// paths keep resolving for the CLI, the bindings and the tests.
pub use kmet_kernel::{
    clothoid, codec, engine, features, frame, gcode, generate, ir, optimize, profile, sdk, units,
};
// `emit` and `resolve` each name a module *and* a function of the same name, and one `use` of the
// name carries both namespaces — so, unlike the twelve above, these two also re-export the function
// and the flat lists below must not restate it.
#[allow(deprecated)]
pub use kmet_kernel::emit;
pub use kmet_kernel::resolve;

pub use clothoid::{corner_blend, fresnel, ClothoidError, CornerBlend, FRESNEL_SERIES_EPSILON};
pub use codec::{
    decode_any_streaming, decode_any_streaming_with_limits, decode_chunked_streaming,
    decode_chunked_streaming_with_limits, decode_streaming, decode_streaming_with_limits,
    decode_with_limits, encode_chunked, export_3mf_xml, import_3mf_xml, BinarySegmentsIterator,
    ChunkedSegmentsIterator, CodecError, DecodeLimits, JsonSegmentsIterator, SegmentStream,
    StreamingDecode, ThreeMfError,
};

pub use compare::{
    compare_reports, render_markdown as render_compare_markdown, CompareDelta, FindingsDelta,
    ScalarDelta, SettingChange, StringChange, TimeDelta,
};
pub use emit::{
    emit_step_nc, emit_stream, emit_stream_to_writer, CncFrame, EmitParams, FirmwareFlavor,
    Kinematics, KrlFrame, KrlTransform, REFERENCE_FIVE_AXIS_LIMITS, REFERENCE_FIVE_AXIS_MACHINE,
};
pub use engine::{simulate, simulate_stream, Metrics};
pub use explain::{build_explain_bundle, render_markdown, ExplainBundle, ExplainReports};
pub use features::{
    expand_features, expand_features_with_limits, ExpandError, ExpandLimits, FeatureNode,
    FeaturePose, FeatureProgram, DEFAULT_MAX_EXPANDED_NODES, DEFAULT_MAX_EXPANDED_OPS,
    DEFAULT_MAX_FEATURE_DEPTH,
};
pub use forensics::{
    analyze as forensics_analyze, Confidence, DeclaredSettings, Estimate, FeatureStat,
    ForensicsReport, Hotspot, LayerModel, SeamHint, TravelStat, TravelStrategy,
};
pub use frame::{FrameError, FrameGraph, TransformSE3};
// The verification-gated rewrite wrappers: kernel mechanism, verifier policy, so they cannot live in
// `kmet-kernel`. They move to `kmet-verify` at plan Task 5; the names re-exported here do not change.
pub use gated::{apply_gated, apply_safe_gated};
pub use gcode::{
    import_gcode, import_gcode_reader, import_gcode_reader_with_map, import_gcode_with_map,
    import_parsed_gcode, import_parsed_gcode_with_map, parse_gcode_lines, DistanceMode,
    ExtrusionMode, GcodeImportError, GcodeImportParams, GcodeModalState, GcodeMotionSpan,
    GcodeParseError, GcodeParser, GcodeRecord, GcodeWord, ImportedGcode, MotionMode, MotionRecord,
    ParsedGcodeLine, ProcessCommand, StateCommand, UnitMode,
};
pub use generate::{
    pocket_design, pocket_ops, tpms_design, tpms_ops, try_pocket_design, try_pocket_ops,
    try_tpms_design, try_tpms_ops, CutMode, PocketError, PocketOptions, PocketShape, Surface,
    TpmsError, TpmsOptions,
};
pub use ir::{Meta, Segment, SegmentKind, Toolpath};
pub use optimize::{
    adaptive_speed, adaptive_speed_with_kinematics, adaptive_speed_with_params, apply_gated_with,
    arc_fit, balanced_pipeline, coasting, coasting_with_dist, max_pipeline, merge_collinear,
    optimize_aggressive_pipeline, optimize_pipeline, safe_pipeline, travel_reorder, z_hop,
    z_hop_with_params, GatedResult, OptimizeMode,
};
pub use profile::{
    import_klipper, FirmwareProfile, KlipperImportError, KlipperImportWarning, MachineKinematics,
    MachineProfile, MachineRotary, MaterialProfile, ProcessProfile, Profile, ProfileError,
};
pub use recommend::{
    apply_executable, classify, ActionKind, Classified, ContractField, ContractOverride,
    ExecutableAction, ExecutionResult, MetricSnapshot, Recommendation, Verdict,
};
pub use report::{
    BatchFileResult, BatchStatus, LicenseStamp, LocatedFinding, ReviewBatch, ReviewReport,
    RewriteReport, RewriteSpanResult, RuleTally, TraceReport,
};
pub use resolve::{resolve_checked, validate_design, Design, Op, ResolveError, ResolveParams};
pub use reverse::{reverse, ReverseError};
pub use sdk::DesignBuilder;
pub use trace::{
    trace_summary, trace_summary_with_analytics, trace_summary_with_sources, LayerStats,
    LayerTraceLinkage, Percentiles, PhaseStats, TraceAnalytics, TraceAnalyticsOptions, TraceError,
    TraceSummary, TraceWindow, WindowOutliers,
};

pub use units::{Angle, Area, Feedrate, Flow, Length, Time, Volume};
pub use verify::{
    catalog, parse_bounds_csv, parse_speed_range_csv, verify, verify_stream, ContractParseError,
    Contracts, Finding, KinematicContracts, Report, RotaryContracts, RotaryTravelRanges, Rule,
    RuleId, Severity,
};

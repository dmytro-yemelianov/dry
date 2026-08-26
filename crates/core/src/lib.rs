//! # dry-core — one import surface over the four KMET crates
//!
//! A facade, and nothing else: it holds no types, no functions and no dependency of its own beyond
//! the three crates it re-exports; `kmet-contracts`, the fourth KMET crate, reaches callers through
//! `kmet-verify`'s re-export of the shared vocabulary rather than through a dependency declared
//! here. Six crates depend on it — `dry-cli`, `dry-llm`, and the four bindings `dry-wasm`,
//! `dry-cloud`, `dry-py` and `dry-verify-runner`, with `sdk/ts` reaching it through the wasm one —
//! and each of them names the engine's whole surface through this crate, under the paths it used
//! before the layers were separated (`docs/01-architecture.md`).
//!
//! What is true of the engine is still true through here: it is dependency-light (no PyO3, no
//! numpy), it compiles to `wasm32-unknown-unknown` unmodified, its IR is unit-typed ([`units`]:
//! mixing units is a compile error), and it is gated byte-for-output against the FullControl
//! behavioural oracle (`docs/03-conformance.md`) clean-room — Dry reproduces FullControl's
//! *outputs*, never its code (`docs/CLEANROOM.md`).
//!
//! **Layering.** Layer 1 — the IR, resolve, lowering, optimisation, generation and emission — lives
//! in `kmet-kernel`, layer 2 — the verification rule registry and the verify-gated rewrite — in
//! `kmet-verify`, and layer 3 — trace, report, forensics, compare, explain, recommend and reverse —
//! in `kmet-trace`. Every one of their modules and names is re-exported below unchanged, so a
//! `dry_core::Toolpath`, `dry_core::emit::emit`, `dry_core::verify::verify` or
//! `dry_core::trace::trace_summary` import resolves exactly as it always did
//! (`docs/superpowers/plans`).

#![forbid(unsafe_code)]

// Layer 3, re-exported module-for-module from `kmet-trace` so `dry_core::<module>::<item>` keeps
// resolving, exactly as the seven `pub mod` declarations that stood here did.
pub use kmet_trace::{compare, explain, forensics, recommend, report, trace};
// `reverse`, like `emit` and `resolve` below, names a module *and* a function of the same name; one
// `use` of the name carries both namespaces, so the flat list must not restate the function.
pub use kmet_trace::reverse;

// Layer 2, re-exported as a module so `dry_core::verify::<item>` keeps resolving; the flat list at
// the bottom of this file re-exports its names as well, exactly as `pub mod verify` did.
pub use kmet_verify as verify;

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
pub use reverse::ReverseError;
pub use sdk::DesignBuilder;
pub use trace::{
    trace_summary, trace_summary_with_analytics, trace_summary_with_sources, LayerStats,
    LayerTraceLinkage, Percentiles, PhaseStats, TraceAnalytics, TraceAnalyticsOptions, TraceError,
    TraceSummary, TraceWindow, WindowOutliers,
};

pub use units::{Angle, Area, Feedrate, Flow, Length, Time, Volume};
// `apply_gated` / `apply_safe_gated` are listed here rather than under `optimize`: the mechanism is
// the kernel's (`optimize::apply_gated_with`) but the policy is the verifier's, and the wrappers now
// live in `kmet-verify` beside it. So the flat `dry_core::apply_gated` every caller already uses is
// unchanged, and the module path is `dry_core::verify::apply_gated`. Restoring the pre-split
// `dry_core::optimize::apply_gated` would put a verifier-policy name back under a kernel module — the
// layering the split exists to remove — and no caller, in-tree or in a binding, used it.
pub use verify::{
    apply_gated, apply_safe_gated, catalog, parse_bounds_csv, parse_speed_range_csv, verify,
    verify_stream, ContractParseError, Contracts, Finding, KinematicContracts, Report,
    RotaryContracts, RotaryTravelRanges, Rule, RuleId, Severity,
};

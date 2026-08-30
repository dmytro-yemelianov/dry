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

#![forbid(unsafe_code)]

pub mod channel;
pub mod clothoid;
pub mod codec;
pub mod compare;
pub mod dexel;
pub mod document;
pub mod emit;
pub mod engine;
pub mod explain;
pub mod features;
pub mod forensics;
pub mod frame;
pub mod gcode;
pub mod generate;
pub mod ir;
pub mod multi_head;
pub mod multi_robot;
pub mod optimize;
pub mod pass;
pub mod pipeline;
pub mod profile;
pub mod provenance;
pub mod quality;
pub mod recommend;
pub mod report;
pub mod resolve;
pub mod reverse;
pub mod schema;
pub mod sdk;
pub mod step_nc;
pub mod tool;
pub mod trace;

pub mod units;
pub mod verify;

pub use channel::{ChannelDefinition, ChannelKind, ChannelRegistry, ChannelValue};
pub use clothoid::{corner_blend, fresnel, ClothoidError, CornerBlend, FRESNEL_SERIES_EPSILON};
pub use codec::{
    decode_any_streaming, decode_any_streaming_with_limits, decode_chunked_streaming,
    decode_chunked_streaming_with_limits, decode_dry2, decode_streaming,
    decode_streaming_with_limits, decode_with_limits, encode_chunked, encode_dry2, export_3mf_xml,
    import_3mf_xml, BinarySegmentsIterator, ChunkedSegmentsIterator, CodecError, DecodeLimits,
    JsonSegmentsIterator, SegmentStream, StreamingDecode, ThreeMfError, DRY2_MAGIC,
};

pub use compare::{
    compare_layer_traces, compare_reports, render_markdown as render_compare_markdown,
    CompareDelta, FindingsDelta, LayerTraceDelta, ScalarDelta, SettingChange, StringChange,
    TimeDelta,
};
pub use dexel::{DexelGrid, DexelSimulationReport};
pub use document::{Dialect, DocumentEnvelope, DocumentMetadata, DocumentValidationError};
#[allow(deprecated)]
pub use emit::emit;
pub use emit::{
    emit_cycle_cancel, emit_gcode_chunks, emit_grbl_laser, emit_plasma_waterjet, emit_step_nc,
    emit_stream, emit_stream_to_writer, render_template, CncFrame, CuttingParams, DrillCycle,
    EmitParams, FirmwareFlavor, GcodeTemplate, Kinematics, KrlFrame, KrlTransform, LaserError,
    LaserMode, LaserParams, LeadInType, PeckDrillCycle, TemplateContext,
    DhParam, RobotJoints6, Robot6AxisModel,
    REFERENCE_FIVE_AXIS_LIMITS, REFERENCE_FIVE_AXIS_MACHINE,
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
pub use frame::{FrameId, Quaternion, Transform3D};
pub use gcode::{
    import_gcode, import_gcode_reader, import_gcode_reader_with_map, import_gcode_with_map,
    import_parsed_gcode, import_parsed_gcode_with_map, parse_gcode_lines, DistanceMode,
    ExtrusionMode, GcodeImportError, GcodeImportParams, GcodeModalState, GcodeMotionSpan,
    GcodeParseError, GcodeParser, GcodeRecord, GcodeWord, ImportedGcode, MotionMode, MotionRecord,
    ParsedGcodeLine, ProcessCommand, StateCommand, UnitMode,
};
pub use generate::{
    drape_design, drape_ops, generate_chamfer_ops, generate_corner_rest_machining_ops,
    generate_lathe_facing_ops, generate_lathe_od_turning_ops, generate_thread_milling_ops,
    pocket_design, pocket_ops, pocket_stepped_ops, tpms_design, tpms_ops, try_pocket_design,
    try_pocket_ops, try_tpms_design, try_tpms_ops, Aabb, BrepError, BrepSolid, ChamferParams,
    CutMode, DrapeError, DrapeOptions, DrapePattern, LatheFacingParams, LatheTurningParams,
    PocketError, PocketOptions, PocketShape, Point3D, RestMachiningParams, Surface,
    SurfacePrimitive, ThreadMillParams, TpmsError, TpmsOptions, Triangle, TriangleMesh, Vector3D,
};
pub use ir::{Meta, Segment, SegmentKind, Toolpath};
pub use multi_head::{emit_idex_mode, emit_select_head, HeadConfig, HeadMode};
pub use multi_robot::{
    calculate_clearance_velocity_scale, check_continuous_dual_robot_trajectory,
    check_dual_robot_clearance, emit_dual_robot_sync_krl, emit_dual_robot_sync_rapid,
    interpolate_dual_robot_waypoint, segment_to_segment_distance_3d, DualRobotCollisionResult,
    DualRobotWaypoint, WorkcellRobot,
};
pub use optimize::{
    adaptive_speed, adaptive_speed_with_kinematics, adaptive_speed_with_params,
    apply_chip_thinning_compensation, apply_gated, apply_safe_gated, arc_fit, balanced_pipeline,
    calculate_chip_thinning_multiplier, calculate_scurve_profile, coasting, coasting_with_dist,
    generate_trochoidal_corner_peel, generate_trochoidal_slot, max_pipeline, merge_collinear,
    optimize_aggressive_pipeline, optimize_constant_mrr, optimize_corner_feedrate,
    optimize_pipeline, safe_pipeline, travel_reorder, z_hop, z_hop_with_params, GatedResult,
    OptimizeMode, SCurveParams, SCurveProfile,
};
pub use pass::PassRole;
pub use pipeline::{lower_document_envelope, PipelineError};
pub use profile::{
    check_compatibility, import_klipper, AxisRange, CompatibilityFinding, CompatibilityReport,
    FirmwareProfile, KlipperImportError, KlipperImportWarning, MachineCapabilities,
    MachineKinematics, MachineProfile, MachineRotary, MaterialProfile, ProcessProfile, Profile,
    ProfileError, Severity as CompatibilitySeverity,
};
pub use provenance::{NodeId, ProvenanceMap, SegmentSpan};
pub use quality::{
    calculate_cusp_height, calculate_mrr, estimate_cutting_power_kw, estimate_surface_roughness_ra,
    evaluate_mrr, evaluate_surface_quality, MrrReport, SurfaceQualityReport,
};
pub use recommend::{
    apply_executable, classify, ActionKind, Classified, ContractField, ContractOverride,
    ExecutableAction, ExecutionResult, MetricSnapshot, Recommendation, Verdict,
};
pub use report::{
    BatchFileResult, BatchStatus, LicenseStamp, LocatedFinding, ReviewBatch, ReviewReport,
    RewriteReport, RewriteSpanResult, RuleTally, TraceReport,
};
pub use resolve::{
    resolve, resolve_checked, validate_design, Design, Op, ResolveError, ResolveParams,
};
pub use reverse::{reverse, ReverseError};
pub use schema::get_dialect_schema;
pub use sdk::DesignBuilder;
pub use step_nc::{lower_workingstep_to_ops, parse_step_nc, StepNcFeature, StepNcWorkingstep};
pub use tool::{ToolDefinition, ToolKind, ToolRegistry};
pub use trace::{
    trace_summary, trace_summary_with_analytics, trace_summary_with_sources, LayerStats,
    LayerTraceLinkage, Percentiles, PhaseStats, TraceAnalytics, TraceAnalyticsOptions, TraceError,
    TraceSummary, TraceWindow, WindowOutliers,
};

pub use units::{Angle, Area, Feedrate, Flow, Length, Time, Volume};
pub use verify::{
    catalog, check_tool_holder_collision, parse_bounds_csv, parse_speed_range_csv, verify,
    verify_stream, CollisionFinding, ContractParseError, Contracts, Finding, KinematicContracts,
    Report, RotaryContracts, RotaryTravelRanges, Rule, RuleId, Severity, ToolHolder,
};

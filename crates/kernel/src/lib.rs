//! # kmet-kernel — KMET layer 1: the IR and the passes that produce it
//!
//! The dependency-light kernel of the engine (`docs/01-architecture.md`): the unit-typed L2 motion
//! dialect ([`ir`], [`units`]), the L1 authoring surface that lowers into it ([`resolve`],
//! [`features`], [`generate`], [`sdk`], [`frame`], [`clothoid`]), the IR→IR passes ([`optimize`]),
//! the encodings ([`codec`], [`gcode`]), the machine description it is all judged against
//! ([`profile`]), and the lowering to machine programs ([`emit`], [`engine`]).
//!
//! It carries **no verifier and no analysis layer**. Layer 2 (`kmet-verify`) and layer 3
//! (`kmet-trace`) depend on this crate; nothing here depends on them. The shared vocabulary they
//! all read — contracts, rule ids, severities, kinematic models — lives one layer below, in
//! `kmet-contracts`. This crate compiles to `wasm32-unknown-unknown` unmodified.
//!
//! Extracted verbatim from `dry-core` (plan Task 4); `dry_core` re-exports every name below, so the
//! bindings and the CLI reach the same surface under the same paths they always have.

#![forbid(unsafe_code)]

pub mod clothoid;
pub mod codec;
pub mod emit;
pub mod engine;
pub mod features;
pub mod frame;
pub mod gcode;

pub mod generate;
pub mod ir;
pub mod optimize;

pub mod profile;
pub mod resolve;
pub mod sdk;

pub mod units;

pub use clothoid::{corner_blend, fresnel, ClothoidError, CornerBlend, FRESNEL_SERIES_EPSILON};
pub use codec::{
    decode_any_streaming, decode_any_streaming_with_limits, decode_chunked_streaming,
    decode_chunked_streaming_with_limits, decode_streaming, decode_streaming_with_limits,
    decode_with_limits, encode_chunked, export_3mf_xml, import_3mf_xml, BinarySegmentsIterator,
    ChunkedSegmentsIterator, CodecError, DecodeLimits, JsonSegmentsIterator, SegmentStream,
    StreamingDecode, ThreeMfError,
};
#[allow(deprecated)]
pub use emit::emit;
pub use emit::{
    emit_step_nc, emit_stream, emit_stream_to_writer, CncFrame, EmitParams, FirmwareFlavor,
    Kinematics, KrlFrame, KrlTransform, REFERENCE_FIVE_AXIS_LIMITS, REFERENCE_FIVE_AXIS_MACHINE,
};
pub use engine::{simulate, simulate_stream, Metrics};
pub use features::{
    expand_features, expand_features_with_limits, ExpandError, ExpandLimits, FeatureNode,
    FeaturePose, FeatureProgram, DEFAULT_MAX_EXPANDED_NODES, DEFAULT_MAX_EXPANDED_OPS,
    DEFAULT_MAX_FEATURE_DEPTH,
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
pub use resolve::{
    resolve, resolve_checked, validate_design, Design, Op, ResolveError, ResolveParams,
};
pub use sdk::DesignBuilder;

pub use units::{Angle, Area, Feedrate, Flow, Length, Time, Volume};

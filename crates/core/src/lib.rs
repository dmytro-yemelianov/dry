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

pub mod codec;
pub mod emit;
pub mod engine;
pub mod forensics;
pub mod gcode;
pub mod ir;
pub mod optimize;
pub mod profile;
pub mod report;
pub mod resolve;
pub mod trace;
pub mod units;
pub mod verify;

pub use codec::{
    decode_any_streaming, decode_chunked_streaming, decode_streaming, encode_chunked,
    BinarySegmentsIterator, ChunkedSegmentsIterator, CodecError, JsonSegmentsIterator,
    SegmentStream, StreamingDecode,
};
pub use emit::{emit, emit_stream, emit_stream_to_writer, EmitParams, FirmwareFlavor, Kinematics};
pub use engine::{simulate, simulate_stream, Metrics};
pub use forensics::{
    analyze as forensics_analyze, Confidence, DeclaredSettings, Estimate, FeatureStat,
    ForensicsReport, Hotspot, LayerModel, SeamHint, TravelStat, TravelStrategy,
};
pub use gcode::{
    import_gcode, import_gcode_reader, import_gcode_reader_with_map, import_gcode_with_map,
    import_parsed_gcode, import_parsed_gcode_with_map, parse_gcode_lines, DistanceMode,
    ExtrusionMode, GcodeImportError, GcodeImportParams, GcodeModalState, GcodeMotionSpan,
    GcodeParseError, GcodeParser, GcodeRecord, GcodeWord, ImportedGcode, MotionMode, MotionRecord,
    ParsedGcodeLine, ProcessCommand, StateCommand, UnitMode,
};
pub use ir::{Meta, Segment, SegmentKind, Toolpath};
pub use optimize::{
    adaptive_speed, adaptive_speed_with_params, arc_fit, coasting, coasting_with_dist,
    merge_collinear, optimize_aggressive_pipeline, optimize_pipeline, travel_reorder, z_hop,
    z_hop_with_params,
};
pub use profile::{
    FirmwareProfile, MachineProfile, MaterialProfile, ProcessProfile, Profile, ProfileError,
};
pub use report::{LocatedFinding, ReviewReport, TraceReport};
pub use resolve::{
    resolve, resolve_checked, validate_design, Design, Op, ResolveError, ResolveParams,
};
pub use trace::{trace_summary, trace_summary_with_sources, TraceError, TraceSummary, TraceWindow};
pub use units::{Angle, Area, Feedrate, Flow, Length, Time, Volume};
pub use verify::{
    catalog, parse_bounds_csv, parse_speed_range_csv, verify, verify_stream, ContractParseError,
    Contracts, Finding, Report, Rule, RuleId, Severity,
};

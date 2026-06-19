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
pub mod gcode;
pub mod ir;
pub mod optimize;
pub mod resolve;
pub mod units;
pub mod verify;

pub use codec::{
    decode_any_streaming, decode_chunked_streaming, decode_streaming, encode_chunked,
    BinarySegmentsIterator, ChunkedSegmentsIterator, CodecError, JsonSegmentsIterator,
    SegmentStream, StreamingDecode,
};
pub use emit::{emit, emit_stream, emit_stream_to_writer, EmitParams, Kinematics};
pub use engine::{simulate, simulate_stream, Metrics};
pub use gcode::{
    import_gcode, import_gcode_reader, import_gcode_reader_with_map, import_gcode_with_map,
    import_parsed_gcode, import_parsed_gcode_with_map, parse_gcode_lines, DistanceMode,
    ExtrusionMode, GcodeImportError, GcodeImportParams, GcodeModalState, GcodeMotionSpan,
    GcodeParseError, GcodeParser, GcodeRecord, GcodeWord, ImportedGcode, MotionMode, MotionRecord,
    ParsedGcodeLine, StateCommand, UnitMode,
};
pub use ir::{Meta, Segment, SegmentKind, Toolpath};
pub use optimize::{arc_fit, merge_collinear, optimize_pipeline, travel_reorder};
pub use resolve::{
    resolve, resolve_checked, validate_design, Design, Op, ResolveError, ResolveParams,
};
pub use units::{Angle, Area, Feedrate, Flow, Length, Time, Volume};
pub use verify::{
    parse_bounds_csv, parse_speed_range_csv, verify, verify_stream, ContractParseError, Contracts,
    Finding, Report, Severity,
};

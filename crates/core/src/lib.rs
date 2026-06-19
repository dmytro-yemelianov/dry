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
pub mod ir;
pub mod optimize;
pub mod resolve;
pub mod units;
pub mod verify;

pub use codec::{decode_streaming, BinarySegmentsIterator, JsonSegmentsIterator, CodecError};
pub use emit::{emit, emit_stream, EmitParams, Kinematics};
pub use engine::{simulate, simulate_stream, Metrics};
pub use ir::{Meta, Segment, Toolpath};
pub use optimize::{arc_fit, merge_collinear, travel_reorder};
pub use resolve::{resolve, Design, Op, ResolveParams};
pub use units::{Angle, Area, Feedrate, Flow, Length, Time, Volume};
pub use verify::{verify, verify_stream, Contracts, Finding, Report, Severity};

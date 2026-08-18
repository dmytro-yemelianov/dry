//! `generate` — L1 *authoring* generators (`docs/01-architecture.md` §1, the design tier).
//!
//! A generator is a pure parametric design: it takes a small option bundle and emits an ordered
//! `Vec<`[`crate::resolve::Op`]`>` (equivalently a [`crate::resolve::Design`]). It sits **upstream** of
//! [`crate::resolve`] — the produced ops are lowered to an L2 [`crate::ir::Toolpath`] by
//! [`crate::resolve::resolve_checked`] exactly like hand-authored designs, so generators are pure L1
//! sugar and inherit the whole engine (verify / simulate / emit) for free.
//!
//! The generators are the TPMS infill ([`tpms`]) with all ten surfaces ([`tpms::Surface`]) and the
//! CNC pocket/profile cutter ([`pocket`]); the PyO3, wasm and TS-SDK exposures are deferred
//! follow-ups.

pub mod pocket;
pub mod tpms;

pub use pocket::{
    pocket_design, pocket_ops, pocket_stepped_ops, try_pocket_design, try_pocket_ops, CutMode,
    PocketError, PocketOptions, PocketShape,
};
pub use tpms::{
    tpms_design, tpms_ops, try_tpms_design, try_tpms_ops, Surface, TpmsError, TpmsOptions,
};

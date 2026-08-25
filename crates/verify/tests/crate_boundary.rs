//! The kernel symbols `kmet-verify` reaches across the layer-1 → layer-2 crate boundary.
//!
//! An integration test compiles as a separate crate, so anything `pub(crate)` fails to resolve here.
//! That is the point: this file is the compile-time contract that the layer-2 boundary stays open.
//! See docs/superpowers/specs/2026-08-25-ip-registration-and-preservation-design.md §5.7.
//!
//! Task 1 wrote it against a boundary that did not exist yet, so it sat in `dry-core` and read the
//! kernel through `dry-core`'s re-exports. Both halves are real now: it lives in the crate that
//! actually consumes these names and imports them from `kmet-kernel` directly, so a narrowing fails
//! against the definition rather than at a re-export one crate away (plan Task 5, fix round 1).
//!
//! It is deliberately redundant. `kmet_verify`'s own `lib.rs` calls `resolve_joints`,
//! `rotary_words` and `machine_position` and reads `Rotary`'s fields, so a narrowing would break the
//! crate before it broke this file. The redundancy is the point of a boundary test: it states the
//! contract by name, in one place, instead of leaving it implied by where the rules happen to reach.

use kmet_kernel::emit::{KinematicsExt, RotaryState};
use kmet_kernel::engine::segment_motion_time;
use kmet_kernel::optimize::get_tangents;
use kmet_kernel::{resolve, Design, ResolveParams, REFERENCE_FIVE_AXIS_MACHINE};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

fn straight_run() -> Design {
    design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    )
}

#[test]
fn segment_motion_time_is_reachable_from_another_crate() {
    let tp = resolve(&straight_run(), &ResolveParams::default());
    let moving = tp.segments.iter().find(|s| s.length.value() > 0.0).unwrap();
    assert!(segment_motion_time(moving).is_some());
}

#[test]
fn get_tangents_is_reachable_from_another_crate() {
    let tp = resolve(&straight_run(), &ResolveParams::default());
    // `get_tangents` is arc-*aware*, not arc-only: it returns the entry and exit direction of any
    // segment long enough to have one, and refuses only the too-short or degenerate (see its own
    // doc). This 10 mm straight run therefore has tangents — both `[1.0, 0.0, 0.0]`.
    let seg = tp.segments.last().unwrap();
    assert!(get_tangents(seg).is_some());
}

#[test]
fn rotary_state_is_nameable_from_another_crate() {
    // Using the type in a `size_of` call proves it is both `pub` and `Sized` from outside the crate
    // without introducing an item at all — so nothing here depends on the underscore convention that
    // happens to suppress `dead_code` for an unused `fn _accepts(_s: &RotaryState) {}`.
    let _ = std::mem::size_of::<RotaryState>();
}

#[test]
fn the_rotary_geometry_is_callable_from_another_crate() {
    // The three rotary rules resolve a segment's orientation through the emitter's own geometry, so
    // `KinematicsExt` and both types its methods hand back — `Joints` and `Rotary` — have to be
    // reachable and, for `Rotary`, readable field by field. A trait method returning a type less
    // visible than itself is a `private_interfaces` error, so `Joints` cannot be narrowed alone.
    let model = REFERENCE_FIVE_AXIS_MACHINE;
    let mut state = RotaryState::default();
    let joints = model
        .resolve_joints(Some([0.0, 0.0, 1.0]), &mut state)
        .expect("+Z is resolvable under every model");
    let words = model.rotary_words(joints);
    assert_eq!(words.len(), 2);
    assert!(words.iter().all(|w| w.value.is_finite() && w.letter != ' '));
}

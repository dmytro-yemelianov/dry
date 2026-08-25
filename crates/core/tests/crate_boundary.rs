//! Symbols `kmet-verify` reaches across the future crate boundary (plan Task 1).
//!
//! An integration test compiles as a separate crate, so anything `pub(crate)` fails to resolve here.
//! That is the point: this file is the compile-time contract that the layer-2 boundary stays open.
//! See docs/superpowers/specs/2026-08-25-ip-registration-and-preservation-design.md §5.7.

use dry_core::emit::RotaryState;
use dry_core::engine::segment_motion_time;
use dry_core::optimize::get_tangents;
use dry_core::{resolve, Design, ResolveParams};

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

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
    // A straight line is not an arc, so there are no tangents — `None` is the correct answer.
    // The assertion under test is that the symbol resolves at all.
    let seg = tp.segments.last().unwrap();
    let _ = get_tangents(seg);
}

#[test]
fn rotary_state_is_nameable_from_another_crate() {
    // Using the type in a `size_of` call proves it is both `pub` and `Sized` from outside the crate,
    // and unlike an unused helper fn it cannot trip `dead_code` under `-D warnings`.
    let _ = std::mem::size_of::<RotaryState>();
}

//! `reverse` reconstructs an L1 `Design` from an L2 `Toolpath`, and each channel it re-emits is a
//! separate branch of that pass. The in-module tests in `reverse.rs` cover temperature, fan and flow;
//! this covers the **power** channel, which had its only assertion in `crates/core/tests/channels.rs`
//! — a `dry-core` integration test, so `cargo test -p drymachina-trace` could not see a regression in the
//! branch that produces it. Moved here verbatim with the pass it exercises (plan Task 6).

use drymachina_kernel::{resolve, Design, ResolveParams};
use drymachina_trace::reverse;

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

#[test]
fn reverse_round_trips_the_power_channel() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"power","level":600},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    let design_back = reverse(&tp).expect("reverse");
    let again = resolve(&design_back, &ResolveParams::default());
    assert_eq!(
        again.segments.last().unwrap().power,
        tp.segments.last().unwrap().power
    );
}

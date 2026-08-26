//! `dry-core` is now a facade. This test names one symbol from each of the four crates through it —
//! if all four resolve, no downstream crate needs to change (plan Task 7).

use dry_core::{
    forensics_analyze, resolve, simulate, trace_summary, verify, Contracts, Design, ResolveParams,
    Toolpath,
};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

#[test]
fn all_four_layers_are_reachable_through_the_facade() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp: Toolpath = resolve(&d, &ResolveParams::default()); // kernel
    let _ = simulate(&tp); // kernel
    let _ = verify(&tp, &Contracts::default()); // verify + contracts
    let _ = trace_summary(&tp, 1.0); // trace
    let _ = forensics_analyze; // trace
}

/// `dry_core::reverse` names a module *and* a function, and `lib.rs` re-exports both with a single
/// `pub use kmet_trace::reverse;` — the construct its comment there describes. `emit` and `resolve`
/// carry the same construct and are witnessed by their many in-tree callers of both namespaces;
/// `reverse` has none anywhere in the workspace, so replacing that line with an aliased module
/// re-export would compile silently and drop `dry_core::reverse(..)` from the public surface. This
/// is that missing witness: the annotated binding pins the function namespace, and naming
/// `reverse::ReverseError` in the same type pins the module namespace beside it.
#[test]
fn reverse_names_both_a_function_and_a_module_through_the_facade() {
    let reverse_fn: fn(&Toolpath) -> Result<Design, dry_core::reverse::ReverseError> =
        dry_core::reverse;

    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let round_tripped = reverse_fn(&tp).expect("reverse a two-move extrusion");
    assert!(
        !round_tripped.ops.is_empty(),
        "reverse must reconstruct the design's ops, not an empty program"
    );
}

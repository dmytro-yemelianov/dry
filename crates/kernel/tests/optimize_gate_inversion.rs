//! `apply_gated_with` is the kernel-side gate mechanism: it runs the pipeline and accepts the result
//! only when the caller's policy reports no *new* error rule. Policy lives in `kmet-verify`; the
//! kernel must not know what a rule is, and every policy below is a synthetic closure returning
//! opaque strings, precisely so that nothing here needs the verifier.
//!
//! That is why the file moved here from `crates/core/tests/` with the mechanism it tests (plan
//! Task 5, fix round 1): a test that names no verifier had no reason to sit two layers above one.
//! `tests/kernel_surface.rs` covers the parameters the same function forwards; this covers the
//! accept/reject decision it makes.

use std::collections::BTreeSet;

use kmet_kernel::optimize::apply_gated_with;
use kmet_kernel::{resolve, Design, OptimizeMode, ResolveParams};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

fn straight_run() -> Design {
    design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":5,"y":0,"z":0.2},
            {"op":"move","x":10,"y":0,"z":0.2}]"#,
    )
}

#[test]
fn accepts_when_policy_reports_no_new_rules() {
    let tp = resolve(&straight_run(), &ResolveParams::default());
    let out = apply_gated_with(&tp, OptimizeMode::Safe, None, |_| BTreeSet::new());
    assert!(out.accepted);
    assert!(out.new_error_rules.is_empty());
}

#[test]
fn rejects_and_returns_input_when_policy_reports_a_new_rule() {
    let tp = resolve(&straight_run(), &ResolveParams::default());
    // Policy: the input is clean, anything else has introduced "Bounds".
    let input_len = tp.segments.len();
    let out = apply_gated_with(&tp, OptimizeMode::Safe, None, |candidate| {
        let mut s = BTreeSet::new();
        if candidate.segments.len() != input_len {
            s.insert("Bounds".to_string());
        }
        s
    });
    assert!(!out.accepted);
    assert_eq!(out.new_error_rules, vec!["Bounds".to_string()]);
    // On rejection the input is returned verbatim.
    assert_eq!(out.toolpath.segments.len(), input_len);
}

#[test]
fn preexisting_rules_do_not_block() {
    let tp = resolve(&straight_run(), &ResolveParams::default());
    // Policy reports the same rule for input and candidate — pre-existing, so not "new".
    let out = apply_gated_with(&tp, OptimizeMode::Safe, None, |_| {
        let mut s = BTreeSet::new();
        s.insert("MaxFlow".to_string());
        s
    });
    assert!(out.accepted);
    assert!(out.new_error_rules.is_empty());
}

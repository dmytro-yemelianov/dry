//! `apply_gated` is the verification policy bound to the kernel's gate mechanism
//! (`drymachina_kernel::optimize::apply_gated_with`, plan Task 2). This proves the two halves rejoin
//! correctly after the split.

use drymachina_contracts::Contracts;
use drymachina_kernel::{resolve, Design, OptimizeMode, ResolveParams};
use drymachina_verify::apply_gated;

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

#[test]
fn clean_toolpath_passes_the_safe_gate() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":5,"y":0,"z":0.2},
            {"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let out = apply_gated(&tp, &Contracts::default(), OptimizeMode::Safe, None);
    assert!(out.accepted);
    assert!(out.new_error_rules.is_empty());
}

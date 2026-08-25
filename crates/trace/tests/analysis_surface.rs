//! Layer 3 stands on the kernel and the verifier and is depended on by nothing — which is why it
//! graduates to its own repository first (plan Task 8).

use kmet_kernel::{resolve, Design, ResolveParams};
use kmet_trace::trace_summary;

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

#[test]
fn trace_summary_runs_over_a_resolved_toolpath() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let s = trace_summary(&tp, 1.0).unwrap();
    assert_eq!(s.window_s, 1.0);
    assert!(s.total_time_s > 0.0);
    assert!(s.segment_count > 0);
}

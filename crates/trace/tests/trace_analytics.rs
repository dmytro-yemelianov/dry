//! The documented relationship between `trace.layers` and `forensics.layers.layer_count`
//! (`docs/11-profiles-and-reports.md` §3.3), pinned in both directions rather than left as prose.
//!
//! The two numbers count different things on purpose: `forensics` counts *distinct Z levels* (sorted
//! and deduped), while a trace layer is a *pass* in execution order. So
//! `trace.layers.len() >= forensics.layers.layer_count`, with equality exactly when Z is
//! non-decreasing and each level is a single contiguous run.

use drymachina_kernel::{import_gcode_with_map, GcodeImportParams};
use drymachina_trace::{forensics_analyze, trace_summary_with_analytics, TraceAnalyticsOptions};
use std::path::PathBuf;

fn layer_counts(source: &str) -> (usize, usize) {
    let imported =
        import_gcode_with_map(source, &GcodeImportParams::default()).expect("import g-code");
    let source_lines: Vec<Option<usize>> = imported
        .segment_source_lines
        .iter()
        .copied()
        .map(Some)
        .collect();
    let trace = trace_summary_with_analytics(
        &imported.toolpath,
        1.0,
        &source_lines,
        &TraceAnalyticsOptions::default(),
    )
    .expect("trace");
    let forensics = forensics_analyze(&imported);
    (trace.layers.len(), forensics.layers.layer_count)
}

#[test]
fn a_monotonic_file_agrees_with_the_forensics_layer_count() {
    let sample = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("examples/sliced-sample.gcode"),
    )
    .expect("cura sample exists");
    let (trace_layers, forensics_layers) = layer_counts(&sample);
    assert_eq!(forensics_layers, 2, "the Cura sample has two Z levels");
    assert_eq!(
        trace_layers, forensics_layers,
        "Z is non-decreasing and each level is one contiguous run, so passes == levels"
    );
}

#[test]
fn a_revisited_z_makes_trace_layers_outnumber_forensics_levels() {
    // Three extruding passes over two distinct Z levels: 0.2, 0.4, then back to 0.2.
    let source = "\
G90
M83
G1 Z0.2 F600
G1 X0 Y0 F9000
G1 X20 Y0 E0.8 F1200
G1 Z0.4 F600
G1 X20 Y20 E0.8 F1200
G1 Z0.2 F600
G1 X0 Y20 E0.8 F1200
";
    let (trace_layers, forensics_layers) = layer_counts(source);
    assert_eq!(forensics_layers, 2, "two *distinct* levels");
    assert_eq!(trace_layers, 3, "three *passes*");
    assert!(trace_layers > forensics_layers);
}

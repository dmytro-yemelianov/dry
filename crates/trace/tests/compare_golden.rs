//! Drift-gated golden for `compare_reports`: build two ExplainReports from committed fixtures, diff
//! them, and assert the result equals `conformance/reports/compare/expected.json`. Regenerate with
//! `DRY_REGEN=1 cargo test -p kmet-trace --test compare_golden`.
use kmet_contracts::Contracts;
use kmet_kernel::{import_gcode_with_map, simulate, GcodeImportParams};
use kmet_trace::{
    compare_reports, forensics_analyze, trace_summary_with_sources, CompareDelta, ExplainReports,
    ReviewReport, TraceReport,
};
use kmet_verify::verify;
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/reports/compare")
}

fn reports(name: &str) -> ExplainReports {
    let src = std::fs::read_to_string(dir().join(name)).unwrap();
    let imp = import_gcode_with_map(&src, &GcodeImportParams::default()).unwrap();
    let metrics = simulate(&imp.toolpath);
    let report = verify(&imp.toolpath, &Contracts::default());
    let review = ReviewReport::build(
        None,
        None,
        imp.toolpath.segments.len(),
        metrics,
        &report,
        |_| None,
    );
    let sources: Vec<_> = imp.segment_source_lines.iter().copied().map(Some).collect();
    let trace = trace_summary_with_sources(&imp.toolpath, 5.0, &sources).unwrap();
    ExplainReports {
        trace: TraceReport {
            file: None,
            profile: None,
            trace,
        },
        forensics: forensics_analyze(&imp),
        verify: review,
    }
}

#[test]
fn compare_matches_golden() {
    let delta = compare_reports(&reports("slow.gcode"), &reports("fast.gcode"));
    let got = serde_json::to_string_pretty(&delta).unwrap();
    let path = dir().join("expected.json");
    if std::env::var("DRY_REGEN").is_ok() {
        std::fs::write(&path, got.clone() + "\n").unwrap();
    }
    let want = std::fs::read_to_string(&path).unwrap();
    let want_delta: CompareDelta = serde_json::from_str(&want).unwrap();
    // Compare via re-serialization so float formatting is consistent on both sides.
    assert_eq!(
        got.trim(),
        serde_json::to_string_pretty(&want_delta).unwrap().trim()
    );
}

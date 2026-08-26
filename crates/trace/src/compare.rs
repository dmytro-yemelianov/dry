//! `compare` — a deterministic forensic diff between two analysed g-code files. Given the two
//! already-computed [`ExplainReports`] (trace + forensics + verify) it reports what changed: time/flow,
//! slicer + declared settings, and safety findings added/removed. Pure and golden-testable; the LLM
//! narrative layer lives in `dry-llm`/the CLI, never here.

use crate::explain::ExplainReports;
use serde::{Deserialize, Serialize};

/// before/after for a numeric metric, with absolute and (when `before != 0`) percent change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalarDelta {
    pub before: f64,
    pub after: f64,
    pub abs: f64,
    pub pct: Option<f64>,
}

impl ScalarDelta {
    fn new(before: f64, after: f64) -> Self {
        let abs = after - before;
        let pct = if before.abs() > f64::EPSILON {
            Some(abs / before * 100.0)
        } else {
            None
        };
        ScalarDelta {
            before,
            after,
            abs,
            pct,
        }
    }
}

/// before/after for a categorical value; only constructed when the two differ.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringChange {
    pub before: String,
    pub after: String,
}

/// Trace-derived timing deltas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeDelta {
    pub total: ScalarDelta,
    pub print: ScalarDelta,
    pub travel: ScalarDelta,
}

/// One changed forensics setting (declared or inferred), as a labelled before/after string pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingChange {
    pub field: String,
    pub before: String,
    pub after: String,
}

/// Verify findings added (present only in `after`) and removed (present only in `before`), keyed by
/// `"<rule>@<line>"` so a finding that moved lines is reported as removed+added rather than silently kept.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingsDelta {
    pub before_count: usize,
    pub after_count: usize,
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

/// The full two-file diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareDelta {
    /// `Some` when the detected slicer changed.
    pub slicer: Option<StringChange>,
    pub time: TimeDelta,
    pub peak_flow_mm3_s: ScalarDelta,
    pub layer_count: ScalarDelta,
    pub travel_distance_mm: ScalarDelta,
    pub retractions: ScalarDelta,
    /// Declared/inferred forensics settings that changed.
    pub settings: Vec<SettingChange>,
    pub findings: FindingsDelta,
    /// The licensing mode this delta was produced under, when the caller stamped one
    /// (see [`crate::LicenseStamp`]) — never set by the engine.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub license: Option<crate::report::LicenseStamp>,
}

/// Diff two analysed files. `before` is `<fileA>`, `after` is `<fileB>`. Pure and deterministic.
pub fn compare_reports(before: &ExplainReports, after: &ExplainReports) -> CompareDelta {
    let (bt, at) = (&before.trace.trace, &after.trace.trace);
    let (bf, af) = (&before.forensics, &after.forensics);

    let slicer = (bf.slicer != af.slicer).then(|| StringChange {
        before: bf.slicer.clone(),
        after: af.slicer.clone(),
    });

    let mut settings = Vec::new();
    let mut push_opt = |field: &str, b: &Option<String>, a: &Option<String>| {
        if b != a {
            settings.push(SettingChange {
                field: field.to_string(),
                before: b.clone().unwrap_or_else(|| "—".into()),
                after: a.clone().unwrap_or_else(|| "—".into()),
            });
        }
    };
    let fmt = |v: Option<f64>| v.map(|x| format!("{x}"));
    push_opt(
        "declared.layer_height_mm",
        &fmt(bf.declared.layer_height_mm),
        &fmt(af.declared.layer_height_mm),
    );
    push_opt(
        "declared.extrusion_width_mm",
        &fmt(bf.declared.extrusion_width_mm),
        &fmt(af.declared.extrusion_width_mm),
    );
    push_opt(
        "declared.infill_angle_deg",
        &fmt(bf.declared.infill_angle_deg),
        &fmt(af.declared.infill_angle_deg),
    );
    push_opt(
        "declared.infill_density",
        &bf.declared.infill_density,
        &af.declared.infill_density,
    );
    if bf.seam.strategy != af.seam.strategy {
        settings.push(SettingChange {
            field: "seam.strategy".into(),
            before: bf.seam.strategy.clone(),
            after: af.seam.strategy.clone(),
        });
    }
    if bf.travel_strategy.hint != af.travel_strategy.hint {
        settings.push(SettingChange {
            field: "travel_strategy.hint".into(),
            before: bf.travel_strategy.hint.clone(),
            after: af.travel_strategy.hint.clone(),
        });
    }

    // `verify.findings` is `Vec<LocatedFinding>` with fields `rule: String`, `severity`,
    // `segment: Option<usize>`, `source_line: Option<usize>`, `message`. Key on rule + source line so a
    // finding that moved lines reads as removed+added rather than silently unchanged.
    let key = |f: &crate::report::LocatedFinding| -> String {
        format!(
            "{}@{}",
            f.rule,
            f.source_line
                .map(|l| l.to_string())
                .unwrap_or_else(|| "?".into())
        )
    };
    let bset: std::collections::BTreeSet<String> =
        before.verify.findings.iter().map(&key).collect();
    let aset: std::collections::BTreeSet<String> = after.verify.findings.iter().map(&key).collect();
    let findings = FindingsDelta {
        before_count: before.verify.findings.len(),
        after_count: after.verify.findings.len(),
        added: aset.difference(&bset).cloned().collect(),
        removed: bset.difference(&aset).cloned().collect(),
    };

    CompareDelta {
        slicer,
        time: TimeDelta {
            total: ScalarDelta::new(bt.total_time_s, at.total_time_s),
            print: ScalarDelta::new(bt.print_time_s, at.print_time_s),
            travel: ScalarDelta::new(bt.travel_time_s, at.travel_time_s),
        },
        peak_flow_mm3_s: ScalarDelta::new(bt.max_flow_mm3_s, at.max_flow_mm3_s),
        layer_count: ScalarDelta::new(bf.layers.layer_count as f64, af.layers.layer_count as f64),
        travel_distance_mm: ScalarDelta::new(
            bf.travel.travel_distance_mm,
            af.travel.travel_distance_mm,
        ),
        retractions: ScalarDelta::new(bf.travel.retractions as f64, af.travel.retractions as f64),
        settings,
        findings,
        license: None,
    }
}

/// Render the delta as a Markdown briefing: a headline table + changed-settings + findings sections.
pub fn render_markdown(delta: &CompareDelta) -> String {
    use std::fmt::Write as _;
    let mut md = String::new();
    let _ = writeln!(md, "# Dry compare (A → B)\n");
    if let Some(s) = &delta.slicer {
        let _ = writeln!(md, "**Slicer:** {} → {}\n", s.before, s.after);
    }
    let row = |md: &mut String, label: &str, d: &ScalarDelta| {
        let pct = d.pct.map(|p| format!(" ({p:+.1}%)")).unwrap_or_default();
        let _ = writeln!(
            md,
            "| {label} | {:.2} | {:.2} | {:+.2}{pct} |",
            d.before, d.after, d.abs
        );
    };
    let _ = writeln!(md, "| Metric | A | B | Δ |");
    let _ = writeln!(md, "|---|---|---|---|");
    row(&mut md, "Total Time (s)", &delta.time.total);
    row(&mut md, "Print Time (s)", &delta.time.print);
    row(&mut md, "Travel Time (s)", &delta.time.travel);
    row(&mut md, "Peak flow (mm³/s)", &delta.peak_flow_mm3_s);
    row(&mut md, "Layers", &delta.layer_count);
    row(&mut md, "Travel dist (mm)", &delta.travel_distance_mm);
    row(&mut md, "Retractions", &delta.retractions);
    if !delta.settings.is_empty() {
        let _ = writeln!(md, "\n## Settings changed\n");
        for s in &delta.settings {
            let _ = writeln!(md, "- `{}`: {} → {}", s.field, s.before, s.after);
        }
    }
    let _ = writeln!(
        md,
        "\n## Findings: {} → {}",
        delta.findings.before_count, delta.findings.after_count
    );
    for a in &delta.findings.added {
        let _ = writeln!(md, "- + {a}");
    }
    for r in &delta.findings.removed {
        let _ = writeln!(md, "- − {r}");
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        forensics_analyze, trace_summary_with_sources, ExplainReports, ReviewReport, TraceReport,
    };
    use drymachina_contracts::Contracts;
    use drymachina_kernel::{import_gcode_with_map, simulate, GcodeImportParams};
    use drymachina_verify::verify;

    // Build an ExplainReports for a g-code string, mirroring how the CLI assembles one.
    fn reports(src: &str) -> ExplainReports {
        let imp = import_gcode_with_map(src, &GcodeImportParams::default()).unwrap();
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
        let trace_report = TraceReport {
            file: None,
            profile: None,
            trace,
        };
        let forensics = forensics_analyze(&imp);
        ExplainReports {
            trace: trace_report,
            forensics,
            verify: review,
        }
    }

    // Two programs identical except feedrate → a non-zero time/flow delta, zero settings/findings delta.
    const SLOW: &str = "G1 X0 Y0 E0\nG1 X20 Y0 E1 F1200\nG1 X20 Y20 E2 F1200\n";
    const FAST: &str = "G1 X0 Y0 E0\nG1 X20 Y0 E1 F3000\nG1 X20 Y20 E2 F3000\n";

    #[test]
    fn faster_file_has_negative_time_delta() {
        let d = compare_reports(&reports(SLOW), &reports(FAST));
        // FAST is quicker → after < before → abs delta negative.
        assert!(d.time.total.after < d.time.total.before);
        assert!(d.time.total.abs < 0.0);
    }

    #[test]
    fn identical_files_have_empty_categorical_deltas() {
        let d = compare_reports(&reports(SLOW), &reports(SLOW));
        assert!(d.slicer.is_none());
        assert!(d.settings.is_empty());
        assert!(d.findings.added.is_empty() && d.findings.removed.is_empty());
        assert!((d.time.total.abs).abs() < 1e-9);
    }

    #[test]
    fn render_lists_the_changed_sections() {
        let md = render_markdown(&compare_reports(&reports(SLOW), &reports(FAST)));
        assert!(md.contains("# Dry compare"));
        assert!(md.contains("Time") && md.contains("Peak flow"));
    }
}

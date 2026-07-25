//! Typed report envelopes for the CLI/SDK report outputs (`docs/11-profiles-and-reports.md`).
//!
//! These give the `review-gcode` and `trace-gcode` JSON a single typed source of truth, so the report
//! wire shape is a real contract (validated by `spec/dry-reports-v1.schema.json` and the golden reports
//! under `conformance/reports/`) rather than an inline `json!` in the CLI.

use crate::engine::{simulate, Metrics};
use crate::gcode::ImportedGcode;
use crate::ir::Toolpath;
use crate::trace::TraceSummary;
use crate::verify::{Finding, Report, RuleId, Severity};
use serde::{Deserialize, Serialize};

/// A [`Finding`] resolved to its original source line (when the toolpath came from imported G-code).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocatedFinding {
    /// Stable kebab-case rule id (see [`crate::RuleId`]).
    pub rule: String,
    pub severity: Severity,
    /// The offending segment index, if local to one move.
    pub segment: Option<usize>,
    /// The original source line, when the finding maps back to imported G-code.
    pub source_line: Option<usize>,
    /// Human-readable description.
    pub message: String,
}

impl LocatedFinding {
    /// Resolve a finding to a source line via the supplied lookup.
    pub fn new(finding: &Finding, source_line: Option<usize>) -> Self {
        LocatedFinding {
            rule: finding.rule.clone(),
            severity: finding.severity,
            segment: finding.segment,
            source_line,
            message: finding.message.clone(),
        }
    }
}

/// The `review-gcode` report: metrics plus located safety findings for an imported G-code file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewReport {
    /// Source file label (when reviewing a file).
    pub file: Option<String>,
    /// Profile label, when a profile was supplied.
    pub profile: Option<String>,
    /// Number of segments in the imported toolpath.
    pub segments: usize,
    pub metrics: Metrics,
    pub findings: Vec<LocatedFinding>,
    /// Number of `error`-severity findings (warnings are not counted).
    pub error_count: usize,
}

impl ReviewReport {
    /// Build a review report from a verify [`Report`], resolving each finding's source line.
    pub fn build(
        file: Option<String>,
        profile: Option<String>,
        segments: usize,
        metrics: Metrics,
        report: &Report,
        mut source_line_for_segment: impl FnMut(usize) -> Option<usize>,
    ) -> Self {
        let findings = report
            .findings
            .iter()
            .map(|f| {
                let source_line = f.segment.and_then(&mut source_line_for_segment);
                LocatedFinding::new(f, source_line)
            })
            .collect();
        ReviewReport {
            file,
            profile,
            segments,
            metrics,
            findings,
            error_count: report.error_count(),
        }
    }

    /// Add source-located warnings for commands preserved by the G-code importer but not modeled by
    /// the verifier. These warnings are intentionally non-fatal for review, while machine-start
    /// workflows can choose to fail closed on any warning.
    pub fn add_unmodeled_gcode(&mut self, imported: &ImportedGcode) {
        self.findings.extend(
            imported
                .unmodeled_commands
                .iter()
                .map(|command| LocatedFinding {
                    rule: RuleId::UnmodeledGcode.as_str().to_string(),
                    severity: RuleId::UnmodeledGcode.default_severity(),
                    segment: None,
                    source_line: Some(command.source_line),
                    message: format!(
                        "{} is preserved but not semantically verified: {}",
                        command.command, command.raw
                    ),
                }),
        );
    }
}

/// The `trace-gcode` report: a windowed motion/time-series summary for a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceReport {
    pub file: Option<String>,
    pub profile: Option<String>,
    pub trace: TraceSummary,
}

/// The per-span outcome of a gated `rewrite-gcode --mode safe` pass (one entry per source motion span).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewriteSpanResult {
    /// The span's index in source order (0-based).
    pub span_index: usize,
    /// Whether the rewrite was accepted for this span (rejected spans pass through verbatim).
    pub accepted: bool,
    /// Segment count of the span before the rewrite.
    pub segment_count_before: usize,
    /// Segment count of the span after the (accepted) rewrite, or unchanged when rejected.
    pub segment_count_after: usize,
    /// The error rule ids the rewrite would have introduced (empty when accepted).
    pub new_error_rules: Vec<String>,
}

/// The `rewrite-gcode --json` report: the per-span accept/reject ledger of a gated optimisation pass
/// plus whole-file before/after metrics (`docs/11-profiles-and-reports.md`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewriteReport {
    /// Source file label (when rewriting a file).
    pub file: Option<String>,
    /// Profile label, when a profile was supplied.
    pub profile: Option<String>,
    /// The optimisation mode used (e.g. `"safe"`).
    pub mode: String,
    /// Total number of source motion spans.
    pub spans_total: usize,
    /// Number of spans whose rewrite was accepted.
    pub spans_accepted: usize,
    /// Number of spans whose rewrite was rejected (and passed through verbatim).
    pub spans_rejected: usize,
    /// Total motion segment count before the rewrite.
    pub segment_count_before: usize,
    /// Total motion segment count after the rewrite.
    pub segment_count_after: usize,
    /// Simulated metrics of the whole motion before the rewrite.
    pub metrics_before: Metrics,
    /// Simulated metrics of the whole motion after the (gated) rewrite.
    pub metrics_after: Metrics,
    /// The per-span accept/reject ledger, in source order.
    pub spans: Vec<RewriteSpanResult>,
}

impl RewriteReport {
    /// Build a rewrite report from the before/after motion toolpaths and the per-span ledger. The
    /// before/after toolpaths are the concatenation of every span's motion (in source order); their
    /// metrics are simulated here so the report is the single typed source of truth for the wire shape.
    pub fn build(
        file: Option<String>,
        profile: Option<String>,
        mode: String,
        before: &Toolpath,
        after: &Toolpath,
        spans: Vec<RewriteSpanResult>,
    ) -> Self {
        let spans_accepted = spans.iter().filter(|s| s.accepted).count();
        let spans_rejected = spans.len() - spans_accepted;
        RewriteReport {
            file,
            profile,
            mode,
            spans_total: spans.len(),
            spans_accepted,
            spans_rejected,
            segment_count_before: before.segments.len(),
            segment_count_after: after.segments.len(),
            metrics_before: simulate(before),
            metrics_after: simulate(after),
            spans,
        }
    }
}

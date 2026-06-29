//! Typed report envelopes for the CLI/SDK report outputs (`docs/11-profiles-and-reports.md`).
//!
//! These give the `review-gcode` and `trace-gcode` JSON a single typed source of truth, so the report
//! wire shape is a real contract (validated by `spec/dry-reports-v1.schema.json` and the golden reports
//! under `conformance/reports/`) rather than an inline `json!` in the CLI.

use crate::engine::Metrics;
use crate::trace::TraceSummary;
use crate::verify::{Finding, Report, Severity};
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
}

/// The `trace-gcode` report: a windowed motion/time-series summary for a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceReport {
    pub file: Option<String>,
    pub profile: Option<String>,
    pub trace: TraceSummary,
}

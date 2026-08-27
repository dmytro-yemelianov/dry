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

/// Which licensing mode produced a report, stamped onto the report envelopes by the CLI.
///
/// Passive data only: the engine never verifies a licence, never reads one, and never sets this — it
/// exists here so the wire shape of a stamped report is part of the same typed contract as the rest.
/// A report the engine built carries `None`, which serializes away entirely, so the golden reports
/// under `conformance/reports/` are byte-identical with and without the field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseStamp {
    /// `"licensed"` or `"evaluation"`.
    pub mode: String,
    /// The licensee, when running licensed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub licensee: Option<String>,
    /// The licence tier, when running licensed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
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
    /// How many segments the verify pass actually looked at. Zero means it proved nothing.
    ///
    /// Carried for the same reason [`Report`] carries it: a clean review over no contracts is not
    /// the same result as a clean review against a machine profile, and a report that cannot tell
    /// them apart invites the first to be read as the second.
    #[serde(default)]
    pub segments_inspected: usize,
    /// The wire ids of every rule that was in force, in catalog order.
    #[serde(default)]
    pub rules_evaluated: Vec<String>,
    /// The licensing mode this report was produced under, when the caller stamped one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub license: Option<LicenseStamp>,
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
            segments_inspected: report.segments_inspected,
            rules_evaluated: report.rules_evaluated.clone(),
            license: None,
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

/// The outcome of one file in a batch review.
///
/// `passed` is *inspected and `error_count == 0`* — warnings do not fail a file, which is the same rule
/// `review-gcode`'s own exit code uses. `errored` is the file that could not be inspected at all, and it
/// is a distinct verdict rather than a failure: an incomplete batch is neither a pass nor a trustworthy
/// gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BatchStatus {
    Passed,
    Failed,
    Errored,
}

/// One file's entry in a [`ReviewBatch`]: either the [`ReviewReport`] it produced, or why it produced
/// none. Exactly one of `review` / `error` is `Some`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchFileResult {
    pub file: String,
    pub status: BatchStatus,
    /// Present iff the file was inspected. The nested report is an ordinary [`ReviewReport`], not a
    /// batch-specific shape, so a single-file review and a batch entry are the same document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewReport>,
    /// Present iff the file was not inspected — the read/import error's own message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl BatchFileResult {
    /// An inspected file. The status follows from the report's `error_count`.
    pub fn inspected(file: String, review: ReviewReport) -> Self {
        let status = if review.error_count > 0 {
            BatchStatus::Failed
        } else {
            BatchStatus::Passed
        };
        BatchFileResult {
            file,
            status,
            review: Some(review),
            error: None,
        }
    }

    /// A file that could not be read or imported: recorded, never fatal to the batch.
    pub fn errored(file: String, error: String) -> Self {
        BatchFileResult {
            file,
            status: BatchStatus::Errored,
            review: None,
            error: Some(error),
        }
    }
}

/// Per-rule roll-up across a batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleTally {
    /// Stable kebab-case rule id (see [`crate::RuleId`]).
    pub rule: String,
    /// Total `error`-severity findings for this rule across the batch.
    pub errors: usize,
    /// Total `warning`-severity findings for this rule across the batch.
    pub warnings: usize,
    /// How many files carry at least one finding for this rule (a rule twice in one file counts one
    /// file, two findings).
    pub files: usize,
}

/// The `review-batch` report: one envelope over N files, each nesting an unmodified [`ReviewReport`].
///
/// The licence is stamped once, on the envelope; nested reports carry no stamp. Exit-code semantics
/// live with the CLI, but the three counts below are what it decides on: any `files_errored` outranks
/// any `files_failed`, because "do not trust this verdict" is a different fact from "this file is
/// unsafe".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewBatch {
    pub files_total: usize,
    pub files_passed: usize,
    pub files_failed: usize,
    pub files_errored: usize,
    /// Profile label, once for the batch (every file is reviewed against the same one).
    pub profile: Option<String>,
    /// Per-rule roll-up, ascending by rule id — derived from a `BTreeMap`, so the order is
    /// deterministic and diffable between runs.
    pub findings_by_rule: Vec<RuleTally>,
    /// One entry per input path, in input order. No dedup: a path listed twice is reviewed twice.
    pub results: Vec<BatchFileResult>,
    /// The licensing mode this report was produced under, when the caller stamped one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub license: Option<LicenseStamp>,
}

impl ReviewBatch {
    /// Aggregate per-file outcomes into the batch envelope. Pure: the caller does the I/O.
    pub fn build(profile: Option<String>, results: Vec<BatchFileResult>) -> Self {
        let count = |want: BatchStatus| results.iter().filter(|r| r.status == want).count();
        let (files_passed, files_failed, files_errored) = (
            count(BatchStatus::Passed),
            count(BatchStatus::Failed),
            count(BatchStatus::Errored),
        );

        let mut tallies: std::collections::BTreeMap<&str, (usize, usize, usize)> =
            std::collections::BTreeMap::new();
        for result in &results {
            let Some(review) = result.review.as_ref() else {
                continue;
            };
            let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
            for finding in &review.findings {
                let entry = tallies.entry(finding.rule.as_str()).or_default();
                match finding.severity {
                    Severity::Error => entry.0 += 1,
                    Severity::Warning => entry.1 += 1,
                }
                if seen.insert(finding.rule.as_str()) {
                    entry.2 += 1;
                }
            }
        }
        let findings_by_rule = tallies
            .into_iter()
            .map(|(rule, (errors, warnings, files))| RuleTally {
                rule: rule.to_string(),
                errors,
                warnings,
                files,
            })
            .collect();

        ReviewBatch {
            files_total: results.len(),
            files_passed,
            files_failed,
            files_errored,
            profile,
            findings_by_rule,
            results,
            license: None,
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
    /// The licensing mode this report was produced under, when the caller stamped one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub license: Option<LicenseStamp>,
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
            license: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Metrics;

    fn located(rule: &str, severity: Severity) -> LocatedFinding {
        LocatedFinding {
            rule: rule.to_string(),
            severity,
            segment: Some(0),
            source_line: None,
            message: "probe".to_string(),
        }
    }

    fn review(file: &str, findings: Vec<LocatedFinding>) -> ReviewReport {
        let error_count = findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count();
        ReviewReport {
            file: Some(file.to_string()),
            profile: None,
            segments: 3,
            metrics: Metrics::default(),
            findings,
            error_count,
            segments_inspected: 3,
            rules_evaluated: Vec::new(),
            license: None,
        }
    }

    #[test]
    fn a_files_status_follows_its_error_count_not_its_warnings() {
        let warned = BatchFileResult::inspected(
            "warned.gcode".into(),
            review("warned.gcode", vec![located("bead", Severity::Warning)]),
        );
        assert_eq!(warned.status, BatchStatus::Passed);
        let failed = BatchFileResult::inspected(
            "failed.gcode".into(),
            review("failed.gcode", vec![located("max-flow", Severity::Error)]),
        );
        assert_eq!(failed.status, BatchStatus::Failed);
    }

    #[test]
    fn exactly_one_of_review_and_error_is_present() {
        for result in [
            BatchFileResult::inspected("a.gcode".into(), review("a.gcode", vec![])),
            BatchFileResult::errored("b.gcode".into(), "cannot import".into()),
        ] {
            assert_ne!(result.review.is_some(), result.error.is_some());
        }
    }

    #[test]
    fn the_envelope_counts_each_verdict_separately() {
        let batch = ReviewBatch::build(
            Some("voron24-abs".into()),
            vec![
                BatchFileResult::inspected("a.gcode".into(), review("a.gcode", vec![])),
                BatchFileResult::inspected(
                    "b.gcode".into(),
                    review("b.gcode", vec![located("max-flow", Severity::Error)]),
                ),
                BatchFileResult::errored("c.gcode".into(), "unsupported word at line 12".into()),
            ],
        );
        assert_eq!(batch.files_total, 3);
        assert_eq!(batch.files_passed, 1);
        assert_eq!(batch.files_failed, 1);
        assert_eq!(batch.files_errored, 1);
        assert_eq!(batch.profile.as_deref(), Some("voron24-abs"));
        // Input order is preserved; the errored file carries no report.
        assert_eq!(
            batch.results.iter().map(|r| r.status).collect::<Vec<_>>(),
            vec![
                BatchStatus::Passed,
                BatchStatus::Failed,
                BatchStatus::Errored
            ]
        );
    }

    #[test]
    fn a_rule_twice_in_one_file_counts_one_file_and_two_findings() {
        let batch = ReviewBatch::build(
            None,
            vec![
                BatchFileResult::inspected(
                    "a.gcode".into(),
                    review(
                        "a.gcode",
                        vec![
                            located("max-flow", Severity::Error),
                            located("max-flow", Severity::Error),
                            located("bead", Severity::Warning),
                        ],
                    ),
                ),
                BatchFileResult::inspected(
                    "b.gcode".into(),
                    review("b.gcode", vec![located("bead", Severity::Warning)]),
                ),
                // An errored file contributes no findings at all.
                BatchFileResult::errored("c.gcode".into(), "cannot read".into()),
            ],
        );
        // Ascending by rule id, derived from a BTreeMap — deterministic and diffable.
        assert_eq!(
            batch
                .findings_by_rule
                .iter()
                .map(|t| (t.rule.as_str(), t.errors, t.warnings, t.files))
                .collect::<Vec<_>>(),
            vec![("bead", 0, 2, 2), ("max-flow", 2, 0, 1)]
        );
    }

    #[test]
    fn an_empty_batch_aggregates_to_zeroes() {
        let batch = ReviewBatch::build(None, Vec::new());
        assert_eq!(batch.files_total, 0);
        assert!(batch.findings_by_rule.is_empty());
        assert!(batch.results.is_empty());
    }

    #[test]
    fn the_batch_status_wire_form_is_lowercase_and_the_optionals_skip() {
        let json = serde_json::to_value(BatchFileResult::errored(
            "c.gcode".into(),
            "cannot read".into(),
        ))
        .unwrap();
        assert_eq!(json["status"], "errored");
        assert!(
            json.get("review").is_none(),
            "an absent report skips, never null"
        );
        let json = serde_json::to_value(ReviewBatch::build(None, Vec::new())).unwrap();
        assert!(json.get("license").is_none());
        assert_eq!(json["profile"], serde_json::Value::Null);
    }
}

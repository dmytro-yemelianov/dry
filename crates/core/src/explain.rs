//! `explain` — assemble a deterministic, LLM-ready explanation bundle from the engine's structured
//! analyses (trace + forensics + verify).
//!
//! The engine never calls an LLM. `explain` produces a *facts-plus-prompt* bundle the user pastes into
//! Claude (or that Claude Code / an agent / an MCP consumes): the deterministic reports stay the ground
//! truth, and a curated prompt asks the model to explain "what is this print, why is it slow, what's
//! risky" and propose concrete changes — with a hard guardrail that every change is a hypothesis to be
//! re-verified by `dry`, never trusted on the model's word. The prompt is a fixed template with light,
//! deterministic interpolation, so the bundle is reproducible and golden-testable. See
//! `docs/superpowers/specs/2026-06-29-llm-explain-bundle-design.md` and `docs/05-product-directions.md` §4.

use crate::{ForensicsReport, ReviewReport, TraceReport};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// The three deterministic reports an explanation is grounded in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainReports {
    pub trace: TraceReport,
    pub forensics: ForensicsReport,
    pub verify: ReviewReport,
}

/// A self-contained, LLM-ready explanation bundle: the deterministic facts plus a curated prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainBundle {
    /// Source file label, when explaining a file.
    pub file: Option<String>,
    /// Profile label, when a profile was supplied.
    pub profile: Option<String>,
    /// True when verify ran against a supplied profile's contracts (vs. engine defaults).
    pub profiled: bool,
    pub reports: ExplainReports,
    /// The curated instruction block to send to an LLM alongside `reports`.
    pub prompt: String,
}

/// The guardrail sentence every bundle's prompt carries — the LLM may *suggest* changes, but `dry`
/// gates them. Exposed so callers/tests can assert it is present.
pub const GUARDRAIL: &str = "Any setting or profile change you propose is a HYPOTHESIS, not a verified \
fix: it MUST be re-checked by re-running `dry verify` / `dry review-gcode` against the profile before \
it is trusted. Never present a change as safe without that gate.";

/// Build the bundle from the three already-computed reports. Pure and deterministic.
pub fn build_explain_bundle(
    file: Option<String>,
    profile: Option<String>,
    profiled: bool,
    reports: ExplainReports,
) -> ExplainBundle {
    let prompt = build_prompt(file.as_deref(), profile.as_deref(), profiled);
    ExplainBundle {
        file,
        profile,
        profiled,
        reports,
        prompt,
    }
}

fn build_prompt(file: Option<&str>, profile: Option<&str>, profiled: bool) -> String {
    let file = file.unwrap_or("(stdin)");
    let profile_line = match (profile, profiled) {
        (Some(p), true) => format!("profile `{p}` (verify ran against its contracts)"),
        _ => "no profile (verify ran with default contracts)".to_string(),
    };
    format!(
        "You are a senior 3D-printing / CNC process engineer. Below this prompt are three deterministic \
analyses of a sliced G-code toolpath, produced by the Dry compiler: a TRACE (per-window motion/time \
series), a FORENSICS report (slicer + settings inference, every value confidence-tagged \
`from-comment`/`measured`/`inferred`), and a VERIFY report (machine-safety findings).\n\n\
Ground rules — do not violate:\n\
- The numbers in the reports are ground truth from a deterministic engine. Do not recompute, estimate, \
or invent metrics; cite the reports.\n\
- Respect every forensics confidence tag — never state an `inferred` value as certain.\n\
- {GUARDRAIL}\n\n\
Context: file `{file}`, {profile_line}.\n\n\
Do the following, in order:\n\
1. WHAT IS THIS PRINT? Summarise from the forensics report (slicer, layer model, line width, infill \
angle/spacing, seam, travel strategy), noting confidence where it matters.\n\
2. WHERE DOES THE TIME GO, AND WHY IS IT SLOW? Use the trace windows — name the slowest windows and the \
dominant cost (print vs travel vs dwell), and the feedrate/flow ceilings.\n\
3. WHAT'S RISKY? Summarise the verify findings by severity and what each implies for the print.\n\
4. RECOMMENDATIONS. Propose a prioritised table — columns: Change | Expected effect | How to re-verify \
with dry — ordered by impact, each naming the exact profile field or slicer setting.\n\n\
This analysis works best with a frontier model: Claude Opus 4.8 (`claude-opus-4-8`); use Claude Fable \
5 (`claude-fable-5`) for the most demanding analysis."
    )
}

/// Render the bundle as a self-contained Markdown briefing: at-a-glance headlines, the three reports as
/// labelled JSON blocks, and the curated prompt. Paste the whole document into an LLM.
pub fn render_markdown(bundle: &ExplainBundle) -> String {
    let f = &bundle.reports.forensics;
    let t = &bundle.reports.trace.trace;
    let v = &bundle.reports.verify;
    let file = bundle.file.as_deref().unwrap_or("(stdin)");

    let mut md = String::new();
    let _ = writeln!(md, "# Dry explain — {file}\n");
    let _ = writeln!(
        md,
        "A deterministic explanation bundle: the facts (trace + forensics + verify) plus a ready \
prompt. Paste this whole document into Claude, or hand it to an agent.\n"
    );

    let _ = writeln!(md, "## Headlines\n");
    let _ = writeln!(md, "| Fact | Value |");
    let _ = writeln!(md, "|---|---|");
    let _ = writeln!(md, "| Slicer | {} |", f.slicer);
    let _ = writeln!(md, "| Layers | {} |", f.layers.layer_count);
    let _ = writeln!(
        md,
        "| Total time | {:.1} s (print {:.1} s, travel {:.1} s) |",
        t.total_time_s, t.print_time_s, t.travel_time_s
    );
    let _ = writeln!(md, "| Peak flow | {:.2} mm³/s |", t.max_flow_mm3_s);
    let profile_note = if bundle.profiled {
        format!("profile `{}`", bundle.profile.as_deref().unwrap_or("?"))
    } else {
        "default contracts".to_string()
    };
    let _ = writeln!(
        md,
        "| Verify | {} finding(s), {} error(s) — {profile_note} |\n",
        v.findings.len(),
        v.error_count
    );

    let _ = writeln!(md, "## Facts\n");
    for (label, json) in [
        ("Trace", serde_json::to_string_pretty(&bundle.reports.trace)),
        (
            "Forensics",
            serde_json::to_string_pretty(&bundle.reports.forensics),
        ),
        (
            "Verify",
            serde_json::to_string_pretty(&bundle.reports.verify),
        ),
    ] {
        let _ = writeln!(md, "### {label}\n");
        let _ = writeln!(md, "```json\n{}\n```\n", json.unwrap_or_default());
    }

    let _ = writeln!(md, "## Prompt — send this to an LLM with the facts above\n");
    let _ = writeln!(md, "```\n{}\n```", bundle.prompt);
    md
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_carries_the_guardrail_and_model_note() {
        let prompt = build_prompt(Some("part.gcode"), Some("voron-abs"), true);
        assert!(prompt.contains(GUARDRAIL), "guardrail must be present");
        assert!(prompt.contains("claude-opus-4-8"), "model recommendation");
        assert!(prompt.contains("part.gcode") && prompt.contains("voron-abs"));
        // the four tasks, in order.
        for marker in [
            "WHAT IS THIS PRINT",
            "WHERE DOES THE TIME GO",
            "WHAT'S RISKY",
            "RECOMMENDATIONS",
        ] {
            assert!(prompt.contains(marker), "missing task: {marker}");
        }
    }

    #[test]
    fn prompt_notes_default_contracts_without_a_profile() {
        let prompt = build_prompt(None, None, false);
        assert!(prompt.contains("default contracts"));
        assert!(prompt.contains("(stdin)"));
    }
}

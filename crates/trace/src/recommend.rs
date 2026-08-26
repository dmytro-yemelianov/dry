//! Recommendation schema shared between the LLM client (`dry-llm`, which deserialises the model's
//! structured output into [`Recommendation`]) and the deterministic executor in [`crate::recommend`].
//! `classify` is the honesty boundary: a recommendation is **executable** only if it names a change
//! `dry` can actually run and re-verify (a rewrite mode or one of the v1 contract fields); everything
//! else is **advisory** — an unverified hypothesis the user applies in their slicer.

use drymachina_kernel::optimize::OptimizeMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ActionKind {
    Rewrite,
    Contract,
    Advisory,
}

/// One model recommendation, as returned in the structured `output_config.format` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub title: String,
    pub rationale: String,
    pub expected_effect: String,
    #[serde(default = "default_priority")]
    pub priority: i64,
    pub action_kind: ActionKind,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub field: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
}

fn default_priority() -> i64 {
    99
}

/// A verify-contract field `dry` can override and re-check in v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractField {
    MaxFlow,
    SpeedRange,
    MinTemp,
    MonotonicZ,
}

/// The parsed override value for a [`ContractField`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContractOverride {
    Scalar(f64),
    Range([f64; 2]),
    Flag(bool),
}

/// A change `dry` can actually apply and re-verify.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutableAction {
    Rewrite {
        mode: OptimizeMode,
    },
    Contract {
        field: ContractField,
        override_: ContractOverride,
    },
}

/// The outcome of classifying a [`Recommendation`].
#[derive(Debug, Clone)]
pub enum Classified {
    Executable(ExecutableAction),
    /// Carries a short human reason it could not be executed.
    Advisory(String),
}

/// Map a model recommendation to an executable action, or demote it to advisory with a reason.
pub fn classify(rec: &Recommendation) -> Classified {
    match rec.action_kind {
        ActionKind::Advisory => Classified::Advisory("model marked this advisory".into()),
        ActionKind::Rewrite => match rec.mode.as_deref() {
            Some("safe") => Classified::Executable(ExecutableAction::Rewrite {
                mode: OptimizeMode::Safe,
            }),
            Some("balanced") => Classified::Executable(ExecutableAction::Rewrite {
                mode: OptimizeMode::Balanced,
            }),
            Some("max") => Classified::Executable(ExecutableAction::Rewrite {
                mode: OptimizeMode::Max,
            }),
            Some(other) => Classified::Advisory(format!("unknown rewrite mode `{other}`")),
            None => Classified::Advisory("rewrite recommendation has no mode".into()),
        },
        ActionKind::Contract => classify_contract(rec),
    }
}

fn classify_contract(rec: &Recommendation) -> Classified {
    let field = match rec.field.as_deref() {
        Some("max_flow") => ContractField::MaxFlow,
        Some("speed_range") => ContractField::SpeedRange,
        Some("min_temp") => ContractField::MinTemp,
        Some("monotonic_z") => ContractField::MonotonicZ,
        Some(other) => {
            return Classified::Advisory(format!(
                "contract field `{other}` is not executable in v1"
            ))
        }
        None => return Classified::Advisory("contract recommendation has no field".into()),
    };
    let raw = match rec.value.as_deref() {
        Some(v) => v.trim(),
        None => return Classified::Advisory("contract recommendation has no value".into()),
    };
    let override_ = match field {
        ContractField::MaxFlow | ContractField::MinTemp => match raw.parse::<f64>() {
            Ok(v) => ContractOverride::Scalar(v),
            Err(_) => return Classified::Advisory(format!("could not parse `{raw}` as a number")),
        },
        ContractField::SpeedRange => match drymachina_contracts::parse_speed_range_csv(raw) {
            Ok(pair) => ContractOverride::Range(pair),
            Err(_) => return Classified::Advisory(format!("could not parse `{raw}` as `min,max`")),
        },
        ContractField::MonotonicZ => match raw {
            "true" | "1" | "yes" => ContractOverride::Flag(true),
            "false" | "0" | "no" => ContractOverride::Flag(false),
            _ => return Classified::Advisory(format!("could not parse `{raw}` as a bool")),
        },
    };
    Classified::Executable(ExecutableAction::Contract { field, override_ })
}

use crate::trace::trace_summary;
use drymachina_contracts::{Contracts, Severity};
use drymachina_kernel::gcode::ImportedGcode;
use drymachina_kernel::ir::Toolpath;
use drymachina_kernel::profile::MachineKinematics;
use drymachina_verify::{apply_gated, verify};

/// Measured state of a toolpath under a set of contracts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSnapshot {
    pub total_time_s: f64,
    pub max_flow_mm3_s: f64,
    pub findings: usize,
    pub error_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Rewrite: time or peak flow improved and the result still verifies clean.
    Improved,
    /// Rewrite: verifies clean but no measurable gain.
    CleanNoGain,
    /// Rewrite: introduced a new error finding (should not happen given the per-span gate).
    Regressed,
    /// Contract: the toolpath is unchanged; this reports the compliance shift under the new limit.
    Informational,
}

/// The measured outcome of applying one [`ExecutableAction`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub action: String,
    pub before: MetricSnapshot,
    pub after: MetricSnapshot,
    pub verdict: Verdict,
    pub note: String,
}

fn snapshot(tp: &Toolpath, contracts: &Contracts, window_s: f64) -> MetricSnapshot {
    let trace = trace_summary(tp, window_s).ok();
    let report = verify(tp, contracts);
    let error_count = report
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    MetricSnapshot {
        total_time_s: trace.as_ref().map(|t| t.total_time_s).unwrap_or(0.0),
        max_flow_mm3_s: trace.as_ref().map(|t| t.max_flow_mm3_s).unwrap_or(0.0),
        findings: report.findings.len(),
        error_count,
    }
}

/// Apply `action`, re-trace + re-verify, and report the measured delta. Deterministic; no I/O.
pub fn apply_executable(
    action: &ExecutableAction,
    imported: &ImportedGcode,
    contracts: &Contracts,
    kinematics: Option<&MachineKinematics>,
    window_s: f64,
) -> ExecutionResult {
    match action {
        ExecutableAction::Rewrite { mode } => {
            let before_tp = &imported.toolpath;
            // Mirror the per-span gated rewrite the `rewrite-gcode --mode` command performs.
            let mut after_segments = Vec::with_capacity(before_tp.segments.len());
            for span in imported.motion_spans() {
                let range = span.segment_range();
                let span_tp = Toolpath {
                    version: before_tp.version,
                    meta: before_tp.meta.clone(),
                    segments: before_tp.segments[range].to_vec(),
                };
                let result = apply_gated(&span_tp, contracts, *mode, kinematics);
                after_segments.extend(result.toolpath.segments);
            }
            let after_tp = Toolpath {
                version: before_tp.version,
                meta: before_tp.meta.clone(),
                segments: after_segments,
            };
            let before = snapshot(before_tp, contracts, window_s);
            let after = snapshot(&after_tp, contracts, window_s);
            let label = match mode {
                OptimizeMode::Safe => "safe",
                OptimizeMode::Balanced => "balanced",
                OptimizeMode::Max => "max",
            };
            let improved = after.total_time_s + 1e-6 < before.total_time_s
                || after.max_flow_mm3_s + 1e-6 < before.max_flow_mm3_s;
            let verdict = if after.error_count > before.error_count {
                Verdict::Regressed
            } else if improved {
                Verdict::Improved
            } else {
                Verdict::CleanNoGain
            };
            let note = format!(
                "time {:.1}s -> {:.1}s, peak flow {:.2} -> {:.2} mm3/s",
                before.total_time_s,
                after.total_time_s,
                before.max_flow_mm3_s,
                after.max_flow_mm3_s
            );
            ExecutionResult {
                action: format!("rewrite-gcode --mode {label}"),
                before,
                after,
                verdict,
                note,
            }
        }
        ExecutableAction::Contract { field, override_ } => {
            let tp = &imported.toolpath;
            let before = snapshot(tp, contracts, window_s);
            let mut modified = contracts.clone();
            let label = apply_contract_override(&mut modified, *field, *override_);
            let after = snapshot(tp, &modified, window_s);
            let note = format!(
                "{label}: findings {} -> {} (errors {} -> {})",
                before.findings, after.findings, before.error_count, after.error_count
            );
            ExecutionResult {
                action: format!("contract {label}"),
                before,
                after,
                verdict: Verdict::Informational,
                note,
            }
        }
    }
}

/// Apply one override onto a cloned `Contracts`, returning a human label like `max_flow=12`.
fn apply_contract_override(
    c: &mut Contracts,
    field: ContractField,
    value: ContractOverride,
) -> String {
    match (field, value) {
        (ContractField::MaxFlow, ContractOverride::Scalar(v)) => {
            c.max_flow = Some(v);
            format!("max_flow={v}")
        }
        (ContractField::MinTemp, ContractOverride::Scalar(v)) => {
            c.min_temp = Some(v);
            format!("min_temp={v}")
        }
        (ContractField::SpeedRange, ContractOverride::Range(r)) => {
            c.speed_range = Some(r);
            format!("speed_range={},{}", r[0], r[1])
        }
        (ContractField::MonotonicZ, ContractOverride::Flag(b)) => {
            c.monotonic_z = b;
            format!("monotonic_z={b}")
        }
        // Classification guarantees field/value agree; this arm is unreachable in practice.
        (f, v) => format!("{f:?}={v:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drymachina_contracts::Contracts;
    use drymachina_kernel::gcode::{import_gcode_with_map, GcodeImportParams};

    // A tiny extruding program with two collinear moves (Safe's merge_collinear has something to do).
    const SAMPLE: &str = "G1 X0 Y0 E0\nG1 X10 Y0 E1\nG1 X20 Y0 E2\n";

    fn imported() -> drymachina_kernel::gcode::ImportedGcode {
        import_gcode_with_map(SAMPLE, &GcodeImportParams::default()).expect("import")
    }

    #[test]
    fn rewrite_safe_produces_measured_result() {
        let imp = imported();
        let action = ExecutableAction::Rewrite {
            mode: OptimizeMode::Safe,
        };
        let r = apply_executable(&action, &imp, &Contracts::default(), None, 5.0);
        assert_eq!(r.action, "rewrite-gcode --mode safe");
        // before/after are populated; verdict is one of the rewrite verdicts (not Informational).
        assert!(r.before.total_time_s >= 0.0 && r.after.total_time_s >= 0.0);
        assert!(!matches!(r.verdict, Verdict::Informational));
    }

    #[test]
    fn contract_override_is_informational_and_same_toolpath() {
        let imp = imported();
        // Tighten max_flow to an impossibly low value so the same toolpath now produces findings.
        let action = ExecutableAction::Contract {
            field: ContractField::MaxFlow,
            override_: ContractOverride::Scalar(0.0001),
        };
        let r = apply_executable(&action, &imp, &Contracts::default(), None, 5.0);
        assert!(matches!(r.verdict, Verdict::Informational));
        // toolpath unchanged → time/flow identical before vs after.
        assert!((r.before.total_time_s - r.after.total_time_s).abs() < 1e-9);
        assert!(r.after.findings >= r.before.findings);
    }

    fn rec(
        kind: ActionKind,
        mode: Option<&str>,
        field: Option<&str>,
        value: Option<&str>,
    ) -> Recommendation {
        Recommendation {
            title: "t".into(),
            rationale: "r".into(),
            expected_effect: "e".into(),
            priority: 1,
            action_kind: kind,
            mode: mode.map(String::from),
            field: field.map(String::from),
            value: value.map(String::from),
        }
    }

    #[test]
    fn rewrite_balanced_is_executable() {
        let c = classify(&rec(ActionKind::Rewrite, Some("balanced"), None, None));
        assert!(matches!(
            c,
            Classified::Executable(ExecutableAction::Rewrite {
                mode: OptimizeMode::Balanced
            })
        ));
    }

    #[test]
    fn rewrite_without_mode_is_advisory() {
        assert!(matches!(
            classify(&rec(ActionKind::Rewrite, None, None, None)),
            Classified::Advisory(_)
        ));
    }

    #[test]
    fn contract_max_flow_is_executable() {
        let c = classify(&rec(
            ActionKind::Contract,
            None,
            Some("max_flow"),
            Some("12"),
        ));
        match c {
            Classified::Executable(ExecutableAction::Contract {
                field: ContractField::MaxFlow,
                override_,
            }) => {
                assert!(
                    matches!(override_, ContractOverride::Scalar(v) if (v - 12.0).abs() < 1e-9)
                );
            }
            other => panic!("expected executable max_flow, got {other:?}"),
        }
    }

    #[test]
    fn contract_speed_range_parses_pair() {
        let c = classify(&rec(
            ActionKind::Contract,
            None,
            Some("speed_range"),
            Some("300,3000"),
        ));
        assert!(matches!(c,
            Classified::Executable(ExecutableAction::Contract { override_: ContractOverride::Range([a, b]), .. })
            if (a - 300.0).abs() < 1e-9 && (b - 3000.0).abs() < 1e-9));
    }

    #[test]
    fn unknown_field_is_advisory() {
        assert!(matches!(
            classify(&rec(
                ActionKind::Contract,
                None,
                Some("infill_density"),
                Some("40")
            )),
            Classified::Advisory(_)
        ));
    }

    #[test]
    fn unparsable_value_is_advisory() {
        assert!(matches!(
            classify(&rec(
                ActionKind::Contract,
                None,
                Some("max_flow"),
                Some("fast")
            )),
            Classified::Advisory(_)
        ));
    }

    #[test]
    fn advisory_kind_is_advisory() {
        assert!(matches!(
            classify(&rec(ActionKind::Advisory, None, None, None)),
            Classified::Advisory(_)
        ));
    }

    #[test]
    fn rewrite_safe_is_executable() {
        let c = classify(&rec(ActionKind::Rewrite, Some("safe"), None, None));
        assert!(matches!(
            c,
            Classified::Executable(ExecutableAction::Rewrite {
                mode: OptimizeMode::Safe
            })
        ));
    }

    #[test]
    fn rewrite_max_is_executable() {
        let c = classify(&rec(ActionKind::Rewrite, Some("max"), None, None));
        assert!(matches!(
            c,
            Classified::Executable(ExecutableAction::Rewrite {
                mode: OptimizeMode::Max
            })
        ));
    }

    #[test]
    fn contract_min_temp_is_executable() {
        let c = classify(&rec(
            ActionKind::Contract,
            None,
            Some("min_temp"),
            Some("210"),
        ));
        match c {
            Classified::Executable(ExecutableAction::Contract {
                field: ContractField::MinTemp,
                override_,
            }) => {
                assert!(
                    matches!(override_, ContractOverride::Scalar(v) if (v - 210.0).abs() < 1e-9)
                );
            }
            other => panic!("expected executable min_temp, got {other:?}"),
        }
    }

    #[test]
    fn contract_monotonic_z_true_is_executable() {
        let c = classify(&rec(
            ActionKind::Contract,
            None,
            Some("monotonic_z"),
            Some("true"),
        ));
        assert!(matches!(
            c,
            Classified::Executable(ExecutableAction::Contract {
                field: ContractField::MonotonicZ,
                override_: ContractOverride::Flag(true),
            })
        ));
    }
}

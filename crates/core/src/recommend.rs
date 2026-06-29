//! Recommendation schema shared between the LLM client (`dry-llm`, which deserialises the model's
//! structured output into [`Recommendation`]) and the deterministic executor in [`crate::recommend`].
//! `classify` is the honesty boundary: a recommendation is **executable** only if it names a change
//! `dry` can actually run and re-verify (a rewrite mode or one of the v1 contract fields); everything
//! else is **advisory** — an unverified hypothesis the user applies in their slicer.

use crate::optimize::OptimizeMode;
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
        ContractField::SpeedRange => match crate::verify::parse_speed_range_csv(raw) {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}

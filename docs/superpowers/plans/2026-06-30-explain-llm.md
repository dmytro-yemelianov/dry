# `dry explain --llm` (online closed loop) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an online `dry explain --llm --model <id>` path that calls the Claude Messages API, gets structured recommendations, and *closes the loop* — dry applies the executable ones (rewrite modes / contract overrides), re-traces + re-verifies, and reports measured before/after numbers with a gate verdict; advisory (re-slice-only) recommendations are marked unverified.

**Architecture:** A new feature-gated crate `dry-llm` holds the *only* network code (raw HTTP via `ureq` against `POST /v1/messages`). `dry-core` stays pure and gains the shared recommendation/result types, a `classify` function (executable vs advisory), and a deterministic `apply_executable` executor that runs existing transforms (`apply_gated` rewrite modes / `verify` contract overrides) and measures the delta. `dry-cli` orchestrates behind a `llm` cargo feature.

**Tech Stack:** Rust (workspace: `crates/core`, `crates/cli`, new `crates/llm`), `clap` (derive), `serde`/`serde_json`, `ureq` (blocking HTTP). No async runtime. Anthropic Messages API: `POST https://api.anthropic.com/v1/messages`, headers `x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`; structured output via `output_config.format`.

## Global Constraints

- **`dry-core` must stay pure** — no HTTP, no async, no `ureq`/`reqwest`/`tokio`. The only network code lives in `crates/llm`.
- **Feature-gated** — all `--llm` code in `dry-cli` is behind `#[cfg(feature = "llm")]`; the default `cargo build`/`cargo test` (no `--features llm`) must keep compiling and must not pull a TLS/HTTP stack. The `dry-llm` dependency in `dry-cli` is `optional = true`.
- **No default model** — `--model` is required whenever `--llm` is set; error (via `die`) if absent. Never call the API without an explicit model.
- **API key from `ANTHROPIC_API_KEY` env only** — no `--api-key` flag. Missing key → `die`.
- **Workspace versions** — `version.workspace = true`, `edition.workspace = true` (2021), `license.workspace = true`, `repository.workspace = true` in the new crate, matching `crates/core/Cargo.toml`.
- **No live network in CI** — unit tests cover request encoding, response decoding, classification, executor, and cost only. Any test that hits the real API is `#[ignore]`d.
- **Model IDs (exact strings)**: `claude-opus-4-8`, `claude-opus-4-7`, `claude-opus-4-6`, `claude-sonnet-4-6`, `claude-haiku-4-5`, `claude-fable-5`. Pricing per 1M tokens (input/output): opus-4-8/4-7/4-6 = 5/25, sonnet-4-6 = 3/15, haiku-4-5 = 1/5, fable-5 = 10/50.
- **v1 executable contract fields**: `max_flow` (f64), `speed_range` (`min,max`), `min_temp` (f64), `monotonic_z` (bool). Any other `field` value classifies as **advisory** (honest demotion). Rewrite modes: `safe`, `balanced`, `max`.
- **Commit cadence** — one commit per task (after its tests pass).

---

### Task 1: Shared recommendation types + classifier (`dry-core`)

**Files:**
- Create: `crates/core/src/recommend.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod recommend;` near the other `pub mod` lines ~14-28, and a `pub use recommend::{...}` re-export near the other `pub use` lines)
- Test: inline `#[cfg(test)] mod tests` in `crates/core/src/recommend.rs`

**Interfaces:**
- Consumes: `crate::OptimizeMode` (from `crate::optimize`).
- Produces:
  - `pub struct Recommendation { pub title: String, pub rationale: String, pub expected_effect: String, pub priority: i64, pub action_kind: ActionKind, pub mode: Option<String>, pub field: Option<String>, pub value: Option<String> }` (derives `Debug, Clone, Serialize, Deserialize`).
  - `pub enum ActionKind { Rewrite, Contract, Advisory }` (serde `rename_all = "lowercase"`).
  - `pub enum ExecutableAction { Rewrite { mode: OptimizeMode }, Contract { field: ContractField, value: f64 } }` plus a `SpeedRange([f64;2])` / `MonotonicZ(bool)` carrier — see code (a `ContractOverride` enum holds the parsed value).
  - `pub enum ContractField { MaxFlow, SpeedRange, MinTemp, MonotonicZ }`.
  - `pub enum Classified { Executable(ExecutableAction), Advisory(String) }`.
  - `pub fn classify(rec: &Recommendation) -> Classified`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/core/src/recommend.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn rec(kind: ActionKind, mode: Option<&str>, field: Option<&str>, value: Option<&str>) -> Recommendation {
        Recommendation {
            title: "t".into(), rationale: "r".into(), expected_effect: "e".into(), priority: 1,
            action_kind: kind, mode: mode.map(String::from),
            field: field.map(String::from), value: value.map(String::from),
        }
    }

    #[test]
    fn rewrite_balanced_is_executable() {
        let c = classify(&rec(ActionKind::Rewrite, Some("balanced"), None, None));
        assert!(matches!(c, Classified::Executable(ExecutableAction::Rewrite { mode: OptimizeMode::Balanced })));
    }

    #[test]
    fn rewrite_without_mode_is_advisory() {
        assert!(matches!(classify(&rec(ActionKind::Rewrite, None, None, None)), Classified::Advisory(_)));
    }

    #[test]
    fn contract_max_flow_is_executable() {
        let c = classify(&rec(ActionKind::Contract, None, Some("max_flow"), Some("12")));
        match c {
            Classified::Executable(ExecutableAction::Contract { field: ContractField::MaxFlow, override_ }) => {
                assert!(matches!(override_, ContractOverride::Scalar(v) if (v - 12.0).abs() < 1e-9));
            }
            other => panic!("expected executable max_flow, got {other:?}"),
        }
    }

    #[test]
    fn contract_speed_range_parses_pair() {
        let c = classify(&rec(ActionKind::Contract, None, Some("speed_range"), Some("300,3000")));
        assert!(matches!(c,
            Classified::Executable(ExecutableAction::Contract { override_: ContractOverride::Range([a, b]), .. })
            if (a - 300.0).abs() < 1e-9 && (b - 3000.0).abs() < 1e-9));
    }

    #[test]
    fn unknown_field_is_advisory() {
        assert!(matches!(classify(&rec(ActionKind::Contract, None, Some("infill_density"), Some("40"))), Classified::Advisory(_)));
    }

    #[test]
    fn unparsable_value_is_advisory() {
        assert!(matches!(classify(&rec(ActionKind::Contract, None, Some("max_flow"), Some("fast"))), Classified::Advisory(_)));
    }

    #[test]
    fn advisory_kind_is_advisory() {
        assert!(matches!(classify(&rec(ActionKind::Advisory, None, None, None)), Classified::Advisory(_)));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dry-core recommend`
Expected: FAIL — `recommend.rs` / `classify` not yet defined (compile error).

- [ ] **Step 3: Write the implementation**

Create `crates/core/src/recommend.rs` (above the test module):

```rust
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
    Rewrite { mode: OptimizeMode },
    Contract { field: ContractField, override_: ContractOverride },
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
            Some("safe") => Classified::Executable(ExecutableAction::Rewrite { mode: OptimizeMode::Safe }),
            Some("balanced") => Classified::Executable(ExecutableAction::Rewrite { mode: OptimizeMode::Balanced }),
            Some("max") => Classified::Executable(ExecutableAction::Rewrite { mode: OptimizeMode::Max }),
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
        Some(other) => return Classified::Advisory(format!("contract field `{other}` is not executable in v1")),
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
```

Then in `crates/core/src/lib.rs`, add `pub mod recommend;` with the other module declarations and:

```rust
pub use recommend::{
    classify, ActionKind, Classified, ContractField, ContractOverride, ExecutableAction,
    Recommendation,
};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dry-core recommend`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/recommend.rs crates/core/src/lib.rs
git commit -m "feat(core): recommendation schema + executable/advisory classifier"
```

---

### Task 2: Deterministic executor `apply_executable` (`dry-core`)

**Files:**
- Modify: `crates/core/src/recommend.rs` (append the executor + its types)
- Modify: `crates/core/src/lib.rs` (extend the `pub use recommend::{...}` with the new names)
- Test: inline tests in `crates/core/src/recommend.rs`

**Interfaces:**
- Consumes: `crate::gcode::ImportedGcode` (fields `toolpath: Toolpath`, method `motion_spans()`), `crate::ir::Toolpath`, `crate::optimize::{apply_gated, OptimizeMode}`, `crate::profile::MachineKinematics`, `crate::verify::{verify, Contracts, Severity}`, `crate::trace::trace_summary`. From Task 1: `ExecutableAction`, `ContractField`, `ContractOverride`.
- Produces:
  - `pub struct MetricSnapshot { pub total_time_s: f64, pub max_flow_mm3_s: f64, pub findings: usize, pub error_count: usize }` (derives `Debug, Clone, Serialize, Deserialize`).
  - `pub enum Verdict { Improved, CleanNoGain, Regressed, Informational }` (serde `rename_all = "snake_case"`).
  - `pub struct ExecutionResult { pub action: String, pub before: MetricSnapshot, pub after: MetricSnapshot, pub verdict: Verdict, pub note: String }` (derives `Debug, Clone, Serialize, Deserialize`).
  - `pub fn apply_executable(action: &ExecutableAction, imported: &ImportedGcode, contracts: &Contracts, kinematics: Option<&MachineKinematics>, window_s: f64) -> ExecutionResult`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/core/src/recommend.rs`:

```rust
    use crate::gcode::{import_gcode, GcodeImportParams};
    use crate::verify::Contracts;

    // A tiny extruding program with two collinear moves (Safe's merge_collinear has something to do).
    const SAMPLE: &str = "G1 X0 Y0 E0\nG1 X10 Y0 E1\nG1 X20 Y0 E2\n";

    fn imported() -> crate::gcode::ImportedGcode {
        import_gcode(SAMPLE, &GcodeImportParams::default()).expect("import")
    }

    #[test]
    fn rewrite_safe_produces_measured_result() {
        let imp = imported();
        let action = ExecutableAction::Rewrite { mode: OptimizeMode::Safe };
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dry-core recommend`
Expected: FAIL — `apply_executable` / `MetricSnapshot` not defined.

- [ ] **Step 3: Write the implementation**

Append to `crates/core/src/recommend.rs` (before the test module):

```rust
use crate::gcode::ImportedGcode;
use crate::ir::Toolpath;
use crate::optimize::apply_gated;
use crate::profile::MachineKinematics;
use crate::trace::trace_summary;
use crate::verify::{verify, Contracts, Severity};

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
    let error_count = report.findings.iter().filter(|f| f.severity == Severity::Error).count();
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
                before.total_time_s, after.total_time_s, before.max_flow_mm3_s, after.max_flow_mm3_s
            );
            ExecutionResult { action: format!("rewrite-gcode --mode {label}"), before, after, verdict, note }
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
fn apply_contract_override(c: &mut Contracts, field: ContractField, value: ContractOverride) -> String {
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
```

Extend the `pub use recommend::{...}` in `crates/core/src/lib.rs`:

```rust
pub use recommend::{
    apply_executable, classify, ActionKind, Classified, ContractField, ContractOverride,
    ExecutableAction, ExecutionResult, MetricSnapshot, Recommendation, Verdict,
};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dry-core recommend`
Expected: PASS (9 tests).

- [ ] **Step 5: Confirm the whole core crate still builds clean**

Run: `cargo test -p dry-core`
Expected: PASS (all existing tests + the new ones).

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/recommend.rs crates/core/src/lib.rs
git commit -m "feat(core): apply_executable — run a recommendation and measure the delta"
```

---

### Task 3: New `dry-llm` crate + request builder

**Files:**
- Create: `crates/llm/Cargo.toml`
- Create: `crates/llm/src/lib.rs`
- Modify: `Cargo.toml` (workspace `members` — add `"crates/llm"`)
- Test: inline tests in `crates/llm/src/lib.rs`

**Interfaces:**
- Consumes: `dry_core::ExplainBundle` (fields `prompt: String`, `reports: ExplainReports`).
- Produces:
  - `pub struct ClientConfig { pub api_key: String, pub model: String, pub max_tokens: u32 }`
  - `pub fn build_request(cfg: &ClientConfig, bundle: &dry_core::ExplainBundle) -> serde_json::Value`
  - `pub const RECOMMENDATIONS_SCHEMA: &str` (the JSON-schema string embedded in `output_config.format`).

- [ ] **Step 1: Create the crate manifest and register it**

Create `crates/llm/Cargo.toml`:

```toml
[package]
name = "dry-llm"
description = "Dry's Anthropic Messages client — the only network code in the workspace."
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
dry-core = { path = "../core" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
ureq = { version = "2", features = ["json", "tls"] }
```

Edit the workspace `Cargo.toml` `members` line to:

```toml
members = ["crates/core", "crates/cli", "crates/llm"]
```

- [ ] **Step 2: Write the failing test**

Create `crates/llm/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use dry_core::{build_explain_bundle, ExplainReports};

    fn sample_bundle() -> dry_core::ExplainBundle {
        // Build a minimal bundle from real reports so the test exercises the actual types.
        use dry_core::{forensics_analyze, import_gcode, simulate, trace_summary_with_sources, verify,
                       Contracts, GcodeImportParams, ReviewReport, TraceReport};
        let imp = import_gcode("G1 X0 Y0 E0\nG1 X10 Y0 E1\n", &GcodeImportParams::default()).unwrap();
        let metrics = simulate(&imp.toolpath);
        let report = verify(&imp.toolpath, &Contracts::default());
        let review = ReviewReport::build(None, None, imp.toolpath.segments.len(), metrics, &report, |_| None);
        let sources: Vec<_> = imp.segment_source_lines.iter().copied().map(Some).collect();
        let trace = trace_summary_with_sources(&imp.toolpath, 5.0, &sources).unwrap();
        let trace_report = TraceReport { file: None, profile: None, trace };
        let forensics = forensics_analyze(&imp);
        build_explain_bundle(None, None, false, ExplainReports { trace: trace_report, forensics, verify: review })
    }

    #[test]
    fn request_has_model_system_and_structured_format() {
        let cfg = ClientConfig { api_key: "k".into(), model: "claude-sonnet-4-6".into(), max_tokens: 4096 };
        let body = build_request(&cfg, &sample_bundle());
        assert_eq!(body["model"], "claude-sonnet-4-6");
        assert_eq!(body["max_tokens"], 4096);
        // The curated prompt is the system message.
        assert!(body["system"].as_str().unwrap().contains("process engineer"));
        // Structured output is requested.
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        assert!(body["output_config"]["format"]["schema"]["properties"]["recommendations"].is_object());
        // The reports JSON rides in the user message.
        let user = body["messages"][0]["content"].as_str().unwrap();
        assert!(user.contains("\"trace\"") && user.contains("\"forensics\"") && user.contains("\"verify\""));
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p dry-llm`
Expected: FAIL — `build_request` / `ClientConfig` not defined.

- [ ] **Step 4: Write the implementation**

Prepend to `crates/llm/src/lib.rs` (above the test module):

```rust
//! `dry-llm` — the only network code in the workspace. A thin, blocking Anthropic *Messages* client
//! (`ureq`) that sends a [`dry_core::ExplainBundle`] (the curated prompt + the deterministic reports)
//! and gets back structured recommendations the engine then gates. No async runtime.

use serde::Deserialize;

/// Connection + model parameters for one call.
pub struct ClientConfig {
    pub api_key: String,
    pub model: String,
    pub max_tokens: u32,
}

/// JSON schema embedded in `output_config.format` to force machine-actionable recommendations.
/// Flat (`additionalProperties: false`), within structured-output limits (no recursion/constraints).
pub const RECOMMENDATIONS_SCHEMA: &str = r#"{
  "type": "object",
  "additionalProperties": false,
  "required": ["summary", "time_analysis", "risks", "recommendations"],
  "properties": {
    "summary": {"type": "string"},
    "time_analysis": {"type": "string"},
    "risks": {"type": "string"},
    "recommendations": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["title", "rationale", "expected_effect", "priority", "action_kind"],
        "properties": {
          "title": {"type": "string"},
          "rationale": {"type": "string"},
          "expected_effect": {"type": "string"},
          "priority": {"type": "integer"},
          "action_kind": {"type": "string", "enum": ["rewrite", "contract", "advisory"]},
          "mode": {"type": "string", "enum": ["safe", "balanced", "max"]},
          "field": {"type": "string"},
          "value": {"type": "string"}
        }
      }
    }
  }
}"#;

/// Build the `POST /v1/messages` request body. Pure — no network.
pub fn build_request(cfg: &ClientConfig, bundle: &dry_core::ExplainBundle) -> serde_json::Value {
    let reports = serde_json::to_string(&bundle.reports).unwrap_or_default();
    let schema: serde_json::Value = serde_json::from_str(RECOMMENDATIONS_SCHEMA).expect("schema is valid JSON");
    serde_json::json!({
        "model": cfg.model,
        "max_tokens": cfg.max_tokens,
        "system": bundle.prompt,
        "messages": [
            { "role": "user", "content": format!("Here are the deterministic reports as JSON:\n\n{reports}") }
        ],
        "output_config": { "format": { "type": "json_schema", "schema": schema } }
    })
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p dry-llm`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/llm/Cargo.toml crates/llm/src/lib.rs
git commit -m "feat(llm): new dry-llm crate + Messages request builder"
```

---

### Task 4: `dry-llm` response decoder

**Files:**
- Modify: `crates/llm/src/lib.rs`
- Test: inline tests

**Interfaces:**
- Consumes: `dry_core::Recommendation` (deserialised from the model's structured text block).
- Produces:
  - `pub struct Usage { pub input_tokens: u64, pub output_tokens: u64 }` (derive `Debug, Clone, Deserialize`).
  - `pub struct AnalysisResponse { pub summary: String, pub time_analysis: String, pub risks: String, pub recommendations: Vec<dry_core::Recommendation>, pub usage: Usage }`.
  - `pub enum LlmError { MissingKey, Http(u16, String), Refusal(String), Decode(String), Transport(String) }` (derive `Debug`; impl `Display` + `std::error::Error`).
  - `pub fn decode_response(body: &serde_json::Value) -> Result<AnalysisResponse, LlmError>`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/llm/src/lib.rs`:

```rust
    #[test]
    fn decodes_structured_success() {
        let analysis = serde_json::json!({
            "summary": "PLA benchy", "time_analysis": "travel-bound", "risks": "none",
            "recommendations": [{
                "title": "Reorder travel", "rationale": "lots of travel", "expected_effect": "-15% time",
                "priority": 1, "action_kind": "rewrite", "mode": "max"
            }]
        }).to_string();
        let body = serde_json::json!({
            "stop_reason": "end_turn",
            "content": [{ "type": "text", "text": analysis }],
            "usage": { "input_tokens": 4210, "output_tokens": 905 }
        });
        let r = decode_response(&body).expect("decode");
        assert_eq!(r.summary, "PLA benchy");
        assert_eq!(r.recommendations.len(), 1);
        assert_eq!(r.usage.input_tokens, 4210);
    }

    #[test]
    fn refusal_is_an_error_not_a_panic() {
        let body = serde_json::json!({
            "stop_reason": "refusal",
            "stop_details": { "category": "cyber" },
            "content": []
        });
        assert!(matches!(decode_response(&body), Err(LlmError::Refusal(_))));
    }

    #[test]
    fn malformed_content_is_decode_error() {
        let body = serde_json::json!({
            "stop_reason": "end_turn",
            "content": [{ "type": "text", "text": "not json" }],
            "usage": { "input_tokens": 1, "output_tokens": 1 }
        });
        assert!(matches!(decode_response(&body), Err(LlmError::Decode(_))));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p dry-llm`
Expected: FAIL — `decode_response` / `LlmError` not defined.

- [ ] **Step 3: Write the implementation**

Add to `crates/llm/src/lib.rs` (above the test module):

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct AnalysisResponse {
    pub summary: String,
    pub time_analysis: String,
    pub risks: String,
    pub recommendations: Vec<dry_core::Recommendation>,
    pub usage: Usage,
}

#[derive(Debug)]
pub enum LlmError {
    MissingKey,
    Http(u16, String),
    Refusal(String),
    Decode(String),
    Transport(String),
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmError::MissingKey => write!(f, "set ANTHROPIC_API_KEY to use --llm"),
            LlmError::Http(code, body) => write!(f, "Anthropic API returned HTTP {code}: {body}"),
            LlmError::Refusal(cat) => write!(f, "model declined the request (category: {cat})"),
            LlmError::Decode(msg) => write!(f, "could not parse the model response: {msg}"),
            LlmError::Transport(msg) => write!(f, "network error calling the Anthropic API: {msg}"),
        }
    }
}
impl std::error::Error for LlmError {}

#[derive(Deserialize)]
struct StructuredAnalysis {
    summary: String,
    time_analysis: String,
    risks: String,
    recommendations: Vec<dry_core::Recommendation>,
}

/// Parse a `POST /v1/messages` response body into an [`AnalysisResponse`]. Pure — no network.
pub fn decode_response(body: &serde_json::Value) -> Result<AnalysisResponse, LlmError> {
    if body["stop_reason"] == "refusal" {
        let category = body["stop_details"]["category"].as_str().unwrap_or("unspecified");
        return Err(LlmError::Refusal(category.to_string()));
    }
    let text = body["content"]
        .as_array()
        .and_then(|blocks| blocks.iter().find(|b| b["type"] == "text"))
        .and_then(|b| b["text"].as_str())
        .ok_or_else(|| LlmError::Decode("response had no text content block".into()))?;
    let analysis: StructuredAnalysis =
        serde_json::from_str(text).map_err(|e| LlmError::Decode(format!("{e}: {text}")))?;
    let usage: Usage = serde_json::from_value(body["usage"].clone()).unwrap_or(Usage {
        input_tokens: 0,
        output_tokens: 0,
    });
    Ok(AnalysisResponse {
        summary: analysis.summary,
        time_analysis: analysis.time_analysis,
        risks: analysis.risks,
        recommendations: analysis.recommendations,
        usage,
    })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p dry-llm`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/llm/src/lib.rs
git commit -m "feat(llm): decode structured response, handle refusal + parse errors"
```

---

### Task 5: `dry-llm` network call + cost helper

**Files:**
- Modify: `crates/llm/src/lib.rs`
- Test: inline tests (cost only; the network call gets an `#[ignore]`d smoke test)

**Interfaces:**
- Produces:
  - `pub fn analyze(cfg: &ClientConfig, bundle: &dry_core::ExplainBundle) -> Result<AnalysisResponse, LlmError>` (the only function that touches the network).
  - `pub fn cost_usd(model: &str, usage: &Usage) -> Option<f64>`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/llm/src/lib.rs`:

```rust
    #[test]
    fn cost_known_model() {
        let u = Usage { input_tokens: 1_000_000, output_tokens: 1_000_000 };
        let c = cost_usd("claude-sonnet-4-6", &u).unwrap();
        assert!((c - 18.0).abs() < 1e-9, "1M in @ $3 + 1M out @ $15 = $18, got {c}");
    }

    #[test]
    fn cost_unknown_model_is_none() {
        let u = Usage { input_tokens: 10, output_tokens: 10 };
        assert!(cost_usd("some-future-model", &u).is_none());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p dry-llm`
Expected: FAIL — `cost_usd` not defined.

- [ ] **Step 3: Write the implementation**

Add to `crates/llm/src/lib.rs` (above the test module):

```rust
/// Per-1M-token (input, output) USD pricing, keyed by exact model id.
fn price_per_mtok(model: &str) -> Option<(f64, f64)> {
    match model {
        "claude-opus-4-8" | "claude-opus-4-7" | "claude-opus-4-6" => Some((5.0, 25.0)),
        "claude-sonnet-4-6" => Some((3.0, 15.0)),
        "claude-haiku-4-5" => Some((1.0, 5.0)),
        "claude-fable-5" => Some((10.0, 50.0)),
        _ => None,
    }
}

/// Estimated USD cost for a call, or `None` for an unknown model.
pub fn cost_usd(model: &str, usage: &Usage) -> Option<f64> {
    let (in_rate, out_rate) = price_per_mtok(model)?;
    Some((usage.input_tokens as f64 / 1e6) * in_rate + (usage.output_tokens as f64 / 1e6) * out_rate)
}

/// Send the bundle to the Anthropic Messages API and decode the structured reply.
/// This is the only function in the workspace that performs network I/O.
pub fn analyze(cfg: &ClientConfig, bundle: &dry_core::ExplainBundle) -> Result<AnalysisResponse, LlmError> {
    if cfg.api_key.is_empty() {
        return Err(LlmError::MissingKey);
    }
    let body = build_request(cfg, bundle);
    let resp = ureq::post("https://api.anthropic.com/v1/messages")
        .set("x-api-key", &cfg.api_key)
        .set("anthropic-version", "2023-06-01")
        .set("content-type", "application/json")
        .send_json(body);
    match resp {
        Ok(r) => {
            let json: serde_json::Value = r
                .into_json()
                .map_err(|e| LlmError::Decode(format!("invalid JSON from API: {e}")))?;
            decode_response(&json)
        }
        Err(ureq::Error::Status(code, r)) => {
            let snippet = r.into_string().unwrap_or_default();
            Err(LlmError::Http(code, snippet.chars().take(500).collect()))
        }
        Err(ureq::Error::Transport(t)) => Err(LlmError::Transport(t.to_string())),
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p dry-llm`
Expected: PASS (cost tests; no network hit).

- [ ] **Step 5: Confirm the workspace builds**

Run: `cargo build`
Expected: clean build of all default-member crates (core, cli, llm).

- [ ] **Step 6: Commit**

```bash
git add crates/llm/src/lib.rs
git commit -m "feat(llm): analyze() network call + per-model cost estimate"
```

---

### Task 6: `llm` cargo feature + `--llm` flags on `explain`

**Files:**
- Modify: `crates/cli/Cargo.toml` (optional `dry-llm` dep + `[features]`)
- Modify: `crates/cli/src/main.rs` (add `--llm`, `--model`, `--max-applies` to the `Explain` variant ~`crates/cli/src/main.rs:204-242`)
- Test: a `--help`/build check (no behavioural test here; behaviour lands in Task 7)

**Interfaces:**
- Consumes: existing `Cmd::Explain { .. }` clap struct.
- Produces: three new fields on `Cmd::Explain`: `llm: bool`, `model: Option<String>`, `max_applies: usize`.

- [ ] **Step 1: Add the optional dependency and feature**

Edit `crates/cli/Cargo.toml` to add under `[dependencies]`:

```toml
dry-llm = { path = "../llm", optional = true }
```

and add a new section:

```toml
[features]
llm = ["dep:dry-llm"]
```

- [ ] **Step 2: Add the CLI flags**

In `crates/cli/src/main.rs`, inside the `Explain { .. }` variant (after the existing `out: Option<String>` field, before the closing `},`), add:

```rust
        /// Call Claude directly: build the bundle, get recommendations, apply the executable ones,
        /// and report measured before/after results. Requires --model and ANTHROPIC_API_KEY.
        #[arg(long)]
        llm: bool,
        /// Claude model id for --llm (e.g. claude-sonnet-4-6, claude-opus-4-8). Required with --llm.
        #[arg(long)]
        model: Option<String>,
        /// Cap on how many executable recommendations --llm actually applies (highest priority first).
        #[arg(long, default_value_t = 4)]
        max_applies: usize,
```

- [ ] **Step 3: Thread the new fields through the existing handler binding**

In the `Cmd::Explain { .. } =>` match arm (~`crates/cli/src/main.rs:806-820`), add the three new names to the destructuring so the arm compiles, and short-circuit to the new path when `llm` is set. At the top of the arm body insert:

```rust
            if llm {
                return run_explain_llm(ExplainLlmArgs {
                    file, profile, filament_diameter, line_width, layer_height, max_flow, bounds,
                    monotonic_z, min_temp, speed_range, window_s, json, out,
                    model, max_applies,
                });
            }
```

and add `llm,`, `model,`, `max_applies,` to the destructuring pattern of the arm. (`run_explain_llm` and `ExplainLlmArgs` are defined in Task 7; for this task add a temporary stub so it compiles — see Step 4.)

- [ ] **Step 4: Add a compile stub for the Task-7 entry point**

At the end of `crates/cli/src/main.rs`, add:

```rust
struct ExplainLlmArgs {
    file: String,
    profile: Option<String>,
    filament_diameter: Option<f64>,
    line_width: Option<f64>,
    layer_height: Option<f64>,
    max_flow: Option<f64>,
    bounds: Option<String>,
    monotonic_z: bool,
    min_temp: Option<f64>,
    speed_range: Option<String>,
    window_s: f64,
    json: bool,
    out: Option<String>,
    model: Option<String>,
    max_applies: usize,
}

#[cfg(not(feature = "llm"))]
fn run_explain_llm(_args: ExplainLlmArgs) -> std::process::ExitCode {
    die("this build was compiled without --llm support; rebuild with `cargo build --features llm`".into())
}
```

- [ ] **Step 5: Verify both build configurations compile**

Run: `cargo build` (default, no llm)
Expected: clean build; `dry explain --llm` would exit with the "compiled without --llm support" message.

Run: `cargo build --features llm`
Expected: build fails *only* with "cannot find function `run_explain_llm` for the `llm` cfg" — that arm is delivered in Task 7. (If you prefer a green build here, also add a `#[cfg(feature = "llm")] fn run_explain_llm(_: ExplainLlmArgs) -> std::process::ExitCode { unimplemented!() }` stub and replace it in Task 7.)

- [ ] **Step 6: Commit**

```bash
git add crates/cli/Cargo.toml crates/cli/src/main.rs
git commit -m "feat(cli): llm cargo feature + --llm/--model/--max-applies flags on explain"
```

---

### Task 7: Orchestration + rendering (`run_explain_llm`)

**Files:**
- Modify: `crates/cli/src/main.rs` (replace the `#[cfg(feature = "llm")]` stub with the real `run_explain_llm`)
- Test: manual verification (network) + `cargo test` for the unchanged default build

**Interfaces:**
- Consumes: `dry_llm::{analyze, cost_usd, ClientConfig, LlmError}`; `dry_core::{classify, apply_executable, Classified, ExecutionResult, ExecutableAction}`; existing CLI helpers `load_profile`, `gcode_review_params`, `contracts_from_inputs`, `profile_label`, `import_gcode_reader_with_map`, `simulate`, `verify`, `trace_summary_with_sources`, `forensics_analyze`, `build_explain_bundle`, `ReviewReport::build`, `die`.
- Produces: `#[cfg(feature = "llm")] fn run_explain_llm(args: ExplainLlmArgs) -> std::process::ExitCode`.

- [ ] **Step 1: Write the real handler**

Replace the Task-6 stub (keep the `#[cfg(not(feature = "llm"))]` one) with, in `crates/cli/src/main.rs`:

```rust
#[cfg(feature = "llm")]
fn run_explain_llm(args: ExplainLlmArgs) -> std::process::ExitCode {
    use dry_core::{apply_executable, classify, Classified};

    let model = args.model.unwrap_or_else(|| {
        die("--llm requires --model <id> (e.g. --model claude-sonnet-4-6)".into())
    });
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .unwrap_or_else(|_| die("set ANTHROPIC_API_KEY to use --llm".into()));

    // 1. Build the bundle exactly as the offline path does.
    let input = std::fs::File::open(&args.file)
        .unwrap_or_else(|e| die(format!("cannot read {}: {e}", args.file)));
    let profile = load_profile(args.profile.as_deref());
    let params = gcode_review_params(profile.as_ref(), args.filament_diameter, args.line_width, args.layer_height);
    let imported = import_gcode_reader_with_map(input, &params)
        .unwrap_or_else(|e| die(format!("cannot import {}: {e}", args.file)));
    let metrics = simulate(&imported.toolpath);
    let profiled = profile.is_some();
    let contracts = contracts_from_inputs(
        profile.as_ref(), args.bounds.as_deref(), args.max_flow, args.speed_range.as_deref(),
        args.monotonic_z, args.min_temp,
    );
    let report = verify(&imported.toolpath, &contracts);
    let review = dry_core::ReviewReport::build(
        Some(args.file.clone()), profile_label(profile.as_ref()), imported.toolpath.segments.len(),
        metrics, &report, |segment| imported.source_line_for_segment(segment),
    );
    let source_lines: Vec<_> = imported.segment_source_lines.iter().copied().map(Some).collect();
    let trace = trace_summary_with_sources(&imported.toolpath, args.window_s, &source_lines)
        .unwrap_or_else(|e| die(format!("cannot trace {}: {e}", args.file)));
    let trace_report = dry_core::TraceReport { file: Some(args.file.clone()), profile: profile_label(profile.as_ref()), trace };
    let forensics = dry_core::forensics_analyze(&imported);
    let bundle = dry_core::build_explain_bundle(
        Some(args.file.clone()), profile_label(profile.as_ref()), profiled,
        dry_core::ExplainReports { trace: trace_report, forensics, verify: review },
    );

    // 2. Call Claude.
    let cfg = dry_llm::ClientConfig { api_key, model: model.clone(), max_tokens: 8192 };
    let analysis = dry_llm::analyze(&cfg, &bundle).unwrap_or_else(|e| die(e.to_string()));

    // 3. Cost readout (stderr).
    match dry_llm::cost_usd(&model, &analysis.usage) {
        Some(c) => eprintln!("{model} · in {} tok / out {} tok · ~${c:.4}",
                             analysis.usage.input_tokens, analysis.usage.output_tokens),
        None => eprintln!("{model} · in {} tok / out {} tok · (pricing unknown for {model})",
                          analysis.usage.input_tokens, analysis.usage.output_tokens),
    }

    // 4. Classify, then apply executable recommendations (highest priority first, capped).
    let kinematics = profile.as_ref().and_then(|p| p.machine.kinematics.as_ref());
    let mut recs: Vec<_> = analysis.recommendations.iter().collect();
    recs.sort_by_key(|r| r.priority);
    let mut results: Vec<(String, ExecutionResult)> = Vec::new();
    let mut advisories: Vec<(String, String)> = Vec::new();
    let mut applied = 0usize;
    let mut skipped = 0usize;
    for rec in &recs {
        match classify(rec) {
            Classified::Executable(action) => {
                if applied >= args.max_applies {
                    skipped += 1;
                    continue;
                }
                let result = apply_executable(&action, &imported, &contracts, kinematics, args.window_s);
                results.push((rec.title.clone(), result));
                applied += 1;
            }
            Classified::Advisory(reason) => advisories.push((rec.title.clone(), reason)),
        }
    }
    if skipped > 0 {
        eprintln!("note: {skipped} executable recommendation(s) skipped (over --max-applies {})", args.max_applies);
    }

    // 5. Render.
    let rendered = if args.json {
        let envelope = serde_json::json!({
            "meta": { "file": args.file, "model": model, "profiled": profiled },
            "analysis": { "summary": analysis.summary, "time_analysis": analysis.time_analysis, "risks": analysis.risks },
            "recommendations": analysis.recommendations,
            "results": results.iter().map(|(_, r)| r).collect::<Vec<_>>(),
            "usage": { "input_tokens": analysis.usage.input_tokens, "output_tokens": analysis.usage.output_tokens },
            "cost_usd": dry_llm::cost_usd(&model, &analysis.usage),
        });
        serde_json::to_string_pretty(&envelope).unwrap() + "\n"
    } else {
        render_llm_markdown(&args.file, &model, &analysis, &results, &advisories)
    };
    match args.out {
        Some(path) => std::fs::write(&path, rendered).unwrap_or_else(|e| die(format!("cannot write {path}: {e}"))),
        None => print!("{rendered}"),
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(feature = "llm")]
fn render_llm_markdown(
    file: &str,
    model: &str,
    analysis: &dry_llm::AnalysisResponse,
    results: &[(String, dry_core::ExecutionResult)],
    advisories: &[(String, String)],
) -> String {
    use std::fmt::Write as _;
    let mut md = String::new();
    let _ = writeln!(md, "# Dry explain --llm — {file}  (model {model})\n");
    let _ = writeln!(md, "## Summary\n\n{}\n", analysis.summary);
    let _ = writeln!(md, "## Time analysis\n\n{}\n", analysis.time_analysis);
    let _ = writeln!(md, "## Risks\n\n{}\n", analysis.risks);
    let _ = writeln!(md, "## Results — measured by dry\n");
    let _ = writeln!(md, "| Change | Status | Measured |");
    let _ = writeln!(md, "|---|---|---|");
    for (title, r) in results {
        let _ = writeln!(md, "| {title} | {} ({:?}) | {} |", r.action, r.verdict, r.note);
    }
    for (title, reason) in advisories {
        let _ = writeln!(md, "| {title} | advisory — unverified | {reason}; apply in your slicer |");
    }
    md
}
```

- [ ] **Step 2: Build both configurations**

Run: `cargo build`
Expected: clean (default build still routes `--llm` to the "compiled without --llm support" stub).

Run: `cargo build --features llm`
Expected: clean build of `dry` with the live path.

- [ ] **Step 3: Confirm the default test suite is unaffected**

Run: `cargo test`
Expected: PASS — no behavioural change to any non-llm path.

- [ ] **Step 4: Manual end-to-end verification (network — run locally with a key)**

```bash
export ANTHROPIC_API_KEY=sk-ant-...
cargo run --features llm -- explain conformance/fixtures/<some>.gcode \
  --llm --model claude-sonnet-4-6
```

Expected: stderr shows a `… · in N / out M tok · ~$X` cost line; stdout shows the Summary/Time/Risks sections followed by a Results table where any rewrite/contract recommendation has a measured `time …s -> …s` note and a verdict, and advisory rows read "unverified … apply in your slicer". (Pick any fixture under `conformance/fixtures/`; if none is g-code, generate one with `dry generate`/`dry emit` first.)

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/main.rs
git commit -m "feat(cli): explain --llm closed loop — call, classify, apply, render"
```

---

### Task 8: Docs + CHANGELOG + CI feature build

**Files:**
- Modify: `docs/15-cli-cookbook.md`
- Modify: `docs/11-profiles-and-reports.md`
- Modify: `docs/05-product-directions.md`
- Modify: `CHANGELOG.md`
- Modify: `.github/workflows/ci.yml` (add a job/step that builds + tests with `--features llm`)

**Interfaces:** none (documentation + CI).

- [ ] **Step 1: Cookbook recipe**

Add an `## explain --llm — online closed loop` section to `docs/15-cli-cookbook.md` documenting: requires `ANTHROPIC_API_KEY` and `--model`; what executable vs advisory means; that cost prints to stderr; and that to produce the improved g-code you run `rewrite-gcode --mode <winner>` (explain is an analyst). Include the command:

```
ANTHROPIC_API_KEY=… dry explain part.gcode --llm --model claude-sonnet-4-6 --profile voron-abs
```

- [ ] **Step 2: Document the `--llm --json` envelope**

In `docs/11-profiles-and-reports.md`, add a short subsection describing the `{meta, analysis, recommendations, results, usage, cost_usd}` envelope and noting it is **not** drift-gated (model output is non-deterministic), unlike the offline `ExplainBundle`.

- [ ] **Step 3: Mark Direction 4 progress**

In `docs/05-product-directions.md` §4, note the online `explain --llm` closed loop shipped (and that `compare` is the next phase).

- [ ] **Step 4: CHANGELOG**

Under `[Unreleased]` in `CHANGELOG.md`, add a bullet:

```
- `dry explain --llm --model <id>`: online path that calls the Claude Messages API and closes the loop
  — applies executable recommendations (rewrite modes / contract overrides), re-traces + re-verifies, and
  reports measured before/after with a gate verdict; advisory (re-slice-only) suggestions are marked
  unverified. New feature-gated `dry-llm` crate is the only network code; `dry-core` stays pure.
```

- [ ] **Step 5: CI builds the feature**

In `.github/workflows/ci.yml`, add a step to the existing build/test job (or a small dedicated one, `runs-on: [self-hosted, X64]`) that runs:

```yaml
      - name: build + test with llm feature
        run: |
          cargo build --features llm
          cargo test -p dry-llm
```

(`dry-llm`'s tests are all offline, so this is safe in CI.)

- [ ] **Step 6: Final full check**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test && cargo build --features llm && cargo test -p dry-llm`
Expected: all clean.

- [ ] **Step 7: Commit**

```bash
git add docs/ CHANGELOG.md .github/workflows/ci.yml
git commit -m "docs+ci: document explain --llm and build the llm feature in CI"
```

---

## Self-Review

**Spec coverage** (each design section → task):
- Crates & boundary (new `dry-llm`, pure core, feature-gated cli) → Tasks 3, 6.
- CLI surface (`--llm`/`--model`/`--max-applies`, key from env, `--json`, stderr cost) → Tasks 6, 7.
- Closed loop / data flow → Task 7 (orchestration) over Tasks 1, 2, 4, 5.
- Structured output schema → Task 3 (`RECOMMENDATIONS_SCHEMA`).
- Executor (`apply_executable`, snapshots, verdicts) → Task 2.
- `dry-llm` surface (`build_request`/`decode_response`/`analyze`/cost) → Tasks 3, 4, 5.
- Cost readout + price table → Task 5 (`cost_usd`), Task 7 (stderr line).
- Error handling (missing key/model, HTTP, refusal, parse) → Tasks 4, 5, 7.
- Determinism & testing (deterministic sub-parts tested, no live network) → Tasks 1, 2, 4, 5; manual e2e in Task 7; CI feature build in Task 8.
- Docs → Task 8. Phase 2 (`compare`) → intentionally out of scope for this plan.

**Placeholder scan:** every code step shows real code; commands have expected output. No TBD/TODO.

**Type consistency:** `classify` returns `Classified` (Task 1) consumed in Task 7; `apply_executable(action, &ImportedGcode, &Contracts, Option<&MachineKinematics>, f64) -> ExecutionResult` (Task 2) called with the same arg order in Task 7; `ExecutableAction`/`ContractField`/`ContractOverride` names match across Tasks 1, 2, 7; `build_request`/`decode_response`/`analyze`/`cost_usd`/`Usage`/`AnalysisResponse`/`ClientConfig`/`LlmError` names match across Tasks 3–5 and 7; `RECOMMENDATIONS_SCHEMA` defined in Task 3 and referenced by `build_request`. The `field` enum in the schema (Task 3) is open `string`; the executable subset (`max_flow`/`speed_range`/`min_temp`/`monotonic_z`) is enforced by `classify` (Task 1) per the Global Constraints — other fields demote to advisory, matching the spec's honesty rule.

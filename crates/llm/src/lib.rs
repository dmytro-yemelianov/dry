//! `dry-llm` — the only network code in the workspace. A thin, blocking Anthropic *Messages* client
//! (`ureq`) that sends a [`dry_core::ExplainBundle`] (the curated prompt + the deterministic reports)
//! and gets back structured recommendations the engine then gates. No async runtime.

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
    let schema: serde_json::Value =
        serde_json::from_str(RECOMMENDATIONS_SCHEMA).expect("schema is valid JSON");
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

#[cfg(test)]
mod tests {
    use super::*;
    use dry_core::{build_explain_bundle, ExplainReports};

    fn sample_bundle() -> dry_core::ExplainBundle {
        // Build a minimal bundle from real reports so the test exercises the actual types.
        // Fix vs brief: import_gcode returns Toolpath; import_gcode_with_map returns ImportedGcode
        // (which has .toolpath and .segment_source_lines fields forensics_analyze needs).
        use dry_core::{
            forensics_analyze, import_gcode_with_map, simulate, trace_summary_with_sources, verify,
            Contracts, GcodeImportParams, ReviewReport, TraceReport,
        };
        let imp =
            import_gcode_with_map("G1 X0 Y0 E0\nG1 X10 Y0 E1\n", &GcodeImportParams::default())
                .unwrap();
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
        build_explain_bundle(
            None,
            None,
            false,
            ExplainReports {
                trace: trace_report,
                forensics,
                verify: review,
            },
        )
    }

    #[test]
    fn request_has_model_system_and_structured_format() {
        let cfg = ClientConfig {
            api_key: "k".into(),
            model: "claude-sonnet-4-6".into(),
            max_tokens: 4096,
        };
        let body = build_request(&cfg, &sample_bundle());
        assert_eq!(body["model"], "claude-sonnet-4-6");
        assert_eq!(body["max_tokens"], 4096);
        // The curated prompt is the system message.
        assert!(body["system"]
            .as_str()
            .unwrap()
            .contains("process engineer"));
        // Structured output is requested.
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        assert!(
            body["output_config"]["format"]["schema"]["properties"]["recommendations"].is_object()
        );
        // The reports JSON rides in the user message.
        let user = body["messages"][0]["content"].as_str().unwrap();
        assert!(
            user.contains("\"trace\"")
                && user.contains("\"forensics\"")
                && user.contains("\"verify\"")
        );
    }
}

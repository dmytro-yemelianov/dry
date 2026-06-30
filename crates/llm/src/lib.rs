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
    Some(
        (usage.input_tokens as f64 / 1e6) * in_rate + (usage.output_tokens as f64 / 1e6) * out_rate,
    )
}

/// POST a Messages request body and return the parsed JSON response. The only network I/O in the
/// workspace; both `analyze` (explain) and `narrate_compare` (compare) go through here.
pub fn post_messages(
    cfg: &ClientConfig,
    body: serde_json::Value,
) -> Result<serde_json::Value, LlmError> {
    if cfg.api_key.is_empty() {
        return Err(LlmError::MissingKey);
    }
    match ureq::post("https://api.anthropic.com/v1/messages")
        .set("x-api-key", &cfg.api_key)
        .set("anthropic-version", "2023-06-01")
        .set("content-type", "application/json")
        .send_json(body)
    {
        Ok(r) => r
            .into_json()
            .map_err(|e| LlmError::Decode(format!("invalid JSON from API: {e}"))),
        Err(ureq::Error::Status(code, r)) => {
            let snippet = r.into_string().unwrap_or_default();
            Err(LlmError::Http(code, snippet.chars().take(500).collect()))
        }
        Err(ureq::Error::Transport(t)) => Err(LlmError::Transport(t.to_string())),
    }
}

/// Send the bundle to the Anthropic Messages API and decode the structured reply.
pub fn analyze(
    cfg: &ClientConfig,
    bundle: &dry_core::ExplainBundle,
) -> Result<AnalysisResponse, LlmError> {
    let json = post_messages(cfg, build_request(cfg, bundle))?;
    decode_response(&json)
}

/// Parse a `POST /v1/messages` response body into an [`AnalysisResponse`]. Pure — no network.
pub fn decode_response(body: &serde_json::Value) -> Result<AnalysisResponse, LlmError> {
    if body["stop_reason"] == "refusal" {
        let category = body["stop_details"]["category"]
            .as_str()
            .unwrap_or("unspecified");
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

    #[test]
    fn cost_known_model() {
        let u = Usage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
        };
        let c = cost_usd("claude-sonnet-4-6", &u).unwrap();
        assert!(
            (c - 18.0).abs() < 1e-9,
            "1M in @ $3 + 1M out @ $15 = $18, got {c}"
        );
    }

    #[test]
    fn cost_unknown_model_is_none() {
        let u = Usage {
            input_tokens: 10,
            output_tokens: 10,
        };
        assert!(cost_usd("some-future-model", &u).is_none());
    }

    #[test]
    fn post_messages_empty_key_returns_missing_key() {
        let cfg = ClientConfig {
            api_key: "".into(),
            model: "claude-sonnet-4-6".into(),
            max_tokens: 4096,
        };
        assert!(matches!(
            post_messages(&cfg, serde_json::json!({})),
            Err(LlmError::MissingKey)
        ));
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

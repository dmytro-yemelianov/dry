//! The engine half of the published TPMS option contract.
//!
//! `spec/dry-tpms-options-v1.schema.json` describes the untrusted-ingress option bundle that
//! `crates/wasm` and `py/` deserialize from raw JSON. A schema is prose unless something checks it
//! against the code it claims to describe, so the corpus in `spec/examples/tpms-options/` carries
//! two verdicts per case: the schema's (checked by `tools/validate_reports.py`, which never imports
//! dry-core) and the engine's (checked here). Neither tool can move without the other noticing.

use std::path::PathBuf;

use dry_core::{try_tpms_ops, TpmsOptions};
use serde_json::Value;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples/tpms-options")
}

/// What the engine does with one option document: the deserializer and the generator are one gate
/// from the caller's point of view, so a serde failure counts as a refusal with serde's message.
fn engine_verdict(json: &str) -> Result<usize, String> {
    let options: TpmsOptions = serde_json::from_str(json).map_err(|e| e.to_string())?;
    try_tpms_ops(&options)
        .map(|ops| ops.len())
        .map_err(|e| e.to_string())
}

#[test]
fn the_engine_agrees_with_the_published_tpms_option_corpus() {
    let dir = corpus_dir();
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    let cases = manifest["cases"].as_array().expect("cases array");
    assert!(!cases.is_empty(), "the option corpus must not be empty");

    for case in cases {
        let name = case["file"].as_str().expect("case file");
        let expected = case["engine"].as_str().expect("engine verdict");
        let body = std::fs::read_to_string(dir.join(name))
            .unwrap_or_else(|e| panic!("{name}: cannot read case: {e}"));
        match (expected, engine_verdict(&body)) {
            ("accepted", Ok(ops)) => assert!(
                ops > 0,
                "{name}: accepted bundles must emit ops; the vacuity check should have refused it"
            ),
            ("accepted", Err(message)) => {
                panic!("{name}: manifest says accepted, engine refused: {message}")
            }
            ("refused", Ok(ops)) => {
                panic!("{name}: manifest says refused, engine accepted and emitted {ops} ops")
            }
            ("refused", Err(message)) => {
                let quoted = case["refusal"]
                    .as_str()
                    .expect("refused case needs refusal text");
                assert!(
                    message.contains(quoted),
                    "{name}: refusal text drifted\n  manifest: {quoted}\n  engine:   {message}"
                );
            }
            (other, _) => panic!("{name}: unknown engine verdict {other:?}"),
        }
    }
}

/// The invariant that makes the published schema safe to hand to a caller: it never refuses a bundle
/// the engine would run. Checked from both sides — `tools/validate_reports.py` enforces it on the
/// labels, and this test proves the labels' engine column is not fiction.
#[test]
fn no_case_is_schema_invalid_while_the_engine_accepts_it() {
    let dir = corpus_dir();
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    for case in manifest["cases"].as_array().unwrap() {
        let name = case["file"].as_str().unwrap();
        if case["schema"] == "invalid" {
            let body = std::fs::read_to_string(dir.join(name)).unwrap();
            assert!(
                engine_verdict(&body).is_err(),
                "{name}: the schema refuses a bundle the engine accepts"
            );
        }
    }
}

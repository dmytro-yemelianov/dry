//! wasm-bindgen binding — the Dry engine (resolve + emit + simulate) in the browser and Node, the same
//! Rust core that powers the native CLI and the Python SDK. The L1 design crosses the boundary as JSON
//! (`[{"op":"move","x":..}, ...]`); the engine itself never depends on wasm-bindgen (this crate is
//! isolated from the core cargo workspace).

use dry_core::{
    emit, merge_collinear, resolve, simulate, verify, Contracts, Design, EmitParams, Op,
    ResolveParams,
};
use wasm_bindgen::prelude::*;

fn parse(ops_json: &str, params_json: &str) -> Result<(Design, ResolveParams), JsError> {
    let ops: Vec<Op> =
        serde_json::from_str(ops_json).map_err(|e| JsError::new(&format!("design: {e}")))?;
    let params: ResolveParams =
        serde_json::from_str(params_json).map_err(|e| JsError::new(&format!("params: {e}")))?;
    Ok((Design { ops }, params))
}

/// Resolve a design and emit motion g-code (returned as a JS array of strings).
#[wasm_bindgen]
pub fn resolve_gcode(
    ops_json: &str,
    params_json: &str,
    relative_e: bool,
) -> Result<Vec<String>, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let tp = resolve(&d, &p);
    Ok(emit(
        &tp,
        &EmitParams {
            relative_e,
            travel_g1_e0: false,
            five_axis: false,
        },
    ))
}

/// Resolve a design and return its simulation metrics as a JSON string.
#[wasm_bindgen]
pub fn resolve_metrics(ops_json: &str, params_json: &str) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    serde_json::to_string(&simulate(&resolve(&d, &p))).map_err(|e| JsError::new(&e.to_string()))
}

/// Resolve a design and return the L2 Dry IR as a JSON string.
#[wasm_bindgen]
pub fn resolve_ir(ops_json: &str, params_json: &str) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    Ok(resolve(&d, &p).to_json())
}

/// Resolve a design, optimize it (merge collinear extruding moves), and return the resulting
/// L2 Dry IR as a JSON string. Compare its `segments.len()` against [`resolve_ir`] to see how
/// many redundant moves the optimizer collapsed.
#[wasm_bindgen]
pub fn resolve_optimized_ir(ops_json: &str, params_json: &str) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    Ok(merge_collinear(&resolve(&d, &p)).to_json())
}

/// Resolve a design and verify it against machine-safety contracts, returning the JSON
/// [`dry_core::Report`] (`{"findings":[{"rule","severity","segment","message"}]}`).
///
/// The contract is supplied as scalars (wasm-bindgen + the demo are simplest with primitives):
/// `max_flow_opt` is the volumetric-flow ceiling (mm³/s) and `min_temp_opt` is the minimum
/// nozzle temperature (°C). The convention is **`0` (or any non-positive value) means unset** —
/// that check is then disabled. All other contract fields use their defaults (no bounds, no
/// speed range, Z not required monotonic).
#[wasm_bindgen]
pub fn resolve_verify(
    ops_json: &str,
    params_json: &str,
    max_flow_opt: f64,
    min_temp_opt: f64,
) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let contracts = Contracts {
        max_flow: if max_flow_opt > 0.0 {
            Some(max_flow_opt)
        } else {
            None
        },
        min_temp: if min_temp_opt > 0.0 {
            Some(min_temp_opt)
        } else {
            None
        },
        ..Contracts::default()
    };
    let report = verify(&resolve(&d, &p), &contracts);
    serde_json::to_string(&report).map_err(|e| JsError::new(&e.to_string()))
}

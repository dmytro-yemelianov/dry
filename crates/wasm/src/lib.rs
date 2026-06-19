//! wasm-bindgen binding — the Dry engine (resolve + emit + simulate) in the browser and Node, the same
//! Rust core that powers the native CLI and the Python SDK. The L1 design crosses the boundary as JSON
//! (`[{"op":"move","x":..}, ...]`); the engine itself never depends on wasm-bindgen (this crate is
//! isolated from the core cargo workspace).

use dry_core::{
    emit, optimize_pipeline, parse_bounds_csv, parse_speed_range_csv, resolve_checked, simulate,
    verify, Contracts, Design, EmitParams, Kinematics, Op, ResolveParams,
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
    travel_g1_e0: bool,
    five_axis: bool,
    kinematics_str: &str,
) -> Result<Vec<String>, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    let kinematics =
        Kinematics::named(kinematics_str).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(emit(
        &tp,
        &EmitParams {
            relative_e,
            travel_g1_e0,
            five_axis,
            kinematics,
        },
    ))
}

/// Resolve a design and return its simulation metrics as a JSON string.
#[wasm_bindgen]
pub fn resolve_metrics(ops_json: &str, params_json: &str) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&simulate(&tp)).map_err(|e| JsError::new(&e.to_string()))
}

/// Resolve a design and return the L2 Dry IR as a JSON string.
#[wasm_bindgen]
pub fn resolve_ir(ops_json: &str, params_json: &str) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    Ok(resolve_checked(&d, &p)
        .map_err(|e| JsError::new(&e.to_string()))?
        .to_json())
}

/// Resolve a design and return the L2 Dry IR as a binary byte array.
#[wasm_bindgen]
pub fn resolve_binary(ops_json: &str, params_json: &str) -> Result<Vec<u8>, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    Ok(resolve_checked(&d, &p)
        .map_err(|e| JsError::new(&e.to_string()))?
        .to_bytes())
}

/// Resolve a design, run the standard L2 optimization pipeline, and return the resulting
/// L2 Dry IR as a JSON string.
#[wasm_bindgen]
pub fn resolve_optimized_ir(ops_json: &str, params_json: &str) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(optimize_pipeline(&tp).to_json())
}

/// Resolve a design and verify it against machine-safety contracts, returning the JSON
/// [`dry_core::Report`] (`{"findings":[{"rule","severity","segment","message"}]}`).
///
/// The contract is supplied as scalars/strings. `max_flow_opt` is the volumetric-flow ceiling (mm³/s),
/// `min_temp_opt` is the minimum nozzle temperature (°C), `bounds_str` is the comma-separated build bounds,
/// `monotonic_z` requires Z to be non-decreasing, and `speed_range_str` is the comma-separated speed range.
/// The convention is 0 (or any non-positive value) for flow/temp means unset. Empty strings for bounds
/// and speed range mean unset.
#[wasm_bindgen]
pub fn resolve_verify(
    ops_json: &str,
    params_json: &str,
    max_flow_opt: f64,
    min_temp_opt: f64,
    bounds_str: &str,
    monotonic_z: bool,
    speed_range_str: &str,
) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let bounds = if bounds_str.trim().is_empty() {
        None
    } else {
        Some(parse_bounds_csv(bounds_str).map_err(|e| JsError::new(&e.to_string()))?)
    };
    let speed_range = if speed_range_str.trim().is_empty() {
        None
    } else {
        Some(parse_speed_range_csv(speed_range_str).map_err(|e| JsError::new(&e.to_string()))?)
    };
    let contracts = Contracts {
        bounds,
        max_flow: if max_flow_opt > 0.0 {
            Some(max_flow_opt)
        } else {
            None
        },
        speed_range,
        monotonic_z,
        min_temp: if min_temp_opt > 0.0 {
            Some(min_temp_opt)
        } else {
            None
        },
    };
    let tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    let report = verify(&tp, &contracts);
    serde_json::to_string(&report).map_err(|e| JsError::new(&e.to_string()))
}

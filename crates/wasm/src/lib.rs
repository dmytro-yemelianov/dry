//! wasm-bindgen binding — the Dry engine (resolve + emit + simulate) in the browser and Node, the same
//! Rust core that powers the native CLI and the Python SDK. The L1 design crosses the boundary as JSON
//! (`[{"op":"move","x":..}, ...]`); the engine itself never depends on wasm-bindgen (this crate is
//! isolated from the core cargo workspace).

use dry_core::{
    emit, merge_collinear, resolve, simulate, verify, Contracts, Design, EmitParams, Kinematics,
    Op, ResolveParams,
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
    let tp = resolve(&d, &p);
    let kinematics = match kinematics_str {
        "ac" => Kinematics::Ac { pivot_offset: [0.0, 0.0, 0.0], rotary_offset: [0.0, 0.0] },
        "bc" => Kinematics::Bc { pivot_offset: [0.0, 0.0, 0.0], rotary_offset: [0.0, 0.0] },
        _ => Kinematics::Ab { pivot_offset: [0.0, 0.0, 0.0], rotary_offset: [0.0, 0.0] },
    };
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
    serde_json::to_string(&simulate(&resolve(&d, &p))).map_err(|e| JsError::new(&e.to_string()))
}

/// Resolve a design and return the L2 Dry IR as a JSON string.
#[wasm_bindgen]
pub fn resolve_ir(ops_json: &str, params_json: &str) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    Ok(resolve(&d, &p).to_json())
}

/// Resolve a design and return the L2 Dry IR as a binary byte array.
#[wasm_bindgen]
pub fn resolve_binary(ops_json: &str, params_json: &str) -> Result<Vec<u8>, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    Ok(resolve(&d, &p).to_bytes())
}

/// Resolve a design, optimize it (merge collinear extruding moves), and return the resulting
/// L2 Dry IR as a JSON string. Compare its `segments.len()` against [`resolve_ir`] to see how
/// many redundant moves the optimizer collapsed.
#[wasm_bindgen]
pub fn resolve_optimized_ir(ops_json: &str, params_json: &str) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    Ok(merge_collinear(&resolve(&d, &p)).to_json())
}

fn parse_bounds_wasm(s: &str) -> Result<[[f64; 2]; 3], JsError> {
    let v: Result<Vec<f64>, _> = s.split(',').map(|t| t.trim().parse::<f64>()).collect();
    let v = v.map_err(|e| JsError::new(&format!("bounds: {e}")))?;
    if v.len() != 6 {
        return Err(JsError::new(
            "bounds needs 6 comma-separated numbers: x0,x1,y0,y1,z0,z1",
        ));
    }
    Ok([[v[0], v[1]], [v[2], v[3]], [v[4], v[5]]])
}

fn parse_speed_range_wasm(s: &str) -> Result<[f64; 2], JsError> {
    let v: Result<Vec<f64>, _> = s.split(',').map(|t| t.trim().parse::<f64>()).collect();
    let v = v.map_err(|e| JsError::new(&format!("speed range: {e}")))?;
    if v.len() != 2 {
        return Err(JsError::new(
            "speed range needs 2 comma-separated numbers: min,max",
        ));
    }
    Ok([v[0], v[1]])
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
        Some(parse_bounds_wasm(bounds_str)?)
    };
    let speed_range = if speed_range_str.trim().is_empty() {
        None
    } else {
        Some(parse_speed_range_wasm(speed_range_str)?)
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
    let report = verify(&resolve(&d, &p), &contracts);
    serde_json::to_string(&report).map_err(|e| JsError::new(&e.to_string()))
}

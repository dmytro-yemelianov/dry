//! wasm-bindgen binding — the Dry engine (resolve + emit + simulate) in the browser and Node, the same
//! Rust core that powers the native CLI and the Python SDK. The L1 design crosses the boundary as JSON
//! (`[{"op":"move","x":..}, ...]`); the engine itself never depends on wasm-bindgen (this crate is
//! isolated from the core cargo workspace).

use dry_core::{
    emit, optimize_pipeline, resolve_checked, simulate, try_tpms_ops, verify, Contracts, Design,
    EmitParams, Kinematics, Op, ResolveParams, TpmsOptions,
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
    let kinematics = Kinematics::named(kinematics_str).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(emit(
        &tp,
        &EmitParams {
            relative_e,
            travel_g1_e0,
            five_axis,
            kinematics,
            ..EmitParams::default()
        },
    ))
}

/// Generate a gyroid TPMS infill design, resolve it, and emit motion g-code (a JS array of strings).
///
/// `tpms_options_json` is the camelCase [`TpmsOptions`] bundle (e.g. `{"cellSize":12,"cellsX":3}`);
/// `params_json` is the machine/material [`ResolveParams`]. The remaining flags mirror
/// [`resolve_gcode`]. The gyroid field uses `libm`, so the output differs sub-micron from the TS SDK's
/// `Math`-based generator — there is no byte-identity contract between them.
#[wasm_bindgen]
pub fn resolve_tpms_gcode(
    tpms_options_json: &str,
    params_json: &str,
    relative_e: bool,
    travel_g1_e0: bool,
    five_axis: bool,
    kinematics_str: &str,
) -> Result<Vec<String>, JsError> {
    let options: TpmsOptions = serde_json::from_str(tpms_options_json)
        .map_err(|e| JsError::new(&format!("tpms options: {e}")))?;
    let params: ResolveParams =
        serde_json::from_str(params_json).map_err(|e| JsError::new(&format!("params: {e}")))?;
    let ops = try_tpms_ops(&options).map_err(|e| JsError::new(&e.to_string()))?;
    let design = Design { ops };
    let tp = resolve_checked(&design, &params).map_err(|e| JsError::new(&e.to_string()))?;
    let kinematics = Kinematics::named(kinematics_str).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(emit(
        &tp,
        &EmitParams {
            relative_e,
            travel_g1_e0,
            five_axis,
            kinematics,
            ..EmitParams::default()
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

/// Build `[[x0, x1], [y0, y1], [z0, z1]]` build-volume bounds from the flat 6-value form the TS SDK
/// passes (`[x0, x1, y0, y1, z0, z1]`), validating the shape and returning a clear [`JsError`] (never a
/// panic) on a malformed input. `wasm-bindgen` cannot marshal `Vec<Vec<f64>>`, so the boundary stays
/// flat (`Float64Array`) and the structure is rebuilt here.
fn build_bounds(bounds: Option<Box<[f64]>>) -> Result<Option<[[f64; 2]; 3]>, JsError> {
    let Some(values) = bounds else {
        return Ok(None);
    };
    if values.len() != 6 {
        return Err(JsError::new(
            "bounds must be 6 values [x0, x1, y0, y1, z0, z1]",
        ));
    }
    if values.iter().any(|v| !v.is_finite()) {
        return Err(JsError::new("bounds values must all be finite"));
    }
    Ok(Some([
        [values[0], values[1]],
        [values[2], values[3]],
        [values[4], values[5]],
    ]))
}

/// Build a `[min, max]` range from the flat 2-value form the TS SDK passes, validating the shape and
/// returning a clear [`JsError`] (never a panic) on a malformed input.
fn build_range(name: &str, range: Option<Box<[f64]>>) -> Result<Option<[f64; 2]>, JsError> {
    let Some(values) = range else {
        return Ok(None);
    };
    if values.len() != 2 {
        return Err(JsError::new(&format!(
            "{name} must be [min, max] (2 values)"
        )));
    }
    if values.iter().any(|v| !v.is_finite()) {
        return Err(JsError::new(&format!("{name} values must all be finite")));
    }
    Ok(Some([values[0], values[1]]))
}

/// Resolve a design and verify it against machine-safety contracts, returning the JSON
/// [`dry_core::Report`] (`{"findings":[{"rule","severity","segment","message"}]}`).
///
/// Limits cross the boundary as native typed values (no CSV round-trip). `wasm-bindgen` cannot marshal
/// `Vec<Vec<f64>>`, so the structured contracts are passed flat: `bounds` is `[x0, x1, y0, y1, z0, z1]`
/// and each range (`speed_range`, `first_layer_height_range`, `first_layer_speed_range`) is
/// `[min, max]`; an absent (`None`/`undefined`) array disables that check. The scalar ceilings
/// (`max_flow_opt`, `min_temp_opt`, `max_retraction_distance_opt`, `max_retraction_speed_opt`,
/// `max_travel_without_retract_opt`) follow the convention that 0 (or any non-positive value) means
/// unset. `monotonic_z` requires Z to be non-decreasing.
#[wasm_bindgen]
pub fn resolve_verify(
    ops_json: &str,
    params_json: &str,
    max_flow_opt: f64,
    min_temp_opt: f64,
    bounds: Option<Box<[f64]>>,
    monotonic_z: bool,
    speed_range: Option<Box<[f64]>>,
    max_retraction_distance_opt: f64,
    max_retraction_speed_opt: f64,
    max_travel_without_retract_opt: f64,
    first_layer_height_range: Option<Box<[f64]>>,
    first_layer_speed_range: Option<Box<[f64]>>,
) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let positive = |v: f64| if v > 0.0 { Some(v) } else { None };
    let contracts = Contracts {
        bounds: build_bounds(bounds)?,
        max_flow: positive(max_flow_opt),
        speed_range: build_range("speed_range", speed_range)?,
        monotonic_z,
        min_temp: positive(min_temp_opt),
        max_retraction_distance: positive(max_retraction_distance_opt),
        max_retraction_speed: positive(max_retraction_speed_opt),
        max_travel_without_retract: positive(max_travel_without_retract_opt),
        first_layer_height_range: build_range(
            "first_layer_height_range",
            first_layer_height_range,
        )?,
        first_layer_speed_range: build_range("first_layer_speed_range", first_layer_speed_range)?,
    };
    let tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    let report = verify(&tp, &contracts);
    serde_json::to_string(&report).map_err(|e| JsError::new(&e.to_string()))
}

//! wasm-bindgen binding — the Dry engine (resolve + emit + simulate) in the browser and Node, the same
//! Rust core that powers the native CLI and the Python SDK. The L1 design crosses the boundary as JSON
//! (`[{"op":"move","x":..}, ...]`); the engine itself never depends on wasm-bindgen (this crate is
//! isolated from the core cargo workspace).

use dry_core::{
    balanced_pipeline, emit, optimize_pipeline, resolve_checked, safe_pipeline, simulate,
    try_tpms_ops, verify, Contracts, Design, EmitParams, KinematicContracts, Kinematics,
    MachineKinematics, Op, ResolveParams, TpmsOptions,
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

/// Generate a TPMS infill design and return its L1 `Op` list as a JSON string (`[{"op":"move",..},..]`).
///
/// `tpms_options_json` is the camelCase [`TpmsOptions`] bundle (e.g. `{"surface":"gyroid","cellSize":12}`).
/// Unlike [`resolve_tpms_gcode`], this stops at L1: it deserializes the options, builds the op list with
/// the engine's [`try_tpms_ops`] (the same `libm`-based generator the native CLI uses), and returns the
/// ops as JSON. The TS SDK delegates its TPMS Op generation here so the two SDKs are byte-identical
/// (the JS `Math`-based path drifts sub-micron from `libm`). An unknown surface (or any malformed field)
/// surfaces as a deserialize [`JsError`]; an invalid-options [`dry_core::TpmsError`] (e.g. a budget
/// overrun) surfaces as its own clear [`JsError`] — never a panic.
#[wasm_bindgen]
pub fn tpms_ops_json(tpms_options_json: &str) -> Result<String, JsError> {
    let options: TpmsOptions = serde_json::from_str(tpms_options_json)
        .map_err(|e| JsError::new(&format!("tpms options: {e}")))?;
    let ops = try_tpms_ops(&options).map_err(|e| JsError::new(&e.to_string()))?;
    // `dry_core::Op` now derives `Serialize` (symmetric with its `Deserialize`), so the wire form is
    // emitted by serde directly — no hand-maintained mirror to drift from the canonical contract.
    serde_json::to_string(&ops).map_err(|e| JsError::new(&e.to_string()))
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

/// Parse the optional `kinematics_json` boundary string into [`MachineKinematics`]. Empty → `None`.
/// A non-empty string that fails to parse is a clear [`JsError`] (never a panic).
///
/// # Name disambiguation
///
/// [`Kinematics`] (also exported in this crate) is the **5-axis rotary geometry** selector used by
/// `resolve_gcode` / `resolve_tpms_gcode` to select the correct arc strategy (Cartesian, CoreXY, …).
/// [`MachineKinematics`] is unrelated: it carries **profile motion limits** — specifically the
/// peak-acceleration and junction-velocity ceilings that feed the `balanced_pipeline` and the
/// `peak-acceleration` / `junction-velocity` verify rules.
fn parse_kinematics(kinematics_json: &str) -> Result<Option<MachineKinematics>, JsError> {
    let s = kinematics_json.trim();
    if s.is_empty() {
        return Ok(None);
    }
    serde_json::from_str::<MachineKinematics>(s)
        .map(Some)
        .map_err(|e| JsError::new(&format!("invalid kinematics_json: {e}")))
}

/// Resolve a design, run the balanced (kinematics-aware) L2 optimization pipeline, and return the
/// resulting L2 Dry IR as a JSON string.
///
/// When `kinematics_json` is a non-empty JSON object (`{"max_acceleration_mm_s2":3000,…}`), the
/// engine runs [`balanced_pipeline`] with those motion limits, which applies arc centripetal speed
/// clamping and junction-velocity capping in addition to all standard optimizations. An empty or
/// whitespace-only string falls back to [`safe_pipeline`] (the same pipeline used by
/// [`resolve_ir`] for parity).
///
/// A malformed non-empty `kinematics_json` surfaces as a clear [`JsError`] — never a panic.
#[wasm_bindgen]
pub fn resolve_balanced_ir(
    ops_json: &str,
    params_json: &str,
    kinematics_json: &str,
) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    let out = match parse_kinematics(kinematics_json)? {
        Some(k) => balanced_pipeline(&tp, Some(&k)),
        None => safe_pipeline(&tp),
    };
    Ok(out.to_json())
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
///
/// The optional `kinematics_json` trailing param accepts the same JSON object as
/// [`resolve_balanced_ir`]: when non-empty, it enables the `peak-acceleration` and
/// `junction-velocity` verify rules; an empty or whitespace-only string disables them (i.e.
/// `Contracts.kinematics = None`).
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
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
    kinematics_json: &str,
) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let positive = |v: f64| if v > 0.0 { Some(v) } else { None };
    let kinematics = parse_kinematics(kinematics_json)?.map(|k| KinematicContracts {
        max_acceleration_mm_s2: k.max_acceleration_mm_s2,
        max_junction_velocity_mm_s: k.max_junction_velocity_mm_s,
    });
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
        kinematics,
    };
    let tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    let report = verify(&tp, &contracts);
    serde_json::to_string(&report).map_err(|e| JsError::new(&e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kinematics_empty_returns_none() {
        assert!(parse_kinematics("").unwrap().is_none());
        assert!(parse_kinematics("   ").unwrap().is_none());
    }

    #[test]
    fn parse_kinematics_valid_json_returns_some() {
        let k = parse_kinematics(r#"{"max_acceleration_mm_s2":3000}"#)
            .unwrap()
            .unwrap();
        assert_eq!(k.max_acceleration_mm_s2, Some(3000.0));
        assert_eq!(k.max_junction_velocity_mm_s, None);
    }

    #[test]
    fn parse_kinematics_both_fields() {
        let k = parse_kinematics(
            r#"{"max_acceleration_mm_s2":5000,"max_junction_velocity_mm_s":10.0}"#,
        )
        .unwrap()
        .unwrap();
        assert_eq!(k.max_acceleration_mm_s2, Some(5000.0));
        assert_eq!(k.max_junction_velocity_mm_s, Some(10.0));
    }

    // JsError::new panics on non-wasm targets; this test only runs under wasm32.
    #[cfg(target_arch = "wasm32")]
    #[test]
    fn parse_kinematics_invalid_json_returns_error() {
        assert!(parse_kinematics("not-json").is_err());
    }
}

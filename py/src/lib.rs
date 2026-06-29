//! PyO3 binding — exposes the Dry engine (resolve + emit + simulate) to Python. A thin adapter over
//! `dry-core`: the L1 design crosses the boundary as JSON (`[{"op":"move","x":..}, ...]`), so the
//! Python SDK (`py/python/dry/`) stays logic-free and just builds the ops. Isolated from the core cargo
//! workspace (this crate links Python); the engine itself never depends on PyO3.

use dry_core::{
    emit, optimize_pipeline, resolve_checked, simulate, try_tpms_ops, verify, Contracts, Design,
    EmitParams, Kinematics, Op, ResolveParams, TpmsOptions,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

fn parse_design(ops_json: &str) -> PyResult<Design> {
    let ops: Vec<Op> = serde_json::from_str(ops_json)
        .map_err(|e| PyValueError::new_err(format!("invalid design: {e}")))?;
    Ok(Design { ops })
}

fn parse_params(params_json: &str) -> PyResult<ResolveParams> {
    serde_json::from_str(params_json)
        .map_err(|e| PyValueError::new_err(format!("invalid params: {e}")))
}

/// Resolve a design and emit motion g-code (one string per line).
#[pyfunction]
#[pyo3(signature = (ops_json, params_json, relative_e=true, travel_g1_e0=false, five_axis=false, kinematics="ab"))]
fn resolve_gcode(
    ops_json: &str,
    params_json: &str,
    relative_e: bool,
    travel_g1_e0: bool,
    five_axis: bool,
    kinematics: &str,
) -> PyResult<Vec<String>> {
    let tp = resolve_checked(&parse_design(ops_json)?, &parse_params(params_json)?)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    let kinematics =
        Kinematics::named(kinematics).map_err(|e| PyValueError::new_err(e.to_string()))?;
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

/// Generate a TPMS infill design and emit motion g-code (one string per line).
///
/// `tpms_options_json` is the TPMS option bundle (camelCase, matching the engine/TS wire form), e.g.
/// `{"surface":"gyroid","cellSize":12}`. An unknown surface name (or any malformed option) is a clean
/// `ValueError`, never a panic.
#[pyfunction]
#[pyo3(signature = (tpms_options_json, params_json, relative_e=true, travel_g1_e0=false, five_axis=false, kinematics="ab"))]
fn resolve_tpms_gcode(
    tpms_options_json: &str,
    params_json: &str,
    relative_e: bool,
    travel_g1_e0: bool,
    five_axis: bool,
    kinematics: &str,
) -> PyResult<Vec<String>> {
    let options: TpmsOptions = serde_json::from_str(tpms_options_json)
        .map_err(|e| PyValueError::new_err(format!("invalid tpms options: {e}")))?;
    let ops = try_tpms_ops(&options).map_err(|e| PyValueError::new_err(e.to_string()))?;
    let tp = resolve_checked(&Design { ops }, &parse_params(params_json)?)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    let kinematics =
        Kinematics::named(kinematics).map_err(|e| PyValueError::new_err(e.to_string()))?;
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
#[pyfunction]
fn resolve_metrics(ops_json: &str, params_json: &str) -> PyResult<String> {
    let tp = resolve_checked(&parse_design(ops_json)?, &parse_params(params_json)?)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    serde_json::to_string(&simulate(&tp)).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Resolve a design and return the L2 toolpath IR as a JSON string.
#[pyfunction]
fn resolve_ir(ops_json: &str, params_json: &str) -> PyResult<String> {
    let tp = resolve_checked(&parse_design(ops_json)?, &parse_params(params_json)?)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(tp.to_json())
}

/// Resolve a design and return the L2 toolpath IR as a binary byte array.
#[pyfunction]
fn resolve_binary(ops_json: &str, params_json: &str) -> PyResult<Vec<u8>> {
    let tp = resolve_checked(&parse_design(ops_json)?, &parse_params(params_json)?)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(tp.to_bytes())
}

/// Resolve a design, run the standard L2 optimization pipeline, and return the resulting
/// L2 toolpath IR as a JSON string.
#[pyfunction]
fn resolve_optimized_ir(ops_json: &str, params_json: &str) -> PyResult<String> {
    let tp = resolve_checked(&parse_design(ops_json)?, &parse_params(params_json)?)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(optimize_pipeline(&tp).to_json())
}

/// Build `[[x0,x1],[y0,y1],[z0,z1]]` build-volume bounds from the structured Python value, validating
/// the shape and returning a clear [`PyValueError`] (never a panic) on a malformed input.
fn build_bounds(bounds: Option<Vec<Vec<f64>>>) -> PyResult<Option<[[f64; 2]; 3]>> {
    let Some(rows) = bounds else {
        return Ok(None);
    };
    if rows.len() != 3 {
        return Err(PyValueError::new_err(
            "bounds must have shape [[x0,x1],[y0,y1],[z0,z1]] (3 axis pairs)",
        ));
    }
    let mut out = [[0.0_f64; 2]; 3];
    for (i, pair) in rows.iter().enumerate() {
        let [lo, hi] = pair.as_slice() else {
            return Err(PyValueError::new_err(format!(
                "bounds axis {i} must be a [min, max] pair"
            )));
        };
        if !lo.is_finite() || !hi.is_finite() {
            return Err(PyValueError::new_err(format!(
                "bounds axis {i} values must be finite"
            )));
        }
        out[i] = [*lo, *hi];
    }
    Ok(Some(out))
}

/// Build a `[min, max]` range from the structured Python value, validating the shape and returning a
/// clear [`PyValueError`] (never a panic) on a malformed input.
fn build_range(name: &str, range: Option<Vec<f64>>) -> PyResult<Option<[f64; 2]>> {
    let Some(values) = range else {
        return Ok(None);
    };
    let [lo, hi] = values.as_slice() else {
        return Err(PyValueError::new_err(format!(
            "{name} must be a [min, max] pair"
        )));
    };
    if !lo.is_finite() || !hi.is_finite() {
        return Err(PyValueError::new_err(format!(
            "{name} values must be finite"
        )));
    }
    Ok(Some([*lo, *hi]))
}

/// Resolve a design and verify it against machine-safety contracts, returning the JSON report.
#[pyfunction]
#[pyo3(signature = (
    ops_json,
    params_json,
    max_flow=None,
    min_temp=None,
    bounds=None,
    monotonic_z=false,
    speed_range=None,
    max_retraction_distance=None,
    max_retraction_speed=None,
    max_travel_without_retract=None,
    first_layer_height_range=None,
    first_layer_speed_range=None,
))]
fn resolve_verify(
    ops_json: &str,
    params_json: &str,
    max_flow: Option<f64>,
    min_temp: Option<f64>,
    bounds: Option<Vec<Vec<f64>>>,
    monotonic_z: bool,
    speed_range: Option<Vec<f64>>,
    max_retraction_distance: Option<f64>,
    max_retraction_speed: Option<f64>,
    max_travel_without_retract: Option<f64>,
    first_layer_height_range: Option<Vec<f64>>,
    first_layer_speed_range: Option<Vec<f64>>,
) -> PyResult<String> {
    let tp = resolve_checked(&parse_design(ops_json)?, &parse_params(params_json)?)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;

    let contracts = Contracts {
        bounds: build_bounds(bounds)?,
        max_flow,
        speed_range: build_range("speed_range", speed_range)?,
        monotonic_z,
        min_temp,
        max_retraction_distance,
        max_retraction_speed,
        max_travel_without_retract,
        first_layer_height_range: build_range(
            "first_layer_height_range",
            first_layer_height_range,
        )?,
        first_layer_speed_range: build_range("first_layer_speed_range", first_layer_speed_range)?,
    };

    let report = verify(&tp, &contracts);
    serde_json::to_string(&report).map_err(|e| PyValueError::new_err(e.to_string()))
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(resolve_gcode, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_tpms_gcode, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_metrics, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_ir, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_binary, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_optimized_ir, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_verify, m)?)?;
    Ok(())
}

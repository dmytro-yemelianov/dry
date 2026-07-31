//! PyO3 binding — exposes the Dry engine (resolve + emit + simulate) to Python. A thin adapter over
//! `dry-core`: the L1 design crosses the boundary as JSON (`[{"op":"move","x":..}, ...]`), so the
//! Python SDK (`py/python/dry/`) stays logic-free and just builds the ops. Isolated from the core cargo
//! workspace (this crate links Python); the engine itself never depends on PyO3.

use dry_core::{
    balanced_pipeline, emit_stream, expand_features as expand_feature_program, optimize_pipeline,
    resolve_checked, safe_pipeline, simulate, try_tpms_ops, verify, Contracts, Design, EmitParams,
    FeatureProgram, KinematicContracts, Kinematics, MachineKinematics, Op, ResolveParams,
    TpmsOptions,
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

/// Expand a bounded L0 feature graph into the canonical L1 op list.
#[pyfunction]
fn expand_features(program_json: &str) -> PyResult<String> {
    let program: FeatureProgram = serde_json::from_str(program_json)
        .map_err(|e| PyValueError::new_err(format!("invalid feature program: {e}")))?;
    let design = expand_feature_program(&program)
        .map_err(|e| PyValueError::new_err(format!("invalid feature program: {e}")))?;
    serde_json::to_string(&design.ops).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Parse the optional `kinematics_json` kwarg into [`MachineKinematics`]. `None` or empty string →
/// `None`. A non-empty string that fails to parse is a clear [`PyValueError`] (never a panic).
///
/// # Name disambiguation — two unrelated concepts
///
/// - **`rotary_axes`** (the `resolve_gcode` / `resolve_tpms_gcode` param): the rotary-axes selector,
///   a STRING `"ab"|"ac"|"bc"` choosing which two rotary axes carry the toolframe orientation in
///   5-axis emit. Parsed into the core [`Kinematics`] enum.
/// - **`kinematics_json`** (this function's input): the machine motion-limits OBJECT
///   `{max_acceleration_mm_s2, max_junction_velocity_mm_s}` ([`MachineKinematics`]) — the
///   peak-acceleration and junction-velocity ceilings that feed `balanced_pipeline` and the
///   `peak-acceleration` / `junction-velocity` verify rules. It has nothing to do with rotary axes.
fn parse_kinematics(kinematics_json: Option<&str>) -> PyResult<Option<MachineKinematics>> {
    let s = match kinematics_json {
        None => return Ok(None),
        Some(s) => s.trim(),
    };
    if s.is_empty() {
        return Ok(None);
    }
    serde_json::from_str::<MachineKinematics>(s)
        .map(Some)
        .map_err(|e| PyValueError::new_err(format!("invalid kinematics_json: {e}")))
}

/// Resolve a design and emit motion g-code (one string per line).
///
/// `rotary_axes` is the rotary-axes selector (the ab/ac/bc STRING) choosing which two rotary axes
/// carry the toolframe orientation in 5-axis emit — unrelated to the machine motion-limits
/// `kinematics_json` OBJECT consumed by `resolve_balanced_ir` / `resolve_verify`.
///
/// IR the emitter refuses — a non-finite word, an arc with no explicit endpoint (which
/// `validate_design` does not require) — raises `ValueError`. It previously came back as an empty
/// list, which callers read as a successfully emitted zero-line program.
#[pyfunction]
#[pyo3(signature = (ops_json, params_json, relative_e=true, travel_g1_e0=false, five_axis=false, rotary_axes="ab"))]
fn resolve_gcode(
    ops_json: &str,
    params_json: &str,
    relative_e: bool,
    travel_g1_e0: bool,
    five_axis: bool,
    rotary_axes: &str,
) -> PyResult<Vec<String>> {
    let tp = resolve_checked(&parse_design(ops_json)?, &parse_params(params_json)?)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    let kinematics =
        Kinematics::named(rotary_axes).map_err(|e| PyValueError::new_err(e.to_string()))?;
    emit_stream(
        tp.segments.iter().cloned().map(Ok),
        &EmitParams {
            relative_e,
            travel_g1_e0,
            five_axis,
            kinematics,
            ..EmitParams::default()
        },
    )
    .map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Generate a TPMS infill design and emit motion g-code (one string per line).
///
/// `tpms_options_json` is the TPMS option bundle (camelCase, matching the engine/TS wire form), e.g.
/// `{"surface":"gyroid","cellSize":12}`. An unknown surface name (or any malformed option) is a clean
/// `ValueError`, never a panic — as is IR the emitter refuses (see `resolve_gcode`).
#[pyfunction]
#[pyo3(signature = (tpms_options_json, params_json, relative_e=true, travel_g1_e0=false, five_axis=false, rotary_axes="ab"))]
fn resolve_tpms_gcode(
    tpms_options_json: &str,
    params_json: &str,
    relative_e: bool,
    travel_g1_e0: bool,
    five_axis: bool,
    rotary_axes: &str,
) -> PyResult<Vec<String>> {
    let options: TpmsOptions = serde_json::from_str(tpms_options_json)
        .map_err(|e| PyValueError::new_err(format!("invalid tpms options: {e}")))?;
    let ops = try_tpms_ops(&options).map_err(|e| PyValueError::new_err(e.to_string()))?;
    let tp = resolve_checked(&Design { ops }, &parse_params(params_json)?)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    let kinematics =
        Kinematics::named(rotary_axes).map_err(|e| PyValueError::new_err(e.to_string()))?;
    emit_stream(
        tp.segments.iter().cloned().map(Ok),
        &EmitParams {
            relative_e,
            travel_g1_e0,
            five_axis,
            kinematics,
            ..EmitParams::default()
        },
    )
    .map_err(|e| PyValueError::new_err(e.to_string()))
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

/// Resolve a design, run the balanced (kinematics-aware) L2 optimization pipeline, and return the
/// resulting L2 toolpath IR as a JSON string.
///
/// When `kinematics_json` is a non-empty JSON object (`{"max_acceleration_mm_s2":3000,…}`), the
/// engine runs [`balanced_pipeline`] with those motion limits, which applies arc centripetal speed
/// clamping and junction-velocity capping in addition to all standard optimizations. `None` or an
/// empty / whitespace-only string falls back to [`safe_pipeline`].
///
/// A malformed non-empty `kinematics_json` surfaces as a `ValueError` — never a panic.
#[pyfunction]
#[pyo3(signature = (ops_json, params_json, kinematics_json=None))]
fn resolve_balanced_ir(
    ops_json: &str,
    params_json: &str,
    kinematics_json: Option<&str>,
) -> PyResult<String> {
    let tp = resolve_checked(&parse_design(ops_json)?, &parse_params(params_json)?)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    let out = match parse_kinematics(kinematics_json)? {
        Some(k) => balanced_pipeline(&tp, Some(&k)),
        None => safe_pipeline(&tp),
    };
    Ok(out.to_json())
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
        if lo > hi {
            let axis = ["x", "y", "z"][i];
            return Err(PyValueError::new_err(format!(
                "bounds {axis} lower bound must be <= upper bound"
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
    if lo > hi {
        return Err(PyValueError::new_err(format!(
            "{name} lower bound must be <= upper bound"
        )));
    }
    Ok(Some([*lo, *hi]))
}

/// Resolve a design and verify it against machine-safety contracts, returning the JSON report.
///
/// The optional `kinematics_json` kwarg accepts the same JSON object as [`resolve_balanced_ir`]:
/// when non-empty, it enables the `peak-acceleration` and `junction-velocity` verify rules; `None`
/// or empty disables them (i.e. `Contracts.kinematics = None`). This kwarg is non-breaking for
/// existing Python callers — existing call sites that omit it keep the old behaviour.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
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
    kinematics_json=None,
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
    kinematics_json: Option<&str>,
) -> PyResult<String> {
    let design = parse_design(ops_json)?;
    let params = parse_params(params_json)?;

    let kinematics = parse_kinematics(kinematics_json)?.map(|k| KinematicContracts {
        max_acceleration_mm_s2: k.max_acceleration_mm_s2,
        max_junction_velocity_mm_s: k.max_junction_velocity_mm_s,
    });

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
        kinematics,
    };

    let tp = resolve_checked(&design, &params).map_err(|e| PyValueError::new_err(e.to_string()))?;
    let report = verify(&tp, &contracts);
    serde_json::to_string(&report).map_err(|e| PyValueError::new_err(e.to_string()))
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(expand_features, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_gcode, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_tpms_gcode, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_metrics, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_ir, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_binary, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_optimized_ir, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_balanced_ir, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_verify, m)?)?;
    Ok(())
}

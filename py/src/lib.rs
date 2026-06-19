//! PyO3 binding — exposes the Dry engine (resolve + emit + simulate) to Python. A thin adapter over
//! `dry-core`: the L1 design crosses the boundary as JSON (`[{"op":"move","x":..}, ...]`), so the
//! Python SDK (`py/python/dry/`) stays logic-free and just builds the ops. Isolated from the core cargo
//! workspace (this crate links Python); the engine itself never depends on PyO3.

use dry_core::{
    emit, parse_bounds_csv, parse_speed_range_csv, resolve_checked, simulate, verify, Contracts,
    Design, EmitParams, Kinematics, Op, ResolveParams,
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

/// Resolve a design and verify it against machine-safety contracts, returning the JSON report.
#[pyfunction]
#[pyo3(signature = (ops_json, params_json, max_flow=None, min_temp=None, bounds=None, monotonic_z=false, speed_range=None))]
fn resolve_verify(
    ops_json: &str,
    params_json: &str,
    max_flow: Option<f64>,
    min_temp: Option<f64>,
    bounds: Option<String>,
    monotonic_z: bool,
    speed_range: Option<String>,
) -> PyResult<String> {
    let tp = resolve_checked(&parse_design(ops_json)?, &parse_params(params_json)?)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;

    let parsed_bounds = match bounds {
        None => None,
        Some(s) => {
            if s.trim().is_empty() {
                None
            } else {
                Some(parse_bounds_csv(&s).map_err(|e| PyValueError::new_err(e.to_string()))?)
            }
        }
    };

    let parsed_speed = match speed_range {
        None => None,
        Some(s) => {
            if s.trim().is_empty() {
                None
            } else {
                Some(parse_speed_range_csv(&s).map_err(|e| PyValueError::new_err(e.to_string()))?)
            }
        }
    };

    let contracts = Contracts {
        bounds: parsed_bounds,
        max_flow,
        speed_range: parsed_speed,
        monotonic_z,
        min_temp,
    };

    let report = verify(&tp, &contracts);
    serde_json::to_string(&report).map_err(|e| PyValueError::new_err(e.to_string()))
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(resolve_gcode, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_metrics, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_ir, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_verify, m)?)?;
    Ok(())
}

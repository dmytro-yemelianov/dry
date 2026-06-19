//! PyO3 binding — exposes the Dry engine (resolve + emit + simulate) to Python. A thin adapter over
//! `dry-core`: the L1 design crosses the boundary as JSON (`[{"op":"move","x":..}, ...]`), so the
//! Python SDK (`py/python/dry/`) stays logic-free and just builds the ops. Isolated from the core cargo
//! workspace (this crate links Python); the engine itself never depends on PyO3.

use dry_core::{emit, resolve, simulate, Design, EmitParams, Kinematics, Op, ResolveParams};
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
#[pyo3(signature = (ops_json, params_json, relative_e=true))]
fn resolve_gcode(ops_json: &str, params_json: &str, relative_e: bool) -> PyResult<Vec<String>> {
    let tp = resolve(&parse_design(ops_json)?, &parse_params(params_json)?);
    Ok(emit(
        &tp,
        &EmitParams {
            relative_e,
            travel_g1_e0: false,
            five_axis: false,
            kinematics: Kinematics::Ab,
        },
    ))
}

/// Resolve a design and return its simulation metrics as a JSON string.
#[pyfunction]
fn resolve_metrics(ops_json: &str, params_json: &str) -> PyResult<String> {
    let tp = resolve(&parse_design(ops_json)?, &parse_params(params_json)?);
    serde_json::to_string(&simulate(&tp)).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Resolve a design and return the L2 toolpath IR as a JSON string.
#[pyfunction]
fn resolve_ir(ops_json: &str, params_json: &str) -> PyResult<String> {
    let tp = resolve(&parse_design(ops_json)?, &parse_params(params_json)?);
    Ok(tp.to_json())
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(resolve_gcode, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_metrics, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_ir, m)?)?;
    Ok(())
}

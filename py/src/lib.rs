//! PyO3 binding — exposes the Dry engine (resolve + emit + simulate) to Python. A thin adapter over
//! `dry-core`: the L1 design crosses the boundary as JSON (`[{"op":"move","x":..}, ...]`), so the
//! Python SDK (`py/python/dry/`) stays logic-free and just builds the ops. Isolated from the core cargo
//! workspace (this crate links Python); the engine itself never depends on PyO3.

use dry_core::generate::drape::{drape_ops, DrapeOptions, TriangleMesh};
use dry_core::{
    balanced_pipeline, emit_stream, expand_features as expand_feature_program, optimize_pipeline,
    resolve_checked, safe_pipeline, simulate, try_pocket_ops, try_tpms_ops, verify, Contracts,
    Design, EmitParams, FeatureProgram, KinematicContracts, Kinematics, MachineKinematics, Op,
    PocketOptions, ResolveParams, TpmsOptions,
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

/// Parse the optional `cnc_frame_json` kwarg into a [`dry_core::CncFrame`].
///
/// `None` or an empty string means "no frame", which is what an FFF program wants. A non-empty
/// string that fails to parse, or a frame the engine rejects (`wcs` outside `54..=59`, a
/// non-positive `spindle_rpm`), is a clean `ValueError` — the frame is validated here rather than
/// left to surface as a malformed program.
fn parse_cnc_frame(cnc_frame_json: Option<&str>) -> PyResult<Option<dry_core::CncFrame>> {
    let s = match cnc_frame_json {
        None => return Ok(None),
        Some(s) => s.trim(),
    };
    if s.is_empty() {
        return Ok(None);
    }
    let frame: dry_core::CncFrame = serde_json::from_str(s)
        .map_err(|e| PyValueError::new_err(format!("invalid cnc_frame_json: {e}")))?;
    frame.validate().map_err(PyValueError::new_err)?;
    Ok(Some(frame))
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
#[pyo3(signature = (ops_json, params_json, relative_e=true, travel_g1_e0=false, five_axis=false, rotary_axes="ab", flavor=None, cnc_frame_json=None))]
#[allow(clippy::too_many_arguments)]
fn resolve_gcode(
    ops_json: &str,
    params_json: &str,
    relative_e: bool,
    travel_g1_e0: bool,
    five_axis: bool,
    rotary_axes: &str,
    flavor: Option<&str>,
    cnc_frame_json: Option<&str>,
) -> PyResult<Vec<String>> {
    let tp = resolve_checked(&parse_design(ops_json)?, &parse_params(params_json)?)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    let kinematics =
        Kinematics::named(rotary_axes).map_err(|e| PyValueError::new_err(e.to_string()))?;
    // One parser in the engine, so this binding cannot silently lag the flavor catalog. It did:
    // the `match` here ended in `_ => Marlin`, so `flavor="siemens"` emitted FFF G-code for a
    // program that asked for a 5-axis mill. An unknown name is now an error.
    let firmware_flavor = match flavor {
        Some(name) => dry_core::FirmwareFlavor::named(name).map_err(PyValueError::new_err)?,
        None => dry_core::FirmwareFlavor::default(),
    };
    // Without a frame the CNC flavors emit motion lines and no machine preamble — no work offset,
    // no tool change, no spindle, and for Siemens no TRAORI. Exposing the flavor without this would
    // have been a hollow parity claim.
    let cnc_frame = parse_cnc_frame(cnc_frame_json)?;
    emit_stream(
        tp.segments.iter().cloned().map(Ok),
        &EmitParams {
            relative_e,
            travel_g1_e0,
            five_axis,
            kinematics,
            flavor: firmware_flavor,
            cnc_frame,
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

/// Generate a TPMS infill design and return its L1 `Op` list as a JSON string.
#[pyfunction]
fn tpms_ops_json(tpms_options_json: &str) -> PyResult<String> {
    let options: TpmsOptions = serde_json::from_str(tpms_options_json)
        .map_err(|e| PyValueError::new_err(format!("invalid tpms options: {e}")))?;
    let ops = try_tpms_ops(&options).map_err(|e| PyValueError::new_err(e.to_string()))?;
    serde_json::to_string(&ops).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Generate a CNC pocket/profile milling design and return its L1 `Op` list as a JSON string.
#[pyfunction]
fn pocket_ops_json(pocket_options_json: &str) -> PyResult<String> {
    let options: PocketOptions = serde_json::from_str(pocket_options_json)
        .map_err(|e| PyValueError::new_err(format!("invalid pocket options: {e}")))?;
    let ops = try_pocket_ops(&options).map_err(|e| PyValueError::new_err(e.to_string()))?;
    serde_json::to_string(&ops).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Generate a CNC pocket/profile design, resolve it, and emit motion g-code (one string per line).
#[pyfunction]
#[pyo3(signature = (pocket_options_json, params_json, relative_e=true, travel_g1_e0=false, five_axis=false, rotary_axes="ab"))]
fn resolve_pocket_gcode(
    pocket_options_json: &str,
    params_json: &str,
    relative_e: bool,
    travel_g1_e0: bool,
    five_axis: bool,
    rotary_axes: &str,
) -> PyResult<Vec<String>> {
    let options: PocketOptions = serde_json::from_str(pocket_options_json)
        .map_err(|e| PyValueError::new_err(format!("invalid pocket options: {e}")))?;
    let ops = try_pocket_ops(&options).map_err(|e| PyValueError::new_err(e.to_string()))?;
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
        // `bead-volume` is opt-in and has no kwarg on this entry point yet; the always-on structural
        // rules apply here regardless.
        bead_volume_tolerance: None,
        kinematics,
        // Rotary limits are machine facts that arrive with a profile; this entry point takes loose
        // kwargs and has no profile, so the three rotary rules stay unevaluated here.
        rotary: None,
        // Whether travels must be dark is a fact about the *process*, and arrives with a profile for
        // the same reason the rotary limits do. Left unset here, so `laser-power-during-travel` stays
        // unevaluated rather than firing on a spindle program whose rapids are correct.
        travel_must_be_dark: None,
    };

    let tp = resolve_checked(&design, &params).map_err(|e| PyValueError::new_err(e.to_string()))?;
    let report = verify(&tp, &contracts);
    serde_json::to_string(&report).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Generate L1 draping ops over a 3D mesh.
#[pyfunction]
fn drape_ops_json(options_json: &str) -> PyResult<String> {
    let options: DrapeOptions = serde_json::from_str(options_json)
        .map_err(|e| PyValueError::new_err(format!("invalid drape options: {e}")))?;
    let ops = drape_ops(&options).map_err(|e| PyValueError::new_err(e.to_string()))?;
    serde_json::to_string(&ops).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Parse OBJ text into serialized TriangleMesh JSON.
#[pyfunction]
fn parse_obj_mesh_json(obj_text: &str) -> PyResult<String> {
    let mesh =
        TriangleMesh::from_obj(obj_text).map_err(|e| PyValueError::new_err(e.to_string()))?;
    serde_json::to_string(&mesh).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Slice a STEP ISO 10303-21 CAD file directly into L1 ops.
#[pyfunction]
fn slice_step_solid_json(
    step_content: &str,
    z_start: f64,
    z_end: f64,
    layer_height: f64,
    samples_per_slice: usize,
    feedrate: f64,
) -> PyResult<String> {
    let solid = dry_core::BrepSolid::parse_step_iso10303(step_content)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    let ops = solid
        .slice_to_l1_ops(z_start, z_end, layer_height, samples_per_slice, feedrate)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    serde_json::to_string(&ops).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Generate CNC Lathe Facing operations from parameters JSON.
#[pyfunction]
fn lathe_facing_ops_json(params_json: &str) -> PyResult<String> {
    let params: dry_core::LatheFacingParams = serde_json::from_str(params_json)
        .map_err(|e| PyValueError::new_err(format!("invalid lathe facing params: {e}")))?;
    let ops = dry_core::generate_lathe_facing_ops(&params).map_err(PyValueError::new_err)?;
    serde_json::to_string(&ops).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Generate CNC Lathe OD Roughing & Finishing operations from parameters JSON.
#[pyfunction]
fn lathe_od_turning_ops_json(params_json: &str) -> PyResult<String> {
    let params: dry_core::LatheTurningParams = serde_json::from_str(params_json)
        .map_err(|e| PyValueError::new_err(format!("invalid lathe turning params: {e}")))?;
    let ops = dry_core::generate_lathe_od_turning_ops(&params).map_err(PyValueError::new_err)?;
    serde_json::to_string(&ops).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Check toolpath for tool holder collision against stock volume bounds.
#[pyfunction]
fn check_tool_holder_collision_json(
    toolpath_json: &str,
    holder_json: &str,
    stock_bounds_json: &str,
) -> PyResult<String> {
    let toolpath: dry_core::Toolpath = serde_json::from_str(toolpath_json)
        .map_err(|e| PyValueError::new_err(format!("invalid toolpath: {e}")))?;
    let holder: dry_core::ToolHolder = serde_json::from_str(holder_json)
        .map_err(|e| PyValueError::new_err(format!("invalid tool holder: {e}")))?;
    let stock_bounds: [f64; 6] = serde_json::from_str(stock_bounds_json)
        .map_err(|e| PyValueError::new_err(format!("invalid stock bounds: {e}")))?;
    let findings = dry_core::check_tool_holder_collision(&toolpath, &holder, stock_bounds);
    serde_json::to_string(&findings).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Reverse-engineer an L1 Design JSON from a resolved L2 Toolpath JSON.
#[pyfunction]
fn reverse_toolpath_json(toolpath_json: &str) -> PyResult<String> {
    let toolpath: dry_core::Toolpath = serde_json::from_str(toolpath_json)
        .map_err(|e| PyValueError::new_err(format!("invalid toolpath: {e}")))?;
    let design =
        dry_core::reverse::reverse(&toolpath).map_err(|e| PyValueError::new_err(e.to_string()))?;
    serde_json::to_string(&design.ops).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Slice an analytical B-Rep multi-solid assembly into continuous L1 operations JSON.
#[pyfunction]
fn slice_brep_assembly_json(
    assembly_json: &str,
    z_start: f64,
    z_end: f64,
    layer_height: f64,
    samples_per_slice: usize,
    feedrate: f64,
) -> PyResult<String> {
    let step_solids: Vec<String> = serde_json::from_str(assembly_json)
        .map_err(|e| PyValueError::new_err(format!("invalid assembly json: {e}")))?;
    let mut asm = dry_core::generate::BrepAssembly::new("python_brep_assembly");
    for step in step_solids {
        let solid = dry_core::BrepSolid::parse_step_iso10303(&step)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        asm.add_solid(solid, dry_core::generate::BrepBodyRole::AdditiveBody);
    }
    let ops = asm
        .slice_to_l1_ops(z_start, z_end, layer_height, samples_per_slice, feedrate)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    serde_json::to_string(&ops).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Slice a multi-solid B-Rep assembly with CSG boolean void subtraction in Python.
#[pyfunction]
fn slice_brep_assembly_csg_json(
    additives_json: &str,
    voids_json: &str,
    z_start: f64,
    z_end: f64,
    layer_height: f64,
    samples_per_slice: usize,
    feedrate: f64,
) -> PyResult<String> {
    let step_additives: Vec<String> = serde_json::from_str(additives_json)
        .map_err(|e| PyValueError::new_err(format!("invalid additives json: {e}")))?;
    let step_voids: Vec<String> = serde_json::from_str(voids_json)
        .map_err(|e| PyValueError::new_err(format!("invalid voids json: {e}")))?;

    let mut asm = dry_core::generate::BrepAssembly::new("python_csg_assembly");
    for step in step_additives {
        let solid = dry_core::BrepSolid::parse_step_iso10303(&step)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        asm.add_solid(solid, dry_core::generate::BrepBodyRole::AdditiveBody);
    }
    for step in step_voids {
        let solid = dry_core::BrepSolid::parse_step_iso10303(&step)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        asm.add_solid(solid, dry_core::generate::BrepBodyRole::SubtractiveVoid);
    }
    let ops = asm
        .slice_with_csg(z_start, z_end, layer_height, samples_per_slice, feedrate)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    serde_json::to_string(&ops).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Optimize toolpath for Constant Material Removal Rate (MRR) in Python.
#[pyfunction]
fn optimize_constant_mrr_json(
    toolpath_json: &str,
    depth_of_cut: f64,
    target_mrr_mm3_min: f64,
    min_feedrate: f64,
    max_feedrate: f64,
) -> PyResult<String> {
    let mut tp: dry_core::Toolpath = serde_json::from_str(toolpath_json)
        .map_err(|e| PyValueError::new_err(format!("invalid toolpath: {e}")))?;
    dry_core::optimize::optimize_constant_mrr(
        &mut tp,
        depth_of_cut,
        target_mrr_mm3_min,
        min_feedrate,
        max_feedrate,
    );
    serde_json::to_string(&tp).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Run the digital-twin machining physics analysis and return the report as JSON.
///
/// `material` is one of `Aluminum6061`, `Steel4140`, `TitaniumTi6Al4V`, `Inconel718`,
/// `ThermoplasticPLA`, `ThermoplasticPEEK`. The estimates are analytic and unvalidated against
/// instrumented cuts — see `docs/14-known-limitations.md`.
#[pyfunction]
fn analyze_machining_physics_json(
    tool_json: &str,
    material: &str,
    params_json: &str,
) -> PyResult<String> {
    let tool: dry_core::CuttingToolGeometry = serde_json::from_str(tool_json)
        .map_err(|e| PyValueError::new_err(format!("invalid tool geometry: {e}")))?;
    let params: dry_core::MachiningOperationParams = serde_json::from_str(params_json)
        .map_err(|e| PyValueError::new_err(format!("invalid operation params: {e}")))?;
    // Route the material through serde so the accepted names are exactly the wire form, and an
    // unknown one is refused rather than silently defaulted.
    let material: dry_core::WorkpieceMaterial = serde_json::from_str(&format!("\"{material}\""))
        .map_err(|e| PyValueError::new_err(format!("unknown workpiece material: {e}")))?;
    let report = dry_core::analyze_machining_physics(&tool, material, &params);
    serde_json::to_string(&report).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Apply the synchronised 5-axis jerk-limited lookahead optimiser to a toolpath.
#[pyfunction]
fn optimize_five_axis_lookahead_json(toolpath_json: &str, params_json: &str) -> PyResult<String> {
    let tp: dry_core::Toolpath = serde_json::from_str(toolpath_json)
        .map_err(|e| PyValueError::new_err(format!("invalid toolpath: {e}")))?;
    let params: dry_core::optimize::FiveAxisLookaheadParams = serde_json::from_str(params_json)
        .map_err(|e| PyValueError::new_err(format!("invalid lookahead params: {e}")))?;
    let out = dry_core::optimize_five_axis_lookahead(&tp, &params);
    serde_json::to_string(&out).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Simulate 3D Dexel grid stock subtraction in Python and return volumetric report.
#[pyfunction]
fn simulate_dexel_stock_json(
    toolpath_json: &str,
    min_x: f64,
    min_y: f64,
    min_z: f64,
    max_x: f64,
    max_y: f64,
    max_z: f64,
    resolution_mm: f64,
    tool_radius: f64,
    is_ballnose: bool,
) -> PyResult<String> {
    let tp: dry_core::Toolpath = serde_json::from_str(toolpath_json)
        .map_err(|e| PyValueError::new_err(format!("invalid toolpath: {e}")))?;
    let mut stock =
        dry_core::DexelGrid::new_stock(min_x, min_y, min_z, max_x, max_y, max_z, resolution_mm)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
    stock.simulate_toolpath(&tp, tool_radius, is_ballnose);
    let report = stock.generate_report();
    serde_json::to_string(&report).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Compute Euclidean distance between two 3D line segments in Python.
#[pyfunction]
fn segment_to_segment_distance_3d_py(
    p1: [f64; 3],
    p2: [f64; 3],
    q1: [f64; 3],
    q2: [f64; 3],
) -> f64 {
    dry_core::segment_to_segment_distance_3d(p1, p2, q1, q2)
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(expand_features, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_gcode, m)?)?;
    m.add_function(wrap_pyfunction!(tpms_ops_json, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_tpms_gcode, m)?)?;
    m.add_function(wrap_pyfunction!(pocket_ops_json, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_pocket_gcode, m)?)?;
    m.add_function(wrap_pyfunction!(drape_ops_json, m)?)?;
    m.add_function(wrap_pyfunction!(parse_obj_mesh_json, m)?)?;
    m.add_function(wrap_pyfunction!(slice_step_solid_json, m)?)?;
    m.add_function(wrap_pyfunction!(slice_brep_assembly_json, m)?)?;
    m.add_function(wrap_pyfunction!(slice_brep_assembly_csg_json, m)?)?;
    m.add_function(wrap_pyfunction!(optimize_constant_mrr_json, m)?)?;
    m.add_function(wrap_pyfunction!(simulate_dexel_stock_json, m)?)?;
    m.add_function(wrap_pyfunction!(analyze_machining_physics_json, m)?)?;
    m.add_function(wrap_pyfunction!(optimize_five_axis_lookahead_json, m)?)?;
    m.add_function(wrap_pyfunction!(segment_to_segment_distance_3d_py, m)?)?;
    m.add_function(wrap_pyfunction!(lathe_facing_ops_json, m)?)?;
    m.add_function(wrap_pyfunction!(lathe_od_turning_ops_json, m)?)?;
    m.add_function(wrap_pyfunction!(check_tool_holder_collision_json, m)?)?;
    m.add_function(wrap_pyfunction!(reverse_toolpath_json, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_metrics, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_ir, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_binary, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_optimized_ir, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_balanced_ir, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_verify, m)?)?;
    Ok(())
}

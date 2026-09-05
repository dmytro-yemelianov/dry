//! wasm-bindgen binding — the Dry engine (resolve + emit + simulate) in the browser and Node, the same
//! Rust core that powers the native CLI and the Python SDK. The L1 design crosses the boundary as JSON
//! (`[{"op":"move","x":..}, ...]`); the engine itself never depends on wasm-bindgen (this crate is
//! isolated from the core cargo workspace).

use dry_core::{
    balanced_pipeline, emit_stream, expand_features as expand_feature_program, optimize_pipeline,
    resolve_checked, safe_pipeline, simulate, try_pocket_ops, try_tpms_ops, verify, Contracts,
    Design, EmitParams, FeatureProgram, KinematicContracts, Kinematics, MachineKinematics, Op,
    PocketOptions, ResolveParams, Toolpath, TpmsOptions,
};
use wasm_bindgen::prelude::*;

fn parse(ops_json: &str, params_json: &str) -> Result<(Design, ResolveParams), JsError> {
    let ops: Vec<Op> =
        serde_json::from_str(ops_json).map_err(|e| JsError::new(&format!("design: {e}")))?;
    let params: ResolveParams =
        serde_json::from_str(params_json).map_err(|e| JsError::new(&format!("params: {e}")))?;
    Ok((Design { ops }, params))
}

/// Expand a bounded L0 feature graph into the canonical L1 op list.
#[wasm_bindgen]
pub fn expand_features(program_json: &str) -> Result<String, JsError> {
    let program: FeatureProgram = serde_json::from_str(program_json)
        .map_err(|e| JsError::new(&format!("feature program: {e}")))?;
    let design = expand_feature_program(&program).map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&design.ops).map_err(|e| JsError::new(&e.to_string()))
}

/// Resolve a design and emit motion g-code (returned as a JS array of strings).
///
/// `rotary_axes` is the **rotary-axes selector** (the ab/ac/bc STRING) choosing which two rotary axes
/// carry the toolframe orientation in 5-axis emit. It is unrelated to the machine motion-limits
/// `kinematics_json` OBJECT (`{max_acceleration_mm_s2,…}`) consumed by [`resolve_balanced_ir`] /
/// [`resolve_verify`].
///
/// IR the emitter refuses — a non-finite word, an arc with no explicit endpoint (which
/// `validate_design` does not require) — surfaces as a [`JsError`]. It previously came back as an
/// empty array, which callers read as a successfully emitted zero-line program.
/// Parse the optional `cnc_frame_json` argument into a [`dry_core::CncFrame`].
///
/// `undefined` or an empty string means "no frame", which is what an FFF program wants. A frame the
/// engine rejects (`wcs` outside `54..=59`, a non-positive `spindle_rpm`) is refused here rather
/// than left to surface as a malformed program.
fn parse_cnc_frame(cnc_frame_json: Option<String>) -> Result<Option<dry_core::CncFrame>, JsError> {
    let raw = match cnc_frame_json {
        None => return Ok(None),
        Some(s) => s,
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let frame: dry_core::CncFrame = serde_json::from_str(trimmed)
        .map_err(|e| JsError::new(&format!("invalid cnc_frame_json: {e}")))?;
    frame.validate().map_err(|e| JsError::new(&e))?;
    Ok(Some(frame))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn resolve_gcode(
    ops_json: &str,
    params_json: &str,
    relative_e: bool,
    travel_g1_e0: bool,
    five_axis: bool,
    rotary_axes: &str,
    flavor: Option<String>,
    cnc_frame_json: Option<String>,
) -> Result<Vec<String>, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    let kinematics = Kinematics::named(rotary_axes).map_err(|e| JsError::new(&e.to_string()))?;
    // Until now this surface could only emit Marlin: it took no flavor at all, so every non-FFF
    // target was unreachable from the browser and the TS SDK. An unknown name is an error, never a
    // silent fall back to Marlin.
    let flavor = match flavor {
        Some(name) => dry_core::FirmwareFlavor::named(&name).map_err(|e| JsError::new(&e))?,
        None => dry_core::FirmwareFlavor::default(),
    };
    let cnc_frame = parse_cnc_frame(cnc_frame_json)?;
    emit_stream(
        tp.segments.iter().cloned().map(Ok),
        &EmitParams {
            relative_e,
            travel_g1_e0,
            five_axis,
            kinematics,
            flavor,
            cnc_frame,
            ..EmitParams::default()
        },
    )
    .map_err(|e| JsError::new(&e.to_string()))
}

/// Generate a gyroid TPMS infill design, resolve it, and emit motion g-code (a JS array of strings).
///
/// `tpms_options_json` is the camelCase [`TpmsOptions`] bundle (e.g. `{"cellSize":12,"cellsX":3}`);
/// `params_json` is the machine/material [`ResolveParams`]. `rotary_axes` is the rotary-axes selector
/// (the ab/ac/bc STRING) — see [`resolve_gcode`]; it is unrelated to the motion-limits `kinematics_json`
/// OBJECT. The gyroid field uses `libm`, so the output differs sub-micron from the TS SDK's
/// `Math`-based generator — there is no byte-identity contract between them.
///
/// IR the emitter refuses surfaces as a [`JsError`], not an empty array — see [`resolve_gcode`].
#[wasm_bindgen]
pub fn resolve_tpms_gcode(
    tpms_options_json: &str,
    params_json: &str,
    relative_e: bool,
    travel_g1_e0: bool,
    five_axis: bool,
    rotary_axes: &str,
) -> Result<Vec<String>, JsError> {
    let options: TpmsOptions = serde_json::from_str(tpms_options_json)
        .map_err(|e| JsError::new(&format!("tpms options: {e}")))?;
    let params: ResolveParams =
        serde_json::from_str(params_json).map_err(|e| JsError::new(&format!("params: {e}")))?;
    let ops = try_tpms_ops(&options).map_err(|e| JsError::new(&e.to_string()))?;
    let design = Design { ops };
    let tp = resolve_checked(&design, &params).map_err(|e| JsError::new(&e.to_string()))?;
    let kinematics = Kinematics::named(rotary_axes).map_err(|e| JsError::new(&e.to_string()))?;
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
    .map_err(|e| JsError::new(&e.to_string()))
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

/// Generate a CNC pocket/profile milling design and return its L1 `Op` list as a JSON string.
#[wasm_bindgen]
pub fn pocket_ops_json(pocket_options_json: &str) -> Result<String, JsError> {
    let options: PocketOptions = serde_json::from_str(pocket_options_json)
        .map_err(|e| JsError::new(&format!("pocket options: {e}")))?;
    let ops = try_pocket_ops(&options).map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&ops).map_err(|e| JsError::new(&e.to_string()))
}

/// Resolve a design and return its simulation metrics as a JSON string.
#[wasm_bindgen]
pub fn resolve_metrics(ops_json: &str, params_json: &str) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&simulate(&tp)).map_err(|e| JsError::new(&e.to_string()))
}

/// Simulate an already-resolved Dry IR (`{"version":..,"segments":[..]}`) and return its metrics as a
/// JSON string. Unlike [`resolve_metrics`], which simulates an L1 design, this takes a toolpath IR
/// directly — so a caller can report the before/after time and peak flow of an optimized or balanced
/// IR, which has no originating op-list. A malformed IR surfaces as a clear [`JsError`] — never a panic.
#[wasm_bindgen]
pub fn metrics_ir(ir_json: &str) -> Result<String, JsError> {
    let tp = Toolpath::from_json(ir_json).map_err(|e| JsError::new(&format!("ir: {e}")))?;
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
    let bounds = [
        [values[0], values[1]],
        [values[2], values[3]],
        [values[4], values[5]],
    ];
    for (axis, [lo, hi]) in ["x", "y", "z"].into_iter().zip(bounds) {
        if lo > hi {
            return Err(JsError::new(&format!(
                "bounds {axis} lower bound must be <= upper bound"
            )));
        }
    }
    Ok(Some(bounds))
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
    if values[0] > values[1] {
        return Err(JsError::new(&format!(
            "{name} lower bound must be <= upper bound"
        )));
    }
    Ok(Some([values[0], values[1]]))
}

/// Parse the optional `kinematics_json` boundary string into [`MachineKinematics`]. Empty → `None`.
/// A non-empty string that fails to parse is a clear [`JsError`] (never a panic).
///
/// # Name disambiguation — two unrelated concepts
///
/// - **`rotary_axes`** (the `resolve_gcode` / `resolve_tpms_gcode` param): the rotary-axes selector,
///   a STRING `"ab"|"ac"|"bc"` choosing which two rotary axes carry the toolframe orientation in
///   5-axis emit. Parsed into the core [`Kinematics`] enum.
/// - **`kinematics_json`** (this function's input): the machine motion-limits OBJECT
///   `{max_acceleration_mm_s2, max_junction_velocity_mm_s}` ([`MachineKinematics`]). It carries
///   peak-acceleration and junction-velocity ceilings that feed [`balanced_pipeline`] and the
///   `peak-acceleration` / `junction-velocity` verify rules. It has nothing to do with rotary axes.
fn parse_kinematics(kinematics_json: &str) -> Result<Option<MachineKinematics>, JsError> {
    parse_kinematics_inner(kinematics_json).map_err(|e| JsError::new(&e))
}

/// The parse itself, with no `JsError` in the signature.
///
/// `JsError::new` calls a wasm-bindgen import, which panics with "cannot call wasm-bindgen imported
/// functions on non-wasm targets" when a host-target test evaluates it. Keeping the boundary
/// conversion in the caller is what lets the rejection path be tested by `cargo test` on the host
/// rather than only under a wasm runner — and this crate has no wasm test runner, so a test gated on
/// `target_arch = "wasm32"` runs nowhere at all.
fn parse_kinematics_inner(kinematics_json: &str) -> Result<Option<MachineKinematics>, String> {
    let s = kinematics_json.trim();
    if s.is_empty() {
        return Ok(None);
    }
    serde_json::from_str::<MachineKinematics>(s)
        .map(Some)
        .map_err(|e| format!("invalid kinematics_json: {e}"))
}

/// Resolve a design, run the balanced (kinematics-aware) L2 optimization pipeline, and return the
/// resulting L2 Dry IR as a JSON string.
///
/// When `kinematics_json` is a non-empty JSON object (`{"max_acceleration_mm_s2":3000,…}`), the
/// engine runs [`balanced_pipeline`] with those motion limits, which applies arc centripetal speed
/// clamping and junction-velocity capping in addition to all standard optimizations. An empty or
/// whitespace-only string falls back to [`safe_pipeline`] (the same pipeline used by
/// [`resolve_optimized_ir`] for parity).
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
        // `bead-volume` is opt-in and has no argument on this entry point yet; the always-on
        // structural rules (continuity, segment-length, arc-length, negative-quantity,
        // filament-consistency) apply here regardless of what is passed.
        bead_volume_tolerance: None,
        kinematics,
        // Rotary limits are machine facts that arrive with a profile; this entry point takes loose
        // arguments and has no profile, so the three rotary rules stay unevaluated here.
        rotary: None,
        // Whether travels must be dark is a fact about the *process*, and arrives with a profile for
        // the same reason the rotary limits do. Left unset here, so `laser-power-during-travel` stays
        // unevaluated rather than firing on a spindle program whose rapids are correct.
        travel_must_be_dark: None,
    };
    let tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    let report = verify(&tp, &contracts);
    serde_json::to_string(&report).map_err(|e| JsError::new(&e.to_string()))
}

/// Compute theoretical surface quality metrics (cusp height and arithmetic roughness Ra).
#[wasm_bindgen]
pub fn compute_surface_quality(tool_radius_mm: f64, stepover_mm: f64) -> Result<String, JsError> {
    let report =
        dry_core::evaluate_surface_quality(tool_radius_mm, stepover_mm).map_err(JsError::new)?;
    serde_json::to_string(&report).map_err(|e| JsError::new(&e.to_string()))
}

/// Encode a design into the compact DRY2 delta binary format.
#[wasm_bindgen]
pub fn encode_dry2_binary(ops_json: &str, params_json: &str) -> Result<Vec<u8>, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(dry_core::encode_dry2(&tp))
}

/// Decode a DRY2 binary payload into a JSON toolpath string.
#[wasm_bindgen]
pub fn decode_dry2_binary(bytes: &[u8]) -> Result<String, JsError> {
    let tp = dry_core::decode_dry2(bytes).map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&tp).map_err(|e| JsError::new(&e.to_string()))
}

/// Emit plasma or abrasive waterjet cutting motion G-code.
#[wasm_bindgen]
pub fn emit_plasma(
    ops_json: &str,
    params_json: &str,
    cutting_params_json: &str,
) -> Result<Vec<String>, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    let cutting_params: dry_core::CuttingParams = if cutting_params_json.trim().is_empty() {
        dry_core::CuttingParams::default()
    } else {
        serde_json::from_str(cutting_params_json)
            .map_err(|e| JsError::new(&format!("cutting params: {e}")))?
    };
    dry_core::emit_plasma_waterjet(&tp, &cutting_params).map_err(|e| JsError::new(&e.to_string()))
}

/// Run corner engagement feedrate optimization on a design.
#[wasm_bindgen]
pub fn optimize_engagement(
    ops_json: &str,
    params_json: &str,
    min_feed_ratio: f64,
) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let mut tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    dry_core::optimize_corner_feedrate(&mut tp, min_feed_ratio);
    serde_json::to_string(&tp).map_err(|e| JsError::new(&e.to_string()))
}

/// Check compatibility of a design against machine capabilities.
#[wasm_bindgen]
pub fn check_machine_compatibility(
    ops_json: &str,
    params_json: &str,
    capabilities_json: &str,
) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    let caps: dry_core::MachineCapabilities = serde_json::from_str(capabilities_json)
        .map_err(|e| JsError::new(&format!("capabilities: {e}")))?;
    let report = dry_core::check_compatibility(&tp, &caps);
    serde_json::to_string(&report).map_err(|e| JsError::new(&e.to_string()))
}

/// Reverse-parse raw G-code text into a structured Toolpath IR JSON for in-browser inspection.
#[wasm_bindgen]
pub fn import_gcode_to_ir(gcode_text: &str) -> Result<String, JsError> {
    let imported = dry_core::import_gcode(gcode_text, &dry_core::GcodeImportParams::default())
        .map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&imported).map_err(|e| JsError::new(&e.to_string()))
}

/// Directly verify raw G-code text against safety contracts without container or server infrastructure.
#[wasm_bindgen]
pub fn verify_gcode_to_report_wasm(
    gcode_text: &str,
    contracts_json: &str,
) -> Result<String, JsError> {
    let import_params = dry_core::GcodeImportParams {
        line_width: Some(0.45),
        layer_height: Some(0.2),
        ..dry_core::GcodeImportParams::default()
    };
    let imported = dry_core::import_gcode(gcode_text, &import_params)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let contracts: Contracts = if contracts_json.trim().is_empty() {
        Contracts::default()
    } else {
        serde_json::from_str(contracts_json)
            .map_err(|e| JsError::new(&format!("contracts: {e}")))?
    };
    let report = verify(&imported, &contracts);
    serde_json::to_string(&report).map_err(|e| JsError::new(&e.to_string()))
}

/// Compute a 7-phase jerk-bounded S-curve trajectory profile.
#[wasm_bindgen]
pub fn compute_scurve_profile(
    v_start: f64,
    v_target: f64,
    max_acceleration: f64,
    max_jerk: f64,
) -> Result<String, JsError> {
    let params = dry_core::SCurveParams {
        v_start,
        v_target,
        max_acceleration,
        max_jerk,
    };
    let profile = dry_core::calculate_scurve_profile(&params).map_err(JsError::new)?;
    serde_json::to_string(&profile).map_err(|e| JsError::new(&e.to_string()))
}

/// Import an ISO 14649 STEP-NC document and lower it into Dry L1 operations JSON.
#[wasm_bindgen]
pub fn import_step_nc_to_ops(step_nc_text: &str) -> Result<String, JsError> {
    let steps = dry_core::parse_step_nc(step_nc_text).map_err(|e| JsError::new(&e))?;
    let mut all_ops = Vec::new();
    for step in &steps {
        all_ops.extend(dry_core::lower_workingstep_to_ops(step));
    }
    serde_json::to_string(&all_ops).map_err(|e| JsError::new(&e.to_string()))
}

/// Generate CNC Lathe Facing operations from parameters JSON.
#[wasm_bindgen]
pub fn generate_lathe_facing_ops_wasm(params_json: &str) -> Result<String, JsError> {
    let params: dry_core::LatheFacingParams = serde_json::from_str(params_json)
        .map_err(|e| JsError::new(&format!("lathe facing params: {e}")))?;
    let ops = dry_core::generate_lathe_facing_ops(&params).map_err(|e| JsError::new(&e))?;
    serde_json::to_string(&ops).map_err(|e| JsError::new(&e.to_string()))
}

/// Generate CNC Lathe OD Roughing & Finishing operations from parameters JSON.
#[wasm_bindgen]
pub fn generate_lathe_od_turning_ops_wasm(params_json: &str) -> Result<String, JsError> {
    let params: dry_core::LatheTurningParams = serde_json::from_str(params_json)
        .map_err(|e| JsError::new(&format!("lathe turning params: {e}")))?;
    let ops = dry_core::generate_lathe_od_turning_ops(&params).map_err(|e| JsError::new(&e))?;
    serde_json::to_string(&ops).map_err(|e| JsError::new(&e.to_string()))
}

/// Check toolpath for tool holder collision against stock volume bounds.
#[wasm_bindgen]
pub fn check_tool_holder_collision_wasm(
    toolpath_json: &str,
    holder_json: &str,
    stock_bounds_json: &str,
) -> Result<String, JsError> {
    let toolpath: dry_core::Toolpath =
        serde_json::from_str(toolpath_json).map_err(|e| JsError::new(&format!("toolpath: {e}")))?;
    let holder: dry_core::ToolHolder = serde_json::from_str(holder_json)
        .map_err(|e| JsError::new(&format!("tool holder: {e}")))?;
    let stock_bounds: [f64; 6] = serde_json::from_str(stock_bounds_json).map_err(|e| {
        JsError::new(&format!(
            "stock bounds [min_x, max_x, min_y, max_y, min_z, max_z]: {e}"
        ))
    })?;
    let findings = dry_core::check_tool_holder_collision(&toolpath, &holder, stock_bounds);
    serde_json::to_string(&findings).map_err(|e| JsError::new(&e.to_string()))
}

/// Reverse-engineer an L1 Design JSON from a resolved L2 Toolpath JSON.
#[wasm_bindgen]
pub fn reverse_toolpath_wasm(toolpath_json: &str) -> Result<String, JsError> {
    let toolpath: dry_core::Toolpath =
        serde_json::from_str(toolpath_json).map_err(|e| JsError::new(&format!("toolpath: {e}")))?;
    let design = dry_core::reverse::reverse(&toolpath).map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&design.ops).map_err(|e| JsError::new(&e.to_string()))
}

/// Slice an ISO 10303-21 STEP CAD solid directly into L1 ops JSON.
#[wasm_bindgen]
pub fn slice_step_solid_wasm(
    step_content: &str,
    z_start: f64,
    z_end: f64,
    layer_height: f64,
    samples_per_slice: usize,
    feedrate: f64,
) -> Result<String, JsError> {
    let solid = dry_core::BrepSolid::parse_step_iso10303(step_content)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let ops = solid
        .slice_to_l1_ops(z_start, z_end, layer_height, samples_per_slice, feedrate)
        .map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&ops).map_err(|e| JsError::new(&e.to_string()))
}

/// Slice a multi-solid B-Rep assembly into L1 ops JSON.
#[wasm_bindgen]
pub fn slice_brep_assembly_wasm(
    assembly_json: &str,
    z_start: f64,
    z_end: f64,
    layer_height: f64,
    samples_per_slice: usize,
    feedrate: f64,
) -> Result<String, JsError> {
    let step_solids: Vec<String> = serde_json::from_str(assembly_json)
        .map_err(|e| JsError::new(&format!("invalid assembly json: {e}")))?;
    let mut asm = dry_core::generate::BrepAssembly::new("wasm_brep_assembly");
    for step in step_solids {
        let solid = dry_core::BrepSolid::parse_step_iso10303(&step)
            .map_err(|e| JsError::new(&e.to_string()))?;
        asm.add_solid(solid, dry_core::generate::BrepBodyRole::AdditiveBody);
    }
    let ops = asm
        .slice_to_l1_ops(z_start, z_end, layer_height, samples_per_slice, feedrate)
        .map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&ops).map_err(|e| JsError::new(&e.to_string()))
}

/// Slice a multi-solid B-Rep assembly with CSG boolean void subtraction in Wasm.
#[wasm_bindgen]
pub fn slice_brep_assembly_csg_wasm(
    step_additives: Vec<String>,
    step_voids: Vec<String>,
    z_start: f64,
    z_end: f64,
    layer_height: f64,
    samples_per_slice: usize,
    feedrate: f64,
) -> Result<String, JsError> {
    let mut asm = dry_core::generate::BrepAssembly::new("wasm_csg_assembly");
    for step in step_additives {
        let solid = dry_core::BrepSolid::parse_step_iso10303(&step)
            .map_err(|e| JsError::new(&e.to_string()))?;
        asm.add_solid(solid, dry_core::generate::BrepBodyRole::AdditiveBody);
    }
    for step in step_voids {
        let solid = dry_core::BrepSolid::parse_step_iso10303(&step)
            .map_err(|e| JsError::new(&e.to_string()))?;
        asm.add_solid(solid, dry_core::generate::BrepBodyRole::SubtractiveVoid);
    }
    let ops = asm
        .slice_with_csg(z_start, z_end, layer_height, samples_per_slice, feedrate)
        .map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&ops).map_err(|e| JsError::new(&e.to_string()))
}

/// Optimize toolpath for Constant Material Removal Rate (MRR) in Wasm.
#[wasm_bindgen]
pub fn optimize_constant_mrr_wasm(
    ops_json: &str,
    params_json: &str,
    depth_of_cut: f64,
    target_mrr_mm3_min: f64,
    min_feedrate: f64,
    max_feedrate: f64,
) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let mut tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    dry_core::optimize::optimize_constant_mrr(
        &mut tp,
        depth_of_cut,
        target_mrr_mm3_min,
        min_feedrate,
        max_feedrate,
    );
    serde_json::to_string(&tp).map_err(|e| JsError::new(&e.to_string()))
}

/// Simulate 3D Dexel grid stock subtraction in Wasm and return the volumetric report.
#[wasm_bindgen]
pub fn simulate_dexel_stock_wasm(
    ops_json: &str,
    params_json: &str,
    min_x: f64,
    min_y: f64,
    min_z: f64,
    max_x: f64,
    max_y: f64,
    max_z: f64,
    resolution_mm: f64,
    tool_radius: f64,
    is_ballnose: bool,
) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    let mut stock =
        dry_core::DexelGrid::new_stock(min_x, min_y, min_z, max_x, max_y, max_z, resolution_mm)
            .map_err(|e| JsError::new(&e))?;
    stock
        .simulate_toolpath(&tp, tool_radius, is_ballnose)
        .map_err(|e| JsError::new(&e))?;
    let report = stock.generate_report();
    serde_json::to_string(&report).map_err(|e| JsError::new(&e.to_string()))
}

/// Generate L1 draping ops over a 3D mesh.
///
/// `options_json` is the serialized `DrapeOptions`, mesh included — the same wire form the Python
/// binding takes. Use [`parse_obj_mesh_wasm`] to turn OBJ text into the `mesh` field.
#[wasm_bindgen]
pub fn drape_ops_wasm(options_json: &str) -> Result<String, JsError> {
    let options: dry_core::generate::drape::DrapeOptions = serde_json::from_str(options_json)
        .map_err(|e| JsError::new(&format!("invalid drape options: {e}")))?;
    let ops =
        dry_core::generate::drape::drape_ops(&options).map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&ops).map_err(|e| JsError::new(&e.to_string()))
}

/// Parse OBJ text into serialized `TriangleMesh` JSON.
#[wasm_bindgen]
pub fn parse_obj_mesh_wasm(obj_text: &str) -> Result<String, JsError> {
    let mesh = dry_core::generate::drape::TriangleMesh::from_obj(obj_text)
        .map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&mesh).map_err(|e| JsError::new(&e.to_string()))
}

/// Run the digital-twin machining physics analysis and return the report as JSON.
///
/// `material` is the wire name of the workpiece material (`Aluminum6061`, `Steel4140`,
/// `TitaniumTi6Al4V`, `Inconel718`, `ThermoplasticPLA`, `ThermoplasticPEEK`); an unknown name is an
/// error rather than a default. The estimates are analytic and unvalidated against instrumented
/// cuts — see `docs/14-known-limitations.md`.
#[wasm_bindgen]
pub fn analyze_machining_physics_wasm(
    tool_json: &str,
    material: &str,
    params_json: &str,
) -> Result<String, JsError> {
    let tool: dry_core::CuttingToolGeometry = serde_json::from_str(tool_json)
        .map_err(|e| JsError::new(&format!("invalid tool geometry: {e}")))?;
    let params: dry_core::MachiningOperationParams = serde_json::from_str(params_json)
        .map_err(|e| JsError::new(&format!("invalid operation params: {e}")))?;
    let material: dry_core::WorkpieceMaterial = serde_json::from_str(&format!("\"{material}\""))
        .map_err(|e| JsError::new(&format!("unknown workpiece material: {e}")))?;
    let report = dry_core::analyze_machining_physics(&tool, material, &params);
    serde_json::to_string(&report).map_err(|e| JsError::new(&e.to_string()))
}

/// Apply the synchronised 5-axis jerk-limited lookahead optimiser to a toolpath.
#[wasm_bindgen]
pub fn optimize_five_axis_lookahead_wasm(
    toolpath_json: &str,
    params_json: &str,
) -> Result<String, JsError> {
    let tp: dry_core::Toolpath = serde_json::from_str(toolpath_json)
        .map_err(|e| JsError::new(&format!("invalid toolpath: {e}")))?;
    let params: dry_core::FiveAxisLookaheadParams = serde_json::from_str(params_json)
        .map_err(|e| JsError::new(&format!("invalid lookahead params: {e}")))?;
    let out = dry_core::optimize_five_axis_lookahead(&tp, &params);
    serde_json::to_string(&out).map_err(|e| JsError::new(&e.to_string()))
}

/// Calculate minimum Euclidean distance between two 3D line segments in Wasm.
#[wasm_bindgen]
pub fn segment_to_segment_distance_3d_wasm(
    p1: &[f64],
    p2: &[f64],
    q1: &[f64],
    q2: &[f64],
) -> Result<f64, JsError> {
    if p1.len() < 3 || p2.len() < 3 || q1.len() < 3 || q2.len() < 3 {
        return Err(JsError::new(
            "Each segment point must have at least 3 coordinates [x, y, z]",
        ));
    }
    let p1_arr = [p1[0], p1[1], p1[2]];
    let p2_arr = [p2[0], p2[1], p2[2]];
    let q1_arr = [q1[0], q1[1], q1[2]];
    let q2_arr = [q2[0], q2[1], q2[2]];
    Ok(dry_core::segment_to_segment_distance_3d(
        p1_arr, p2_arr, q1_arr, q2_arr,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kinematics_empty_returns_none() {
        assert!(parse_kinematics_inner("").unwrap().is_none());
        assert!(parse_kinematics_inner("   ").unwrap().is_none());
    }

    #[test]
    fn parse_kinematics_valid_json_returns_some() {
        let k = parse_kinematics_inner(r#"{"max_acceleration_mm_s2":3000}"#)
            .unwrap()
            .unwrap();
        assert_eq!(k.max_acceleration_mm_s2, Some(3000.0));
        assert_eq!(k.max_junction_velocity_mm_s, None);
    }

    #[test]
    fn parse_kinematics_both_fields() {
        let k = parse_kinematics_inner(
            r#"{"max_acceleration_mm_s2":5000,"max_junction_velocity_mm_s":10.0}"#,
        )
        .unwrap()
        .unwrap();
        assert_eq!(k.max_acceleration_mm_s2, Some(5000.0));
        assert_eq!(k.max_junction_velocity_mm_s, Some(10.0));
    }

    /// The rejection path, now runnable on the host.
    ///
    /// This assertion previously sat behind `#[cfg(target_arch = "wasm32")]` because evaluating a
    /// `JsError` panics off-wasm — and since the crate has no wasm test runner, it executed on no
    /// target at all. Asserting against `parse_kinematics_inner` keeps the rejection covered without
    /// crossing the wasm-bindgen boundary.
    #[test]
    fn parse_kinematics_invalid_json_is_rejected() {
        let err = parse_kinematics_inner("not-json").unwrap_err();
        assert!(
            err.contains("invalid kinematics_json"),
            "the message names the input it rejected: {err}"
        );
    }
}

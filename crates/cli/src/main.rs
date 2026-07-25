//! `dry` — the toolpath compiler CLI. Operates on a Dry IR file (`{version, segments}`, or a fixture
//! wrapping it under an `ir` key). Phase-0 surface: `inspect` / `simulate` / `emit` (`docs/04-tasks.md`).

use clap::{Parser, Subcommand, ValueEnum};
use dry_core::{
    apply_gated, emit_stream_to_writer, import_gcode_reader, import_gcode_reader_with_map,
    import_klipper, optimize_aggressive_pipeline, optimize_pipeline, parse_bounds_csv,
    parse_speed_range_csv, simulate, simulate_stream, trace_summary_with_sources, verify,
    verify_stream, Contracts, EmitParams, FirmwareFlavor, GcodeImportParams, Kinematics,
    OptimizeMode, Profile, RewriteReport, RewriteSpanResult, Toolpath,
};
use std::fs;
use std::io::Write;
use std::process::ExitCode;

/// CLI surface for the **rotary-axes** selector on `dry emit` (the `--rotary-axes` flag): which two
/// rotary axes (`ab`/`ac`/`bc`) carry the toolframe orientation for 5-axis words. This is the
/// ab/ac/bc STRING — distinct from the machine motion-limits `kinematics` OBJECT
/// (`--max-accel` / `--junction-velocity`) consumed by `verify` / `rewrite-gcode --mode balanced`.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum RotaryAxesArg {
    Ab,
    Ac,
    Bc,
}

/// CLI surface for [`OptimizeMode`]: the gated optimisation mode selectable on `dry rewrite-gcode`.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum OptimizeModeArg {
    Safe,
    Balanced,
    Max,
}

impl From<OptimizeModeArg> for OptimizeMode {
    fn from(m: OptimizeModeArg) -> Self {
        match m {
            OptimizeModeArg::Safe => OptimizeMode::Safe,
            OptimizeModeArg::Balanced => OptimizeMode::Balanced,
            OptimizeModeArg::Max => OptimizeMode::Max,
        }
    }
}

impl From<RotaryAxesArg> for Kinematics {
    fn from(k: RotaryAxesArg) -> Self {
        match k {
            RotaryAxesArg::Ab => Kinematics::Ab {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            },
            RotaryAxesArg::Ac => Kinematics::Ac {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            },
            RotaryAxesArg::Bc => Kinematics::Bc {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            },
        }
    }
}

#[derive(Parser)]
#[command(name = "dry", version, about = "Dry — toolpath compiler CLI")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Parse + simulate a Dry IR file and print a concise summary.
    Inspect { file: String },
    /// Simulate a Dry IR file and print its metrics.
    Simulate {
        file: String,
        /// Print metrics as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Emit motion g-code for a Dry IR file.
    Emit {
        file: String,
        /// Emit absolute extrusion (default is relative E).
        #[arg(long)]
        absolute_e: bool,
        /// Emit rotary words from the toolframe orientation (5-axis).
        #[arg(long)]
        five_axis: bool,
        /// Rotary axes (ab/ac/bc) that carry the toolframe orientation for 5-axis words. (Accepts the
        /// legacy `--kinematics` alias; this is the rotary-axes STRING, not the motion-limits object.)
        #[arg(long, visible_alias = "kinematics", value_enum, default_value_t = RotaryAxesArg::Ab)]
        rotary_axes: RotaryAxesArg,
        /// Write to a file instead of stdout.
        #[arg(short, long)]
        out: Option<String>,
    },
    /// Encode a Dry IR (JSON) file to the chunked streaming binary form.
    Pack {
        file: String,
        /// Output path for the `.dry` binary.
        #[arg(short, long)]
        out: String,
    },
    /// Decode a `.dry` binary back to Dry IR JSON (lossless).
    Unpack {
        file: String,
        /// Write JSON to a file instead of stdout.
        #[arg(short, long)]
        out: Option<String>,
    },
    /// Import a Klipper printer.cfg into a dry machine/material profile (kinematics, retraction, build volume).
    ImportPrinterCfg {
        file: String,
        #[arg(long)]
        out: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    /// Import slicer G-code into Dry IR JSON for review, simulation, verification and optimisation.
    ImportGcode {
        file: String,
        /// Machine/material profile JSON to supply import defaults.
        #[arg(long)]
        profile: Option<String>,
        /// Filament diameter in mm, used to recover deposited volume from E motion.
        #[arg(long)]
        filament_diameter: Option<f64>,
        /// Optional line width in mm to attach to extruding segments.
        #[arg(long)]
        line_width: Option<f64>,
        /// Optional layer height in mm to attach to extruding segments.
        #[arg(long)]
        layer_height: Option<f64>,
        /// Write Dry IR JSON to a file instead of stdout.
        #[arg(short, long)]
        out: Option<String>,
    },
    /// Review slicer G-code directly, reporting metrics and contract findings with source line numbers.
    ReviewGcode {
        file: String,
        /// Machine/material profile JSON to supply import defaults and verifier contracts.
        #[arg(long)]
        profile: Option<String>,
        /// Filament diameter in mm, used to recover deposited volume from E motion.
        #[arg(long)]
        filament_diameter: Option<f64>,
        /// Assumed line width in mm for structural bead and flow checks.
        #[arg(long)]
        line_width: Option<f64>,
        /// Assumed layer height in mm for structural bead and flow checks.
        #[arg(long)]
        layer_height: Option<f64>,
        /// Max volumetric flow (mm³/s).
        #[arg(long)]
        max_flow: Option<f64>,
        /// Build volume as `x0,x1,y0,y1,z0,z1` (mm).
        #[arg(long)]
        bounds: Option<String>,
        /// Require Z to be non-decreasing.
        #[arg(long)]
        monotonic_z: bool,
        /// Minimum nozzle temperature (°C) required to extrude.
        #[arg(long)]
        min_temp: Option<f64>,
        /// Allowed feedrate range `min,max` (mm/min) for extruding moves.
        #[arg(long)]
        speed_range: Option<String>,
        /// Maximum retraction distance (mm).
        #[arg(long)]
        max_retraction_distance: Option<f64>,
        /// Maximum retraction speed (mm/min).
        #[arg(long)]
        max_retraction_speed: Option<f64>,
        /// Maximum travel run distance (mm) allowed without a retraction.
        #[arg(long)]
        max_travel_without_retract: Option<f64>,
        /// Allowed first-layer Z height range `min,max` (mm).
        #[arg(long)]
        first_layer_height_range: Option<String>,
        /// Allowed first-layer feedrate range `min,max` (mm/min).
        #[arg(long)]
        first_layer_speed_range: Option<String>,
        /// Maximum toolhead acceleration (mm/s²) for the arc peak-acceleration check.
        #[arg(long)]
        max_accel: Option<f64>,
        /// Maximum junction (square-corner) velocity (mm/s) for the junction-velocity check.
        #[arg(long)]
        junction_velocity: Option<f64>,
        /// Print metrics/findings as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Summarize slicer G-code as fixed-window motion/time-series JSON.
    TraceGcode {
        file: String,
        /// Machine/material profile JSON to supply import defaults.
        #[arg(long)]
        profile: Option<String>,
        /// Filament diameter in mm, used to recover deposited volume from E motion.
        #[arg(long)]
        filament_diameter: Option<f64>,
        /// Optional line width in mm to attach to extruding segments.
        #[arg(long)]
        line_width: Option<f64>,
        /// Optional layer height in mm to attach to extruding segments.
        #[arg(long)]
        layer_height: Option<f64>,
        /// Fixed trace window duration in seconds.
        #[arg(long, default_value_t = 5.0)]
        window_s: f64,
    },
    /// Forensics: infer slicer behavior from G-code (slicer, features, layers, hotspots) with confidence tags.
    ForensicsGcode {
        file: String,
        /// Machine/material profile JSON to supply import defaults.
        #[arg(long)]
        profile: Option<String>,
        /// Filament diameter in mm, used to recover deposited volume from E motion.
        #[arg(long)]
        filament_diameter: Option<f64>,
        /// Optional line width in mm to attach to extruding segments.
        #[arg(long)]
        line_width: Option<f64>,
        /// Optional layer height in mm to attach to extruding segments.
        #[arg(long)]
        layer_height: Option<f64>,
        /// Print the full ForensicsReport as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Assemble an offline LLM-explanation bundle (trace + forensics + verify + a curated prompt).
    ///
    /// The engine never calls an LLM — `explain` produces a facts-plus-prompt briefing you paste into
    /// Claude (or hand to an agent). Markdown by default; `--json` emits the structured ExplainBundle.
    Explain {
        file: String,
        /// Machine/material profile JSON to supply import defaults and verifier contracts.
        #[arg(long)]
        profile: Option<String>,
        /// Filament diameter in mm, used to recover deposited volume from E motion.
        #[arg(long)]
        filament_diameter: Option<f64>,
        /// Assumed line width in mm for structural bead and flow checks.
        #[arg(long)]
        line_width: Option<f64>,
        /// Assumed layer height in mm for structural bead and flow checks.
        #[arg(long)]
        layer_height: Option<f64>,
        /// Max volumetric flow (mm³/s).
        #[arg(long)]
        max_flow: Option<f64>,
        /// Build volume as `x0,x1,y0,y1,z0,z1` (mm).
        #[arg(long)]
        bounds: Option<String>,
        /// Require Z to be non-decreasing.
        #[arg(long)]
        monotonic_z: bool,
        /// Minimum nozzle temperature (°C) required to extrude.
        #[arg(long)]
        min_temp: Option<f64>,
        /// Allowed feedrate range `min,max` (mm/min) for extruding moves.
        #[arg(long)]
        speed_range: Option<String>,
        /// Fixed trace window duration in seconds.
        #[arg(long, default_value_t = 5.0)]
        window_s: f64,
        /// Emit the structured ExplainBundle as JSON instead of Markdown.
        #[arg(long)]
        json: bool,
        /// Write the bundle to this file instead of stdout.
        #[arg(long)]
        out: Option<String>,
        /// Call Claude directly: build the bundle, get recommendations, apply the executable ones,
        /// and report measured before/after results. Requires --model and ANTHROPIC_API_KEY.
        #[arg(long)]
        llm: bool,
        /// Claude model id for --llm (e.g. claude-sonnet-4-6, claude-opus-4-8). Required with --llm.
        #[arg(long)]
        model: Option<String>,
        /// Cap on how many executable recommendations --llm actually applies (highest priority first).
        #[arg(long, default_value_t = 4)]
        max_applies: usize,
    },
    /// Diff two analysed g-code files: settings, time/flow, and safety findings (A → B).
    Compare {
        file_a: String,
        file_b: String,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        filament_diameter: Option<f64>,
        #[arg(long)]
        line_width: Option<f64>,
        #[arg(long)]
        layer_height: Option<f64>,
        #[arg(long, default_value_t = 5.0)]
        window_s: f64,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        out: Option<String>,
        /// Call the model directly: get a narrative over the forensic delta. Requires --model and ANTHROPIC_API_KEY.
        #[arg(long)]
        llm: bool,
        /// Claude model id for --llm (e.g. claude-sonnet-4-6). Required with --llm.
        #[arg(long)]
        model: Option<String>,
    },
    /// Re-emit imported motion while preserving non-motion source G-code lines in place.
    RewriteGcode {
        file: String,
        /// Machine/material profile JSON to supply import defaults.
        #[arg(long)]
        profile: Option<String>,
        /// Filament diameter in mm, used to recover deposited volume from E motion.
        #[arg(long)]
        filament_diameter: Option<f64>,
        /// Optional line width in mm to attach to extruding segments.
        #[arg(long)]
        line_width: Option<f64>,
        /// Optional layer height in mm to attach to extruding segments.
        #[arg(long)]
        layer_height: Option<f64>,
        /// Emit absolute extrusion (default is relative E).
        #[arg(long)]
        absolute_e: bool,
        /// Optimise each contiguous source motion span before splicing it back.
        #[arg(long)]
        optimize: bool,
        /// Also reorder independent extrusion runs to reduce travel. Changes print order.
        #[arg(long)]
        reorder_travel: bool,
        /// Gated optimisation mode (`safe`|`balanced`|`max`). When set, each motion span is rewritten
        /// only if it introduces no new verifier error: `safe` canonicalises geometry, `balanced` adds
        /// adaptive-speed shaping, `max` also reorders travel and adds z-hop.
        #[arg(long, value_enum)]
        mode: Option<OptimizeModeArg>,
        /// Emit a `RewriteReport` as JSON to stdout (requires `--out` for the rewritten G-code).
        #[arg(long)]
        json: bool,
        /// Write rewritten G-code to a file instead of stdout.
        #[arg(short, long)]
        out: Option<String>,
    },
    /// Optimise a Dry IR file (merge collinear, fit arcs) and report the before/after.
    Optimize {
        file: String,
        /// Also reorder independent extrusion runs to reduce travel. Changes print order.
        #[arg(long)]
        reorder_travel: bool,
        /// Write the optimised IR JSON to a file.
        #[arg(short, long)]
        out: Option<String>,
    },
    /// Verify a g-code file and upload it to a Moonraker host (accept/warn/reject gate).
    Upload {
        /// G-code file to review and upload.
        file: String,
        /// Moonraker base URL, for example `http://voron.local`.
        #[arg(long)]
        moonraker: String,
        /// Environment variable containing the optional Moonraker API key.
        #[arg(long, default_value = "MOONRAKER_API_KEY")]
        api_key_env: String,
        /// End-to-end timeout for each Moonraker request, in seconds.
        #[arg(
            long,
            default_value_t = 120,
            value_parser = clap::value_parser!(u64).range(1..)
        )]
        timeout_s: u64,
        /// Start printing after upload; requires a profile and a clean gate unless forced.
        #[arg(long)]
        print: bool,
        /// Explicitly override error/warning/profile gates. Can start unsafe machine motion.
        #[arg(long)]
        force: bool,
        /// Rewrite with the selected gated optimization mode before review and upload.
        #[arg(long, value_enum)]
        rewrite: Option<OptimizeModeArg>,
        /// Machine/material profile JSON used for import defaults and safety contracts.
        #[arg(long)]
        profile: Option<String>,
        /// Override the import filament diameter (mm).
        #[arg(long)]
        filament_diameter: Option<f64>,
        /// Override the imported bead width (mm).
        #[arg(long)]
        line_width: Option<f64>,
        /// Override the imported layer height (mm).
        #[arg(long)]
        layer_height: Option<f64>,
        /// Override maximum volumetric flow (mm³/s).
        #[arg(long)]
        max_flow: Option<f64>,
        /// Override build volume as `x0,x1,y0,y1,z0,z1` (mm).
        #[arg(long)]
        bounds: Option<String>,
        /// Require Z to be non-decreasing.
        #[arg(long)]
        monotonic_z: bool,
        /// Override minimum nozzle temperature required for extrusion (°C).
        #[arg(long)]
        min_temp: Option<f64>,
        /// Override allowed extrusion feedrate range as `min,max` (mm/min).
        #[arg(long)]
        speed_range: Option<String>,
        /// Emit the gate and upload outcome as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Check a Dry IR file against machine-safety contracts; exits 1 if any errors are found.
    /// Flags: `--bounds`, `--max-flow`, `--speed-range`, `--monotonic-z`, `--min-temp`,
    /// `--max-retraction-distance`, `--max-retraction-speed`, `--max-travel-without-retract`,
    /// `--first-layer-height-range`, `--first-layer-speed-range`, `--max-accel`,
    /// `--junction-velocity`, `--json`.
    Verify {
        file: String,
        /// Machine/material profile JSON to supply verifier contracts.
        #[arg(long)]
        profile: Option<String>,
        /// Max volumetric flow (mm³/s).
        #[arg(long)]
        max_flow: Option<f64>,
        /// Build volume as `x0,x1,y0,y1,z0,z1` (mm).
        #[arg(long)]
        bounds: Option<String>,
        /// Require Z to be non-decreasing (e.g. vase mode).
        #[arg(long)]
        monotonic_z: bool,
        /// Minimum nozzle temperature (°C) required to extrude.
        #[arg(long)]
        min_temp: Option<f64>,
        /// Allowed feedrate range `min,max` (mm/min) for extruding moves.
        #[arg(long)]
        speed_range: Option<String>,
        /// Maximum retraction distance (mm).
        #[arg(long)]
        max_retraction_distance: Option<f64>,
        /// Maximum retraction speed (mm/min).
        #[arg(long)]
        max_retraction_speed: Option<f64>,
        /// Maximum travel run distance (mm) allowed without a retraction.
        #[arg(long)]
        max_travel_without_retract: Option<f64>,
        /// Allowed first-layer Z height range `min,max` (mm).
        #[arg(long)]
        first_layer_height_range: Option<String>,
        /// Allowed first-layer feedrate range `min,max` (mm/min).
        #[arg(long)]
        first_layer_speed_range: Option<String>,
        /// Maximum toolhead acceleration (mm/s²) for the arc peak-acceleration check.
        #[arg(long)]
        max_accel: Option<f64>,
        /// Maximum junction (square-corner) velocity (mm/s) for the junction-velocity check.
        #[arg(long)]
        junction_velocity: Option<f64>,
        /// Print findings as JSON.
        #[arg(long)]
        json: bool,
    },
}

fn die(msg: String) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(2);
}

/// The wire label for an [`OptimizeMode`], used for the `RewriteReport.mode` string and stderr summary.
fn optimize_mode_label(mode: OptimizeMode) -> &'static str {
    match mode {
        OptimizeMode::Safe => "safe",
        OptimizeMode::Balanced => "balanced",
        OptimizeMode::Max => "max",
    }
}

/// Heuristic: does this text look like raw slicer G-code (rather than Dry IR JSON)? True when the first
/// meaningful line (skipping blanks and `;`/`(` comments) starts with a `G`/`M`/`T` word.
fn looks_like_gcode(text: &str) -> bool {
    for line in text.lines() {
        let t = line.trim_start();
        if t.is_empty() || t.starts_with(';') || t.starts_with('(') {
            continue;
        }
        let mut chars = t.chars();
        if let Some(c0) = chars.next() {
            if matches!(c0, 'G' | 'M' | 'T' | 'g' | 'm' | 't') {
                return chars.next().is_some_and(|c1| c1.is_ascii_digit());
            }
        }
        return false;
    }
    false
}

/// Die with an actionable hint when an IR command is handed raw G-code.
fn gcode_not_ir(file: &str) -> ! {
    die(format!(
        "{file} looks like raw G-code, not Dry IR JSON. Use `dry import-gcode {file}` to convert it to \
         Dry IR, or `dry review-gcode {file}` / `dry trace-gcode {file}` to work on it directly."
    ))
}

/// Load a Dry IR `Toolpath` from a file that is either a bare `{version, segments}` or a fixture with
/// an `ir` key.
fn load(file: &str) -> Toolpath {
    let text = fs::read_to_string(file).unwrap_or_else(|e| die(format!("cannot read {file}: {e}")));
    if looks_like_gcode(&text) {
        gcode_not_ir(file);
    }
    let v: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| die(format!("invalid JSON in {file}: {e}")));
    let ir = v.get("ir").cloned().unwrap_or(v);
    serde_json::from_value(ir).unwrap_or_else(|e| die(format!("not a Dry IR in {file}: {e}")))
}

fn load_streaming(
    file: &str,
) -> Result<
    Box<dyn Iterator<Item = Result<dry_core::Segment, dry_core::CodecError>>>,
    dry_core::CodecError,
> {
    let mut f =
        std::fs::File::open(file).map_err(|e| dry_core::CodecError::Other(e.to_string()))?;

    let mut magic = [0u8; 4];
    use std::io::Read;
    let bytes_read = f
        .read(&mut magic)
        .map_err(|e| dry_core::CodecError::Other(e.to_string()))?;

    use std::io::Seek;
    f.seek(std::io::SeekFrom::Start(0))
        .map_err(|e| dry_core::CodecError::Other(e.to_string()))?;

    if bytes_read == 4 && (magic == *b"DRY0" || magic == *b"DRY1") {
        let (_version, _meta, iter) = dry_core::decode_any_streaming(f)?;
        Ok(iter)
    } else {
        // Not a Dry binary: sniff a prefix for raw G-code mistakenly passed to an IR command.
        let mut prefix = [0u8; 256];
        let n = f
            .read(&mut prefix)
            .map_err(|e| dry_core::CodecError::Other(e.to_string()))?;
        f.seek(std::io::SeekFrom::Start(0))
            .map_err(|e| dry_core::CodecError::Other(e.to_string()))?;
        if looks_like_gcode(&String::from_utf8_lossy(&prefix[..n])) {
            gcode_not_ir(file);
        }
        let iter = dry_core::JsonSegmentsIterator::new(f);
        Ok(Box::new(iter))
    }
}

fn bbox(tp: &Toolpath) -> [[f64; 2]; 3] {
    let mut b = [[f64::INFINITY, f64::NEG_INFINITY]; 3];
    for s in &tp.segments {
        for (i, axis) in s.end.iter().enumerate() {
            if let Some(v) = axis {
                b[i][0] = b[i][0].min(v.value());
                b[i][1] = b[i][1].max(v.value());
            }
        }
    }
    b
}

fn run(cli: Cli) -> ExitCode {
    match cli.cmd {
        Cmd::Inspect { file } => {
            let tp = load(&file);
            let m = simulate(&tp);
            let b = bbox(&tp);
            println!("inspect: {file}");
            if let Some(meta) = &tp.meta {
                let gen = meta.generator.as_deref().unwrap_or("?");
                let units = meta.units.as_deref().unwrap_or("?");
                println!(
                    "  header:    generator {gen}, units {units}, invariants [{}]",
                    meta.invariants.join(", ")
                );
            }
            println!(
                "  segments:  {} ({} moves with length)",
                tp.segments.len(),
                m.segment_count
            );
            println!(
                "  time:      {:.1}s (print {:.1}s, travel {:.1}s)",
                m.total_time_s.value(),
                m.print_time_s.value(),
                m.travel_time_s.value()
            );
            println!(
                "  material:  {:.4}mm filament, {:.3}mm^3 deposited",
                m.filament_length.value(),
                m.extruded_volume.value()
            );
            println!(
                "  distance:  {:.1}mm extruding, {:.1}mm travel",
                m.extruding_distance.value(),
                m.travel_distance.value()
            );
            println!("  peak flow: {:.2}mm^3/s", m.max_flow_rate.value());
            println!(
                "  bbox:      X[{:.2}, {:.2}] Y[{:.2}, {:.2}] Z[{:.2}, {:.2}]",
                b[0][0], b[0][1], b[1][0], b[1][1], b[2][0], b[2][1]
            );
            ExitCode::SUCCESS
        }
        Cmd::Simulate { file, json } => {
            let stream =
                load_streaming(&file).unwrap_or_else(|e| die(format!("cannot stream {file}: {e}")));
            let m = simulate_stream(stream)
                .unwrap_or_else(|e| die(format!("cannot simulate {file}: {e}")));
            if json {
                println!("{}", serde_json::to_string_pretty(&m).unwrap());
            } else {
                println!(
                    "time {:.3}s | {} segments | {:.4}mm filament | {:.3}mm^3 | peak {:.2}mm^3/s",
                    m.total_time_s.value(),
                    m.segment_count,
                    m.filament_length.value(),
                    m.extruded_volume.value(),
                    m.max_flow_rate.value()
                );
            }
            ExitCode::SUCCESS
        }
        Cmd::Emit {
            file,
            absolute_e,
            five_axis,
            rotary_axes,
            out,
        } => {
            let stream =
                load_streaming(&file).unwrap_or_else(|e| die(format!("cannot stream {file}: {e}")));
            let params = EmitParams {
                relative_e: !absolute_e,
                travel_g1_e0: false,
                five_axis,
                kinematics: rotary_axes.into(),
                ..EmitParams::default()
            };
            match out {
                Some(path) => {
                    let out_file = fs::File::create(&path)
                        .unwrap_or_else(|e| die(format!("cannot write {path}: {e}")));
                    let mut writer = std::io::BufWriter::new(out_file);
                    emit_stream_to_writer(stream, &params, &mut writer)
                        .unwrap_or_else(|e| die(format!("cannot emit {file}: {e}")));
                    writeln!(writer).unwrap_or_else(|e| die(format!("cannot write {path}: {e}")));
                }
                None => {
                    let stdout = std::io::stdout();
                    let mut writer = stdout.lock();
                    emit_stream_to_writer(stream, &params, &mut writer)
                        .unwrap_or_else(|e| die(format!("cannot emit {file}: {e}")));
                    writeln!(writer).unwrap_or_else(|e| die(format!("cannot write stdout: {e}")));
                }
            }
            ExitCode::SUCCESS
        }
        Cmd::Pack { file, out } => {
            let bytes = load(&file)
                .try_to_streaming_bytes()
                .unwrap_or_else(|e| die(format!("cannot encode {file}: {e}")));
            fs::write(&out, &bytes).unwrap_or_else(|e| die(format!("cannot write {out}: {e}")));
            eprintln!("packed {file} → {out} ({} bytes)", bytes.len());
            ExitCode::SUCCESS
        }
        Cmd::Unpack { file, out } => {
            let bytes = fs::read(&file).unwrap_or_else(|e| die(format!("cannot read {file}: {e}")));
            let tp = Toolpath::from_bytes(&bytes)
                .unwrap_or_else(|e| die(format!("not a Dry IR binary {file}: {e}")));
            let json = tp.to_json();
            match out {
                Some(path) => fs::write(&path, json + "\n")
                    .unwrap_or_else(|e| die(format!("cannot write {path}: {e}"))),
                None => println!("{json}"),
            }
            ExitCode::SUCCESS
        }
        Cmd::ImportPrinterCfg { file, out, name } => {
            let text = fs::read_to_string(&file)
                .unwrap_or_else(|e| die(format!("cannot read {file}: {e}")));
            let (mut profile, warnings) =
                import_klipper(&text).unwrap_or_else(|e| die(format!("cannot import {file}: {e}")));
            if let Some(n) = name {
                profile.name = Some(n);
            }
            for w in &warnings {
                eprintln!("warning: {} — {}", w.field, w.message);
            }
            let json = serde_json::to_string_pretty(&profile).unwrap() + "\n";
            match out {
                Some(path) => fs::write(&path, json)
                    .unwrap_or_else(|e| die(format!("cannot write {path}: {e}"))),
                None => print!("{json}"),
            }
            ExitCode::SUCCESS
        }
        Cmd::ImportGcode {
            file,
            profile,
            filament_diameter,
            line_width,
            layer_height,
            out,
        } => {
            let input =
                fs::File::open(&file).unwrap_or_else(|e| die(format!("cannot read {file}: {e}")));
            let profile = load_profile(profile.as_deref());
            let params = gcode_import_params(
                profile.as_ref(),
                filament_diameter,
                line_width,
                layer_height,
            );
            let tp = import_gcode_reader(input, &params)
                .unwrap_or_else(|e| die(format!("cannot import {file}: {e}")));
            let json = tp.to_json();
            match out {
                Some(path) => fs::write(&path, json + "\n")
                    .unwrap_or_else(|e| die(format!("cannot write {path}: {e}"))),
                None => println!("{json}"),
            }
            ExitCode::SUCCESS
        }
        Cmd::ReviewGcode {
            file,
            profile,
            filament_diameter,
            line_width,
            layer_height,
            max_flow,
            bounds,
            monotonic_z,
            min_temp,
            speed_range,
            max_retraction_distance,
            max_retraction_speed,
            max_travel_without_retract,
            first_layer_height_range,
            first_layer_speed_range,
            max_accel,
            junction_velocity,
            json,
        } => {
            let input =
                fs::File::open(&file).unwrap_or_else(|e| die(format!("cannot read {file}: {e}")));
            let profile = load_profile(profile.as_deref());
            let params = gcode_review_params(
                profile.as_ref(),
                filament_diameter,
                line_width,
                layer_height,
            );
            let imported = import_gcode_reader_with_map(input, &params)
                .unwrap_or_else(|e| die(format!("cannot import {file}: {e}")));
            let metrics = simulate(&imported.toolpath);
            let contracts = contracts_from_inputs(
                profile.as_ref(),
                ContractOverrides {
                    bounds: bounds.as_deref(),
                    max_flow,
                    speed_range: speed_range.as_deref(),
                    monotonic_z,
                    min_temp,
                    max_retraction_distance,
                    max_retraction_speed,
                    max_travel_without_retract,
                    first_layer_height_range: first_layer_height_range.as_deref(),
                    first_layer_speed_range: first_layer_speed_range.as_deref(),
                    max_accel,
                    junction_velocity,
                },
            );
            let report = verify(&imported.toolpath, &contracts);
            let mut review = dry_core::ReviewReport::build(
                Some(file.clone()),
                profile_label(profile.as_ref()),
                imported.toolpath.segments.len(),
                metrics.clone(),
                &report,
                |segment| imported.source_line_for_segment(segment),
            );
            review.add_unmodeled_gcode(&imported);

            if json {
                println!("{}", serde_json::to_string_pretty(&review).unwrap());
            } else {
                println!("review-gcode: {file}");
                if let Some(label) = profile_label(profile.as_ref()) {
                    println!("  profile:   {label}");
                }
                println!(
                    "  segments:  {} ({} moves with length)",
                    imported.toolpath.segments.len(),
                    metrics.segment_count
                );
                println!(
                    "  time:      {:.1}s (print {:.1}s, travel {:.1}s)",
                    metrics.total_time_s.value(),
                    metrics.print_time_s.value(),
                    metrics.travel_time_s.value()
                );
                println!(
                    "  material:  {:.4}mm filament, {:.3}mm^3 deposited",
                    metrics.filament_length.value(),
                    metrics.extruded_volume.value()
                );
                println!("  peak flow: {:.2}mm^3/s", metrics.max_flow_rate.value());
                if review.findings.is_empty() {
                    println!("  verify:    OK (no findings)");
                } else {
                    for finding in &review.findings {
                        let seg = finding
                            .segment
                            .map(|i| format!(" seg {i}"))
                            .unwrap_or_default();
                        let line = finding
                            .source_line
                            .map(|line| format!(" line {line}"))
                            .unwrap_or_default();
                        println!(
                            "  [{:?}] {}{line}{seg}: {}",
                            finding.severity, finding.rule, finding.message
                        );
                    }
                    println!(
                        "  verify:    {} finding(s), {} error(s)",
                        review.findings.len(),
                        review.error_count
                    );
                }
            }

            if review.error_count == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Cmd::TraceGcode {
            file,
            profile,
            filament_diameter,
            line_width,
            layer_height,
            window_s,
        } => {
            let input =
                fs::File::open(&file).unwrap_or_else(|e| die(format!("cannot read {file}: {e}")));
            let profile = load_profile(profile.as_deref());
            let params = gcode_import_params(
                profile.as_ref(),
                filament_diameter,
                line_width,
                layer_height,
            );
            let imported = import_gcode_reader_with_map(input, &params)
                .unwrap_or_else(|e| die(format!("cannot import {file}: {e}")));
            let source_lines: Vec<_> = imported
                .segment_source_lines
                .iter()
                .copied()
                .map(Some)
                .collect();
            let trace = trace_summary_with_sources(&imported.toolpath, window_s, &source_lines)
                .unwrap_or_else(|e| die(format!("cannot trace {file}: {e}")));
            let report = dry_core::TraceReport {
                file: Some(file.clone()),
                profile: profile_label(profile.as_ref()),
                trace,
            };
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            ExitCode::SUCCESS
        }
        Cmd::ForensicsGcode {
            file,
            profile,
            filament_diameter,
            line_width,
            layer_height,
            json,
        } => {
            let input =
                fs::File::open(&file).unwrap_or_else(|e| die(format!("cannot read {file}: {e}")));
            let profile = load_profile(profile.as_deref());
            let params = gcode_import_params(
                profile.as_ref(),
                filament_diameter,
                line_width,
                layer_height,
            );
            let imported = import_gcode_reader_with_map(input, &params)
                .unwrap_or_else(|e| die(format!("cannot import {file}: {e}")));
            let report = dry_core::forensics_analyze(&imported);
            if json {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                println!("forensics-gcode: {file}");
                println!("  slicer:    {}", report.slicer);
                println!(
                    "  layers:    {} (height ~{})",
                    report.layers.layer_count,
                    report
                        .layers
                        .layer_height_mm
                        .value
                        .map(|v| format!("{v:.3}mm"))
                        .unwrap_or_else(|| "unknown".into())
                );
                println!(
                    "  line width: ~{}",
                    report
                        .line_width_mm
                        .value
                        .map(|v| format!("{v:.3}mm (inferred)"))
                        .unwrap_or_else(|| "unknown".into())
                );
                if let Some(m) = report.extrusion_multiplier.value {
                    println!("  extrusion×: ~{m:.3} (inferred)");
                }
                if !report.infill_angles_deg.is_empty() {
                    let angles: Vec<String> = report
                        .infill_angles_deg
                        .iter()
                        .map(|a| format!("{a:.0}°"))
                        .collect();
                    println!("  infill angle: {} (inferred)", angles.join("/"));
                }
                if let Some(sp) = report.infill_spacing_mm.value {
                    println!("  infill spacing: ~{sp:.3}mm (inferred)");
                }
                println!(
                    "  seam:      {} ({} loops, inferred)",
                    report.seam.strategy, report.seam.loops
                );
                if report.declared.extrusion_width_mm.is_some()
                    || report.declared.infill_angle_deg.is_some()
                {
                    println!(
                        "  declared:  width {:?}mm, infill {:?}°, density {:?} (from-comment)",
                        report.declared.extrusion_width_mm,
                        report.declared.infill_angle_deg,
                        report.declared.infill_density
                    );
                }
                println!("  features:");
                for f in &report.features {
                    println!(
                        "    {:<12} {:>4} segs, {:.1}mm, {:.0}-{:.0} mm/min, peak {:.2} mm³/s [{}]",
                        f.feature,
                        f.segments,
                        f.extruding_distance_mm,
                        f.min_speed_mm_min,
                        f.max_speed_mm_min,
                        f.peak_flow_mm3_s,
                        match f.source {
                            dry_core::Confidence::FromComment => "from-comment",
                            dry_core::Confidence::Measured => "measured",
                            dry_core::Confidence::Inferred => "inferred",
                        }
                    );
                }
                println!(
                    "  travel:    {} moves, {:.1}mm, {} retractions",
                    report.travel.travel_moves,
                    report.travel.travel_distance_mm,
                    report.travel.retractions
                );
                println!(
                    "  travel strategy: {} ({} z-hops, retract ratio {:.2}, inferred)",
                    report.travel_strategy.hint,
                    report.travel_strategy.z_hops,
                    report.travel_strategy.retraction_ratio
                );
                for h in &report.hotspots {
                    println!("  hotspot:   {} ({}) — {}", h.kind, h.count, h.note);
                }
            }
            ExitCode::SUCCESS
        }
        Cmd::Explain {
            file,
            profile,
            filament_diameter,
            line_width,
            layer_height,
            max_flow,
            bounds,
            monotonic_z,
            min_temp,
            speed_range,
            window_s,
            json,
            out,
            llm,
            model,
            max_applies,
        } => {
            if llm {
                return run_explain_llm(ExplainLlmArgs {
                    file,
                    profile,
                    filament_diameter,
                    line_width,
                    layer_height,
                    max_flow,
                    bounds,
                    monotonic_z,
                    min_temp,
                    speed_range,
                    window_s,
                    json,
                    out,
                    model,
                    max_applies,
                });
            }
            let a = assemble_explain(
                &file,
                profile.as_deref(),
                filament_diameter,
                line_width,
                layer_height,
                bounds.as_deref(),
                max_flow,
                speed_range.as_deref(),
                monotonic_z,
                min_temp,
                window_s,
            );
            let rendered = if json {
                serde_json::to_string_pretty(&a.bundle).unwrap() + "\n"
            } else {
                dry_core::render_markdown(&a.bundle)
            };
            match out {
                Some(path) => fs::write(&path, rendered)
                    .unwrap_or_else(|e| die(format!("cannot write {path}: {e}"))),
                None => print!("{rendered}"),
            }
            ExitCode::SUCCESS
        }
        Cmd::Compare {
            file_a,
            file_b,
            profile,
            filament_diameter,
            line_width,
            layer_height,
            window_s,
            json,
            out,
            llm,
            model,
        } => {
            if llm {
                return run_compare_llm(CompareLlmArgs {
                    file_a,
                    file_b,
                    profile,
                    filament_diameter,
                    line_width,
                    layer_height,
                    window_s,
                    json,
                    out,
                    model,
                });
            }
            let a = assemble_explain(
                &file_a,
                profile.as_deref(),
                filament_diameter,
                line_width,
                layer_height,
                None,
                None,
                None,
                false,
                None,
                window_s,
            );
            let b = assemble_explain(
                &file_b,
                profile.as_deref(),
                filament_diameter,
                line_width,
                layer_height,
                None,
                None,
                None,
                false,
                None,
                window_s,
            );
            let delta = dry_core::compare_reports(&a.bundle.reports, &b.bundle.reports);
            let rendered = if json {
                serde_json::to_string_pretty(&delta).unwrap() + "\n"
            } else {
                dry_core::render_compare_markdown(&delta)
            };
            match out {
                Some(path) => fs::write(&path, rendered)
                    .unwrap_or_else(|e| die(format!("cannot write {path}: {e}"))),
                None => print!("{rendered}"),
            }
            ExitCode::SUCCESS
        }
        Cmd::RewriteGcode {
            file,
            profile,
            filament_diameter,
            line_width,
            layer_height,
            absolute_e,
            optimize,
            reorder_travel,
            mode,
            json,
            out,
        } => {
            let mode = mode.map(OptimizeMode::from);
            if json && out.is_none() {
                die(
                    "--json requires --out: the rewritten G-code is written to the --out file \
                     while the RewriteReport goes to stdout"
                        .into(),
                );
            }
            let input =
                fs::File::open(&file).unwrap_or_else(|e| die(format!("cannot read {file}: {e}")));
            let profile = load_profile(profile.as_deref());
            let params = gcode_import_params(
                profile.as_ref(),
                filament_diameter,
                line_width,
                layer_height,
            );
            let imported = import_gcode_reader_with_map(input, &params)
                .unwrap_or_else(|e| die(format!("cannot import {file}: {e}")));
            let emit_params = EmitParams {
                relative_e: !absolute_e,
                travel_g1_e0: false,
                five_axis: false,
                kinematics: Kinematics::default(),
                flavor: profile
                    .as_ref()
                    .map(|p| p.emit_params().flavor)
                    .unwrap_or(FirmwareFlavor::Marlin),
            };

            let span_tp = |range: std::ops::Range<usize>| Toolpath {
                version: imported.toolpath.version,
                meta: imported.toolpath.meta.clone(),
                segments: imported.toolpath.segments[range].to_vec(),
            };

            if let Some(mode) = mode {
                // gated optimisation: rewrite each motion span only if it introduces no new verifier
                // error under the active profile contracts.
                let mode_label = optimize_mode_label(mode);
                let contracts =
                    contracts_from_inputs(profile.as_ref(), ContractOverrides::default());
                // `balanced` consumes the active profile's deterministic kinematic limits (max
                // acceleration / junction velocity); `safe`/`max` ignore them.
                let kinematics = profile.as_ref().and_then(|p| p.machine.kinematics.as_ref());
                if profile.is_none() {
                    eprintln!(
                        "warning: rewrite-gcode --mode {mode_label} with no --profile — the safety \
                         gate has no machine contracts, so only structural invariants (finite/bead/\
                         arc/travel-extrudes) are checked"
                    );
                }
                let mut span_toolpaths = Vec::new();
                let mut before_segs = Vec::new();
                let mut after_segs = Vec::new();
                let mut span_results = Vec::new();
                for (index, span) in imported.motion_spans().into_iter().enumerate() {
                    let span_toolpath = span_tp(span.segment_range());
                    let segment_count_before = span_toolpath.segments.len();
                    let result = apply_gated(&span_toolpath, &contracts, mode, kinematics);
                    before_segs.extend(span_toolpath.segments.iter().cloned());
                    after_segs.extend(result.toolpath.segments.iter().cloned());
                    span_results.push(RewriteSpanResult {
                        span_index: index,
                        accepted: result.accepted,
                        segment_count_before,
                        segment_count_after: result.toolpath.segments.len(),
                        new_error_rules: result.new_error_rules,
                    });
                    span_toolpaths.push(result.toolpath);
                }
                let rewritten_lines = imported
                    .emit_source_preserving_spans(&span_toolpaths, &emit_params)
                    .unwrap_or_else(|e| die(format!("cannot rewrite {file}: {e}")));
                let rewritten = rewritten_lines.join("\n");

                let before_tp = Toolpath {
                    version: imported.toolpath.version,
                    meta: imported.toolpath.meta.clone(),
                    segments: before_segs,
                };
                let after_tp = Toolpath {
                    version: imported.toolpath.version,
                    meta: imported.toolpath.meta.clone(),
                    segments: after_segs,
                };
                let report = RewriteReport::build(
                    Some(file.clone()),
                    profile_label(profile.as_ref()),
                    mode_label.to_string(),
                    &before_tp,
                    &after_tp,
                    span_results,
                );

                if json {
                    println!("{}", serde_json::to_string_pretty(&report).unwrap());
                    let path = out.expect("--out is required with --json (checked above)");
                    fs::write(&path, rewritten + "\n")
                        .unwrap_or_else(|e| die(format!("cannot write {path}: {e}")));
                } else {
                    eprintln!(
                        "{mode_label} mode: {} spans — {} accepted, {} rejected",
                        report.spans_total, report.spans_accepted, report.spans_rejected
                    );
                    for span in report.spans.iter().filter(|s| !s.accepted) {
                        eprintln!(
                            "  span {} rejected: would introduce {}",
                            span.span_index,
                            span.new_error_rules.join(", ")
                        );
                    }
                    match out {
                        Some(path) => fs::write(&path, rewritten + "\n")
                            .unwrap_or_else(|e| die(format!("cannot write {path}: {e}"))),
                        None => println!("{rewritten}"),
                    }
                }
                ExitCode::SUCCESS
            } else {
                // No mode: passthrough, or the legacy ungated `--optimize` geometry pipeline.
                let span_toolpaths = imported
                    .motion_spans()
                    .into_iter()
                    .map(|span| {
                        let span_toolpath = span_tp(span.segment_range());
                        if optimize {
                            if reorder_travel {
                                optimize_aggressive_pipeline(&span_toolpath)
                            } else {
                                optimize_pipeline(&span_toolpath)
                            }
                        } else {
                            span_toolpath
                        }
                    })
                    .collect::<Vec<_>>();
                let rewritten_lines = imported
                    .emit_source_preserving_spans(&span_toolpaths, &emit_params)
                    .unwrap_or_else(|e| die(format!("cannot rewrite {file}: {e}")));
                let rewritten = rewritten_lines.join("\n");
                match out {
                    Some(path) => fs::write(&path, rewritten + "\n")
                        .unwrap_or_else(|e| die(format!("cannot write {path}: {e}"))),
                    None => println!("{rewritten}"),
                }
                ExitCode::SUCCESS
            }
        }
        Cmd::Optimize {
            file,
            reorder_travel,
            out,
        } => {
            let tp = load(&file);
            let before = tp.segments.len();
            let opt = if reorder_travel {
                optimize_aggressive_pipeline(&tp)
            } else {
                optimize_pipeline(&tp)
            };
            let after = opt.segments.len();
            let m0 = simulate(&tp);
            let m1 = simulate(&opt);
            // total travel distance (sum of `length` over travel segments), before → after.
            let travel = |t: &Toolpath| -> f64 {
                t.segments
                    .iter()
                    .filter(|s| s.travel)
                    .map(|s| s.length.value())
                    .sum()
            };
            let (travel_before, travel_after) = (travel(&tp), travel(&opt));
            eprintln!(
                "optimize: {file} — {before} → {after} segments (−{}); \
                 travel {travel_before:.2}mm → {travel_after:.2}mm; \
                 volume {:.4}mm^3 (Δ{:.2e}), time {:.3}s (Δ{:.2e})",
                before - after,
                m1.extruded_volume.value(),
                (m1.extruded_volume.value() - m0.extruded_volume.value()).abs(),
                m1.total_time_s.value(),
                (m1.total_time_s.value() - m0.total_time_s.value()).abs(),
            );
            if let Some(path) = out {
                fs::write(&path, opt.to_json() + "\n")
                    .unwrap_or_else(|e| die(format!("cannot write {path}: {e}")));
            }
            ExitCode::SUCCESS
        }
        Cmd::Upload {
            file,
            moonraker,
            api_key_env,
            timeout_s,
            print,
            force,
            rewrite,
            profile,
            filament_diameter,
            line_width,
            layer_height,
            max_flow,
            bounds,
            monotonic_z,
            min_temp,
            speed_range,
            json,
        } => run_upload(UploadArgs {
            file,
            moonraker,
            api_key_env,
            timeout_s,
            print,
            force,
            rewrite,
            profile,
            filament_diameter,
            line_width,
            layer_height,
            max_flow,
            bounds,
            monotonic_z,
            min_temp,
            speed_range,
            json,
        }),
        Cmd::Verify {
            file,
            profile,
            max_flow,
            bounds,
            monotonic_z,
            min_temp,
            speed_range,
            max_retraction_distance,
            max_retraction_speed,
            max_travel_without_retract,
            first_layer_height_range,
            first_layer_speed_range,
            max_accel,
            junction_velocity,
            json,
        } => {
            let stream =
                load_streaming(&file).unwrap_or_else(|e| die(format!("cannot stream {file}: {e}")));
            let profile = load_profile(profile.as_deref());
            let contracts = contracts_from_inputs(
                profile.as_ref(),
                ContractOverrides {
                    bounds: bounds.as_deref(),
                    max_flow,
                    speed_range: speed_range.as_deref(),
                    monotonic_z,
                    min_temp,
                    max_retraction_distance,
                    max_retraction_speed,
                    max_travel_without_retract,
                    first_layer_height_range: first_layer_height_range.as_deref(),
                    first_layer_speed_range: first_layer_speed_range.as_deref(),
                    max_accel,
                    junction_velocity,
                },
            );
            let report = verify_stream(stream, &contracts)
                .unwrap_or_else(|e| die(format!("cannot verify {file}: {e}")));
            if json {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else if report.findings.is_empty() {
                println!("verify: {file} — OK (no findings)");
            } else {
                for f in &report.findings {
                    let seg = f.segment.map(|i| format!(" seg {i}")).unwrap_or_default();
                    println!("  [{:?}] {}{seg}: {}", f.severity, f.rule, f.message);
                }
                println!(
                    "verify: {file} — {} finding(s), {} error(s)",
                    report.findings.len(),
                    report.error_count()
                );
            }
            if report.ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
    }
}

fn load_profile(path: Option<&str>) -> Option<Profile> {
    path.map(|path| {
        let text =
            fs::read_to_string(path).unwrap_or_else(|e| die(format!("cannot read {path}: {e}")));
        Profile::from_json(&text).unwrap_or_else(|e| die(format!("bad --profile {path}: {e}")))
    })
}

fn profile_label(profile: Option<&Profile>) -> Option<String> {
    profile.map(|profile| {
        profile
            .name
            .clone()
            .or_else(|| {
                profile
                    .firmware
                    .flavor
                    .as_ref()
                    .map(|flavor| format!("{flavor} profile"))
            })
            .unwrap_or_else(|| "unnamed profile".to_string())
    })
}

fn gcode_import_params(
    profile: Option<&Profile>,
    filament_diameter: Option<f64>,
    line_width: Option<f64>,
    layer_height: Option<f64>,
) -> GcodeImportParams {
    let mut params = profile
        .map(Profile::gcode_import_params)
        .unwrap_or_default();
    if let Some(filament_diameter) = filament_diameter {
        params.filament_diameter = filament_diameter;
    }
    if let Some(line_width) = line_width {
        params.line_width = Some(line_width);
    }
    if let Some(layer_height) = layer_height {
        params.layer_height = Some(layer_height);
    }
    params
}

fn gcode_review_params(
    profile: Option<&Profile>,
    filament_diameter: Option<f64>,
    line_width: Option<f64>,
    layer_height: Option<f64>,
) -> GcodeImportParams {
    let mut params = gcode_import_params(profile, filament_diameter, line_width, layer_height);
    params.line_width = params.line_width.or(Some(0.45));
    params.layer_height = params.layer_height.or(Some(0.2));
    params
}

/// CLI overrides for the verifier [`Contracts`]. Each set field overrides the corresponding
/// profile-sourced value — mirroring how `--max-flow`/`--bounds` already compose with `--profile`:
/// the profile supplies the baseline, a set flag wins. Unset fields leave the profile value intact.
/// The kinematic flags override only their sub-field, preserving the profile's other kinematic limit.
#[derive(Default)]
struct ContractOverrides<'a> {
    bounds: Option<&'a str>,
    max_flow: Option<f64>,
    speed_range: Option<&'a str>,
    monotonic_z: bool,
    min_temp: Option<f64>,
    max_retraction_distance: Option<f64>,
    max_retraction_speed: Option<f64>,
    max_travel_without_retract: Option<f64>,
    first_layer_height_range: Option<&'a str>,
    first_layer_speed_range: Option<&'a str>,
    max_accel: Option<f64>,
    junction_velocity: Option<f64>,
}

fn contracts_from_inputs(profile: Option<&Profile>, overrides: ContractOverrides) -> Contracts {
    let mut contracts = profile.map(Profile::contracts).unwrap_or_default();
    if let Some(bounds) = overrides.bounds {
        contracts.bounds = Some(parse_bounds(bounds));
    }
    if let Some(max_flow) = overrides.max_flow {
        contracts.max_flow = Some(max_flow);
    }
    if let Some(speed_range) = overrides.speed_range {
        contracts.speed_range = Some(parse_speed_range(speed_range));
    }
    if overrides.monotonic_z {
        contracts.monotonic_z = true;
    }
    if let Some(min_temp) = overrides.min_temp {
        contracts.min_temp = Some(min_temp);
    }
    if let Some(max_retraction_distance) = overrides.max_retraction_distance {
        contracts.max_retraction_distance = Some(max_retraction_distance);
    }
    if let Some(max_retraction_speed) = overrides.max_retraction_speed {
        contracts.max_retraction_speed = Some(max_retraction_speed);
    }
    if let Some(max_travel_without_retract) = overrides.max_travel_without_retract {
        contracts.max_travel_without_retract = Some(max_travel_without_retract);
    }
    if let Some(range) = overrides.first_layer_height_range {
        contracts.first_layer_height_range = Some(parse_range("first-layer-height-range", range));
    }
    if let Some(range) = overrides.first_layer_speed_range {
        contracts.first_layer_speed_range = Some(parse_range("first-layer-speed-range", range));
    }
    // Kinematics: a set flag overrides only its sub-field, keeping any profile-sourced limit for the
    // other. Build a fresh `KinematicContracts` when the profile carried none.
    if overrides.max_accel.is_some() || overrides.junction_velocity.is_some() {
        let mut kinematics = contracts.kinematics.unwrap_or_default();
        if let Some(max_accel) = overrides.max_accel {
            kinematics.max_acceleration_mm_s2 = Some(max_accel);
        }
        if let Some(junction_velocity) = overrides.junction_velocity {
            kinematics.max_junction_velocity_mm_s = Some(junction_velocity);
        }
        contracts.kinematics = Some(kinematics);
    }
    contracts
}

/// Parse `x0,x1,y0,y1,z0,z1` into a build volume; exits 2 on a malformed value.
fn parse_bounds(s: &str) -> [[f64; 2]; 3] {
    parse_bounds_csv(s).unwrap_or_else(|e| die(format!("bad --bounds: {e}")))
}

/// Parse `min,max` into a speed range; exits 2 on a malformed value.
fn parse_speed_range(s: &str) -> [f64; 2] {
    parse_speed_range_csv(s).unwrap_or_else(|e| die(format!("bad --speed-range: {e}")))
}

/// Parse `min,max` into a two-element range, attributing a malformed value to `--{flag}`; exits 2.
fn parse_range(flag: &str, s: &str) -> [f64; 2] {
    parse_speed_range_csv(s).unwrap_or_else(|e| die(format!("bad --{flag}: {e}")))
}

/// Assembled inputs needed by the offline `explain` renderer and the LLM handler.
// Fields `imported`, `contracts`, `profile`, `profiled` are consumed only by the
// `#[cfg(feature = "llm")]` handler; suppress the dead-code lint in default builds.
#[cfg_attr(not(feature = "llm"), allow(dead_code))]
struct ExplainAssembly {
    imported: dry_core::ImportedGcode,
    contracts: Contracts,
    profile: Option<Profile>,
    profiled: bool,
    bundle: dry_core::ExplainBundle,
}

/// Import, simulate, verify, trace, forensics, and build the explain bundle.
/// Shared by the offline `Cmd::Explain` arm and `run_explain_llm`.
#[allow(clippy::too_many_arguments)]
fn assemble_explain(
    file: &str,
    profile_path: Option<&str>,
    filament_diameter: Option<f64>,
    line_width: Option<f64>,
    layer_height: Option<f64>,
    bounds: Option<&str>,
    max_flow: Option<f64>,
    speed_range: Option<&str>,
    monotonic_z: bool,
    min_temp: Option<f64>,
    window_s: f64,
) -> ExplainAssembly {
    let input = fs::File::open(file).unwrap_or_else(|e| die(format!("cannot read {file}: {e}")));
    let profile = load_profile(profile_path);
    let params = gcode_review_params(
        profile.as_ref(),
        filament_diameter,
        line_width,
        layer_height,
    );
    let imported = import_gcode_reader_with_map(input, &params)
        .unwrap_or_else(|e| die(format!("cannot import {file}: {e}")));

    // verify against the profile's contracts (+ any CLI overrides).
    let metrics = simulate(&imported.toolpath);
    let profiled = profile.is_some();
    let contracts = contracts_from_inputs(
        profile.as_ref(),
        ContractOverrides {
            bounds,
            max_flow,
            speed_range,
            monotonic_z,
            min_temp,
            ..ContractOverrides::default()
        },
    );
    let report = verify(&imported.toolpath, &contracts);
    let label = profile_label(profile.as_ref());
    let mut review = dry_core::ReviewReport::build(
        Some(file.to_string()),
        label.clone(),
        imported.toolpath.segments.len(),
        metrics,
        &report,
        |segment| imported.source_line_for_segment(segment),
    );
    review.add_unmodeled_gcode(&imported);

    // trace (carrying source-line ranges) + forensics.
    let source_lines: Vec<_> = imported
        .segment_source_lines
        .iter()
        .copied()
        .map(Some)
        .collect();
    let trace = trace_summary_with_sources(&imported.toolpath, window_s, &source_lines)
        .unwrap_or_else(|e| die(format!("cannot trace {file}: {e}")));
    let trace_report = dry_core::TraceReport {
        file: Some(file.to_string()),
        profile: label.clone(),
        trace,
    };
    let forensics = dry_core::forensics_analyze(&imported);

    let bundle = dry_core::build_explain_bundle(
        Some(file.to_string()),
        label,
        profiled,
        dry_core::ExplainReports {
            trace: trace_report,
            forensics,
            verify: review,
        },
    );

    ExplainAssembly {
        imported,
        contracts,
        profile,
        profiled,
        bundle,
    }
}

fn main() -> ExitCode {
    run(Cli::parse())
}

// Fields are consumed by `run_explain_llm`; suppress dead-code in non-llm builds.
#[cfg_attr(not(feature = "llm"), allow(dead_code))]
struct ExplainLlmArgs {
    file: String,
    profile: Option<String>,
    filament_diameter: Option<f64>,
    line_width: Option<f64>,
    layer_height: Option<f64>,
    max_flow: Option<f64>,
    bounds: Option<String>,
    monotonic_z: bool,
    min_temp: Option<f64>,
    speed_range: Option<String>,
    window_s: f64,
    json: bool,
    out: Option<String>,
    model: Option<String>,
    max_applies: usize,
}

#[cfg(not(feature = "llm"))]
fn run_explain_llm(_args: ExplainLlmArgs) -> std::process::ExitCode {
    die(
        "this build was compiled without --llm support; rebuild with `cargo build --features llm`"
            .into(),
    )
}

#[cfg(feature = "llm")]
fn run_explain_llm(args: ExplainLlmArgs) -> std::process::ExitCode {
    use dry_core::{apply_executable, classify, Classified};

    let model = args.model.unwrap_or_else(|| {
        die("--llm requires --model <id> (e.g. --model claude-sonnet-4-6)".into())
    });
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .unwrap_or_else(|_| die("set ANTHROPIC_API_KEY to use --llm".into()));

    // 1. Build the bundle exactly as the offline path does.
    let a = assemble_explain(
        &args.file,
        args.profile.as_deref(),
        args.filament_diameter,
        args.line_width,
        args.layer_height,
        args.bounds.as_deref(),
        args.max_flow,
        args.speed_range.as_deref(),
        args.monotonic_z,
        args.min_temp,
        args.window_s,
    );

    // 2. Call Claude.
    let cfg = dry_llm::ClientConfig {
        api_key,
        model: model.clone(),
        max_tokens: 8192,
    };
    let analysis = dry_llm::analyze(&cfg, &a.bundle).unwrap_or_else(|e| die(e.to_string()));

    // 3. Cost readout (stderr).
    match dry_llm::cost_usd(&model, &analysis.usage) {
        Some(c) => eprintln!(
            "{model} · in {} tok / out {} tok · ~${c:.4}",
            analysis.usage.input_tokens, analysis.usage.output_tokens
        ),
        None => eprintln!(
            "{model} · in {} tok / out {} tok · (pricing unknown for {model})",
            analysis.usage.input_tokens, analysis.usage.output_tokens
        ),
    }

    // 4. Classify, then apply executable recommendations (highest priority first, capped).
    let kinematics = a
        .profile
        .as_ref()
        .and_then(|p| p.machine.kinematics.as_ref());
    let mut recs: Vec<_> = analysis.recommendations.iter().collect();
    recs.sort_by_key(|r| r.priority);
    let mut results: Vec<(String, dry_core::ExecutionResult)> = Vec::new();
    let mut advisories: Vec<(String, String)> = Vec::new();
    let mut applied = 0usize;
    let mut skipped = 0usize;
    for rec in &recs {
        match classify(rec) {
            Classified::Executable(action) => {
                if applied >= args.max_applies {
                    skipped += 1;
                    continue;
                }
                let result = apply_executable(
                    &action,
                    &a.imported,
                    &a.contracts,
                    kinematics,
                    args.window_s,
                );
                results.push((rec.title.clone(), result));
                applied += 1;
            }
            Classified::Advisory(reason) => advisories.push((rec.title.clone(), reason)),
        }
    }
    if skipped > 0 {
        eprintln!(
            "note: {skipped} executable recommendation(s) skipped (over --max-applies {})",
            args.max_applies
        );
    }

    // 5. Render.
    let rendered = if args.json {
        let envelope = serde_json::json!({
            "meta": { "file": args.file, "model": model, "profiled": a.profiled },
            "analysis": {
                "summary": analysis.summary,
                "time_analysis": analysis.time_analysis,
                "risks": analysis.risks,
            },
            "recommendations": analysis.recommendations,
            "results": results.iter().map(|(title, r)| serde_json::json!({ "title": title, "result": r })).collect::<Vec<_>>(),
            "usage": {
                "input_tokens": analysis.usage.input_tokens,
                "output_tokens": analysis.usage.output_tokens,
            },
            "cost_usd": dry_llm::cost_usd(&model, &analysis.usage),
        });
        serde_json::to_string_pretty(&envelope).unwrap() + "\n"
    } else {
        render_llm_markdown(&args.file, &model, &analysis, &results, &advisories)
    };
    match args.out {
        Some(path) => {
            fs::write(&path, rendered).unwrap_or_else(|e| die(format!("cannot write {path}: {e}")))
        }
        None => print!("{rendered}"),
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(feature = "llm")]
fn render_llm_markdown(
    file: &str,
    model: &str,
    analysis: &dry_llm::AnalysisResponse,
    results: &[(String, dry_core::ExecutionResult)],
    advisories: &[(String, String)],
) -> String {
    use std::fmt::Write as _;
    let mut md = String::new();
    let _ = writeln!(md, "# Dry explain --llm — {file}  (model {model})\n");
    let _ = writeln!(md, "## Summary\n\n{}\n", analysis.summary);
    let _ = writeln!(md, "## Time analysis\n\n{}\n", analysis.time_analysis);
    let _ = writeln!(md, "## Risks\n\n{}\n", analysis.risks);
    let _ = writeln!(md, "## Results — measured by dry\n");
    let _ = writeln!(md, "| Change | Status | Measured |");
    let _ = writeln!(md, "|---|---|---|");
    for (title, r) in results {
        let _ = writeln!(
            md,
            "| {title} | {} ({:?}) | {} |",
            r.action, r.verdict, r.note
        );
    }
    for (title, reason) in advisories {
        let _ = writeln!(
            md,
            "| {title} | advisory — unverified | {reason}; apply in your slicer |"
        );
    }
    md
}

// Fields are consumed by `run_compare_llm`; suppress dead-code in non-llm builds.
#[cfg_attr(not(feature = "llm"), allow(dead_code))]
struct CompareLlmArgs {
    file_a: String,
    file_b: String,
    profile: Option<String>,
    filament_diameter: Option<f64>,
    line_width: Option<f64>,
    layer_height: Option<f64>,
    window_s: f64,
    json: bool,
    out: Option<String>,
    model: Option<String>,
}

#[cfg(not(feature = "llm"))]
fn run_compare_llm(_args: CompareLlmArgs) -> std::process::ExitCode {
    die(
        "this build was compiled without --llm support; rebuild with `cargo build --features llm`"
            .into(),
    )
}

// Fields are consumed by the `#[cfg(feature = "moonraker")]` `run_upload`; without the feature the
// stub ignores them, so suppress dead-code only in that build.
#[cfg_attr(not(feature = "moonraker"), allow(dead_code))]
struct UploadArgs {
    file: String,
    moonraker: String,
    api_key_env: String,
    timeout_s: u64,
    print: bool,
    force: bool,
    rewrite: Option<OptimizeModeArg>,
    profile: Option<String>,
    filament_diameter: Option<f64>,
    line_width: Option<f64>,
    layer_height: Option<f64>,
    max_flow: Option<f64>,
    bounds: Option<String>,
    monotonic_z: bool,
    min_temp: Option<f64>,
    speed_range: Option<String>,
    json: bool,
}

#[cfg(not(feature = "moonraker"))]
fn run_upload(_: UploadArgs) -> std::process::ExitCode {
    die(
        "this build was compiled without moonraker support; rebuild with `cargo build --features moonraker`"
            .into(),
    )
}

#[cfg(feature = "moonraker")]
fn run_upload(args: UploadArgs) -> std::process::ExitCode {
    use std::io::Cursor;
    use std::path::Path;
    use std::time::Duration;

    let api_key = std::env::var(&args.api_key_env).ok();
    let profile = load_profile(args.profile.as_deref());
    if profile.is_none() {
        eprintln!(
            "warning: dry upload with no --profile — only structural invariants are checked \
             (no flow/bounds/speed contracts); the gate will accept most files"
        );
    }
    let params = gcode_review_params(
        profile.as_ref(),
        args.filament_diameter,
        args.line_width,
        args.layer_height,
    );
    let contracts = contracts_from_inputs(
        profile.as_ref(),
        ContractOverrides {
            bounds: args.bounds.as_deref(),
            max_flow: args.max_flow,
            speed_range: args.speed_range.as_deref(),
            monotonic_z: args.monotonic_z,
            min_temp: args.min_temp,
            ..ContractOverrides::default()
        },
    );

    // The bytes we will upload: rewritten in memory when --rewrite, else the original file verbatim.
    let rewrite_note;
    let bytes_to_upload: Vec<u8> = if let Some(modearg) = args.rewrite {
        let mode = OptimizeMode::from(modearg);
        let input = fs::File::open(&args.file)
            .unwrap_or_else(|e| die(format!("cannot read {}: {e}", args.file)));
        let imported = import_gcode_reader_with_map(input, &params)
            .unwrap_or_else(|e| die(format!("cannot import {}: {e}", args.file)));
        let emit_params = EmitParams {
            relative_e: true,
            travel_g1_e0: false,
            five_axis: false,
            kinematics: Kinematics::default(),
            flavor: profile
                .as_ref()
                .map(|p| p.emit_params().flavor)
                .unwrap_or(FirmwareFlavor::Marlin),
        };
        let kinematics = profile.as_ref().and_then(|p| p.machine.kinematics.as_ref());
        let mut span_toolpaths = Vec::new();
        for span in imported.motion_spans() {
            let span_tp = Toolpath {
                version: imported.toolpath.version,
                meta: imported.toolpath.meta.clone(),
                segments: imported.toolpath.segments[span.segment_range()].to_vec(),
            };
            span_toolpaths.push(apply_gated(&span_tp, &contracts, mode, kinematics).toolpath);
        }
        let lines = imported
            .emit_source_preserving_spans(&span_toolpaths, &emit_params)
            .unwrap_or_else(|e| die(format!("cannot rewrite {}: {e}", args.file)));
        rewrite_note = format!(" (rewritten --mode {})", optimize_mode_label(mode));
        // Trailing newline mirrors `rewrite-gcode`'s on-disk output (some firmware expects a final \n).
        (lines.join("\n") + "\n").into_bytes()
    } else {
        rewrite_note = String::new();
        fs::read(&args.file).unwrap_or_else(|e| die(format!("cannot read {}: {e}", args.file)))
    };

    // Gate on exactly the bytes we will upload (re-import so findings + source lines match the upload).
    let gate = import_gcode_reader_with_map(Cursor::new(bytes_to_upload.as_slice()), &params)
        .unwrap_or_else(|e| die(format!("cannot import g-code for verification: {e}")));
    let metrics = simulate(&gate.toolpath);
    let report = verify(&gate.toolpath, &contracts);
    let mut review = dry_core::ReviewReport::build(
        Some(args.file.clone()),
        profile_label(profile.as_ref()),
        gate.toolpath.segments.len(),
        metrics,
        &report,
        |segment| gate.source_line_for_segment(segment),
    );
    review.add_unmodeled_gcode(&gate);

    let errors = review.error_count;
    let warnings = review
        .findings
        .iter()
        .filter(|f| f.severity == dry_core::Severity::Warning)
        .count();
    for f in &review.findings {
        let tag = if f.severity == dry_core::Severity::Error {
            "Error"
        } else {
            "Warning"
        };
        let line = f
            .source_line
            .map(|l| format!(" line {l}"))
            .unwrap_or_default();
        eprintln!("  [{tag}] {}{line}: {}", f.rule, f.message);
    }

    let basename = Path::new(&args.file)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("upload.gcode")
        .to_string();
    let rewrite_note = rewrite_note.trim().to_string();
    let print_gate_reason = if args.print && !args.force && profile.is_none() {
        Some("auto-print requires --profile")
    } else if args.print && !args.force && warnings > 0 {
        Some("auto-print blocked because warning findings are present")
    } else {
        None
    };
    if !args.force && (errors > 0 || print_gate_reason.is_some()) {
        if args.json {
            let env = serde_json::json!({
                "gate": "reject",
                "uploaded": false,
                "printed": false,
                "error_count": errors,
                "warning_count": warnings,
                "moonraker_url": args.moonraker,
                "filename": basename,
                "rewrite": rewrite_note,
                "reason": print_gate_reason,
            });
            println!("{}", serde_json::to_string_pretty(&env).unwrap());
        } else {
            let reason = print_gate_reason
                .map(str::to_owned)
                .unwrap_or_else(|| format!("upload blocked by {errors} error finding(s)"));
            eprintln!("error: {reason} (pass --force to override)");
        }
        return std::process::ExitCode::from(1);
    }
    let warn_mode = warnings > 0;

    let cfg = dry_moonraker::MoonrakerConfig {
        base_url: args.moonraker.clone(),
        api_key,
        timeout: Duration::from_secs(args.timeout_s),
    };
    let uploaded = dry_moonraker::upload_file(&cfg, &basename, &bytes_to_upload)
        .unwrap_or_else(|e| die(e.to_string()));

    let mut printed = false;
    if args.print {
        let response = dry_moonraker::start_print(&cfg, &uploaded.filename)
            .unwrap_or_else(|e| die(e.to_string()));
        printed = response.job_started;
    }

    if args.json {
        let gate_verdict = if errors > 0 {
            "reject-forced"
        } else if warn_mode {
            "warn"
        } else {
            "accept"
        };
        let env = serde_json::json!({
            "gate": gate_verdict,
            "uploaded": true,
            "printed": printed,
            "error_count": errors,
            "warning_count": warnings,
            "moonraker_url": args.moonraker,
            "filename": uploaded.filename,
            "rewrite": rewrite_note,
        });
        println!("{}", serde_json::to_string_pretty(&env).unwrap());
    } else {
        eprintln!("upload: {} → {}{}", args.file, args.moonraker, rewrite_note);
        eprintln!(
            "  verify: {} finding(s), {errors} error(s) — uploaded as {}",
            review.findings.len(),
            uploaded.filename
        );
        if printed {
            eprintln!("  printing: started");
        }
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(feature = "llm")]
fn run_compare_llm(args: CompareLlmArgs) -> std::process::ExitCode {
    let model = args.model.unwrap_or_else(|| {
        die("--llm requires --model <id> (e.g. --model claude-sonnet-4-6)".into())
    });
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .unwrap_or_else(|_| die("set ANTHROPIC_API_KEY to use --llm".into()));
    let a = assemble_explain(
        &args.file_a,
        args.profile.as_deref(),
        args.filament_diameter,
        args.line_width,
        args.layer_height,
        None,
        None,
        None,
        false,
        None,
        args.window_s,
    );
    let b = assemble_explain(
        &args.file_b,
        args.profile.as_deref(),
        args.filament_diameter,
        args.line_width,
        args.layer_height,
        None,
        None,
        None,
        false,
        None,
        args.window_s,
    );
    let delta = dry_core::compare_reports(&a.bundle.reports, &b.bundle.reports);
    let cfg = dry_llm::ClientConfig {
        api_key,
        model: model.clone(),
        max_tokens: 4096,
    };
    let narrative = dry_llm::narrate_compare(&cfg, &delta).unwrap_or_else(|e| die(e.to_string()));
    match dry_llm::cost_usd(&model, &narrative.usage) {
        Some(c) => eprintln!(
            "{model} · in {} tok / out {} tok · ~${c:.4}",
            narrative.usage.input_tokens, narrative.usage.output_tokens
        ),
        None => eprintln!(
            "{model} · in {} tok / out {} tok · (pricing unknown for {model})",
            narrative.usage.input_tokens, narrative.usage.output_tokens
        ),
    }
    let rendered = if args.json {
        serde_json::to_string_pretty(&serde_json::json!({
            "delta": delta,
            "narrative": {
                "summary": narrative.summary,
                "what_changed": narrative.what_changed,
                "why_it_matters": narrative.why_it_matters,
                "better": narrative.better,
                "better_rationale": narrative.better_rationale,
            },
            "usage": {
                "input_tokens": narrative.usage.input_tokens,
                "output_tokens": narrative.usage.output_tokens,
            },
            "cost_usd": dry_llm::cost_usd(&model, &narrative.usage),
        }))
        .unwrap()
            + "\n"
    } else {
        let mut md = dry_core::render_compare_markdown(&delta);
        use std::fmt::Write as _;
        let _ = write!(
            md,
            "\n## LLM narrative ({model})\n\n**Summary:** {}\n\n**What changed:** {}\n\n**Why it matters:** {}\n\n**Better:** {} — {}\n",
            narrative.summary,
            narrative.what_changed,
            narrative.why_it_matters,
            narrative.better,
            narrative.better_rationale
        );
        md
    };
    match args.out {
        Some(path) => {
            fs::write(&path, rendered).unwrap_or_else(|e| die(format!("cannot write {path}: {e}")))
        }
        None => print!("{rendered}"),
    }
    std::process::ExitCode::SUCCESS
}

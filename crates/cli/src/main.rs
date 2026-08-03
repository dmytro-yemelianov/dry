//! `dry` — the toolpath compiler CLI. Operates on a Dry IR file (`{version, segments}`, or a fixture
//! wrapping it under an `ir` key). Phase-0 surface: `inspect` / `simulate` / `emit` (`docs/04-tasks.md`).

mod cloud;
mod printer_registry;

use clap::{Parser, Subcommand, ValueEnum};
use dry_core::{
    apply_gated, emit_step_nc, emit_stream_to_writer, import_gcode_reader,
    import_gcode_reader_with_map, import_klipper, optimize_aggressive_pipeline, optimize_pipeline,
    parse_bounds_csv, parse_speed_range_csv, resolve_checked, simulate, simulate_stream,
    trace_summary_with_analytics, trace_summary_with_sources, try_pocket_design, verify,
    verify_stream, BatchFileResult, Contracts, CutMode, EmitParams, FirmwareFlavor,
    GcodeImportParams, Kinematics, KrlFrame, OptimizeMode, PocketOptions, PocketShape, Profile,
    ReviewBatch, RewriteReport, RewriteSpanResult, Toolpath, TraceAnalyticsOptions,
    REFERENCE_FIVE_AXIS_MACHINE,
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

#[derive(Clone, Copy, Debug, ValueEnum)]
enum EmitOutputFormat {
    /// Emit existing FFF-style G-code (Marlin/Klipper/Duet depending on flavor/profile).
    Gcode,
    /// Emit existing CNC/RS-274 output.
    Rs274,
    /// Emit GRBL (laser) output.
    Grbl,
    /// Emit a KUKA KRL module (structure only — never run on a controller; see docs/22-krl-emit.md).
    #[value(alias = "krl")]
    RobotKrl,
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

/// CLI surface for `dry trace-gcode --format`: how the trace is rendered.
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
enum TraceFormatArg {
    Json,
    Csv,
    LayersCsv,
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
enum PrinterCmd {
    /// Search printers by identity, firmware, motion, material, hardware, and macro capabilities.
    Search {
        /// Free-text printer, vendor, model, or variant query.
        query: Option<String>,
        #[arg(long)]
        vendor: Vec<String>,
        #[arg(long)]
        firmware: Vec<String>,
        #[arg(long)]
        kinematics: Vec<String>,
        #[arg(long)]
        material: Vec<String>,
        #[arg(long)]
        nozzle: Option<f64>,
        #[arg(long)]
        build_x: Option<f64>,
        #[arg(long)]
        build_y: Option<f64>,
        #[arg(long)]
        build_z: Option<f64>,
        /// Require a macro definition id, for example `dry:macro/print-start`.
        #[arg(long = "macro")]
        macro_ids: Vec<String>,
        #[arg(long = "hardware-category")]
        hardware_categories: Vec<String>,
        #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u16).range(1..=100))]
        first: u16,
        /// Emit the complete GraphQL result as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Inspect the versioned capabilities and artifacts for one printer.
    Inspect {
        id: String,
        #[arg(long)]
        version: Option<String>,
        /// Emit the complete GraphQL result as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Resolve, hash-verify, and download one runtime dry-profile-v1 artifact.
    Resolve {
        id: String,
        #[arg(long)]
        version: Option<String>,
        #[arg(long)]
        material: Option<String>,
        #[arg(long)]
        nozzle: Option<f64>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(short, long)]
        out: Option<String>,
    },
}

#[derive(Subcommand)]
enum LicenseAction {
    /// Verify and store a license token (argument: token string or a file containing it).
    Activate { token_or_file: String },
    /// Show the active license, its tier and expiry state.
    Status,
}

#[derive(Subcommand)]
enum AuthCmd {
    /// Sign in with Dry Cloud's device authorization flow.
    Login,
    /// Show the current account and monthly usage.
    Status,
    /// Remove the locally stored Dry Cloud token.
    Logout,
}

#[derive(Subcommand)]
enum CloudCmd {
    /// Submit G-code for verification by Dry Cloud and wait for the report.
    Verify {
        /// G-code file to upload.
        file: String,
        /// Printer pack id in the public Dry printer registry.
        #[arg(long)]
        printer: String,
        /// Immutable printer-pack version. When omitted, the cloud resolves the registry default.
        #[arg(long)]
        pack_version: Option<String>,
        /// Print the complete verification report as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum GenerateCmd {
    /// Contour-parallel CNC pocket/profile (rect or circle). Writes resolved Dry IR JSON.
    Pocket {
        /// rect | circle
        #[arg(long, value_parser = ["rect", "circle"])]
        shape: String,
        #[arg(long, allow_hyphen_values = true)]
        x: Option<f64>,
        #[arg(long, allow_hyphen_values = true)]
        y: Option<f64>,
        #[arg(long)]
        width: Option<f64>,
        #[arg(long)]
        height: Option<f64>,
        #[arg(long, allow_hyphen_values = true)]
        cx: Option<f64>,
        #[arg(long, allow_hyphen_values = true)]
        cy: Option<f64>,
        #[arg(long)]
        radius: Option<f64>,
        /// pocket (clear the interior) | profile (single boundary contour)
        #[arg(long, default_value = "pocket", value_parser = ["pocket", "profile"])]
        cut_mode: String,
        #[arg(long)]
        tool_diameter: f64,
        /// Stepover as a fraction of tool diameter in (0, 1]. Rectangular pockets clamp the
        /// resulting inset to ~0.854 of the diameter, the largest value that still clears corners.
        #[arg(long)]
        stepover: Option<f64>,
        #[arg(long)]
        depth: f64,
        #[arg(long)]
        depth_per_pass: Option<f64>,
        #[arg(long, allow_hyphen_values = true)]
        z_top: Option<f64>,
        #[arg(long, allow_hyphen_values = true)]
        safe_z: Option<f64>,
        /// Cutting feed, mm/min.
        #[arg(long)]
        cut_feed: Option<f64>,
        /// Plunge feed, mm/min (default cut_feed / 3).
        #[arg(long)]
        plunge_feed: Option<f64>,
        /// Machine/material profile JSON (supplies ResolveParams defaults).
        #[arg(long)]
        profile: Option<String>,
        /// Write the resolved Dry IR JSON here instead of stdout.
        #[arg(short, long)]
        out: Option<String>,
    },
}

#[derive(Subcommand)]
enum Cmd {
    /// Manage the commercial license (activate a token, show status).
    License {
        #[command(subcommand)]
        action: LicenseAction,
    },
    /// Authenticate with Dry Cloud.
    Auth {
        #[command(subcommand)]
        command: AuthCmd,
        /// Dry Cloud origin. `DRY_CLOUD_URL` takes precedence over the hosted default.
        #[arg(long, global = true)]
        cloud_url: Option<String>,
    },
    /// Run opt-in Dry Cloud operations.
    Cloud {
        #[command(subcommand)]
        command: CloudCmd,
        /// Dry Cloud origin. `DRY_CLOUD_URL` takes precedence over the hosted default.
        #[arg(long, global = true)]
        cloud_url: Option<String>,
    },
    /// Query Dry's hosted printer capability graph.
    Printer {
        #[command(subcommand)]
        command: PrinterCmd,
        /// Registry origin. Can also point at a local or private compatible registry.
        #[arg(
            long,
            global = true,
            default_value = printer_registry::DEFAULT_REGISTRY_URL
        )]
        source: String,
    },
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
        /// Machine/material profile JSON to supply defaults and rotary model.
        #[arg(long)]
        profile: Option<String>,
        /// Emit rotary words from the toolframe orientation (5-axis).
        #[arg(long)]
        five_axis: bool,
        /// Emit RS-274 / GRBL / KRL output instead of the default FFF G-code target.
        #[arg(long, default_value = "gcode", value_enum)]
        format: EmitOutputFormat,
        /// Rotary axes (ab/ac/bc) that carry the toolframe orientation for 5-axis words. (Accepts the
        /// legacy `--kinematics` alias; this is the rotary-axes STRING, not the motion-limits object.)
        #[arg(long, visible_alias = "kinematics", value_enum)]
        rotary_axes: Option<RotaryAxesArg>,
        /// Also write a STEP-NC intent file with the same program to this path.
        #[arg(long)]
        step_nc: Option<String>,
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
    /// Generate a parametric design and write its resolved Dry IR.
    Generate {
        #[command(subcommand)]
        what: GenerateCmd,
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
        /// Also compute layer linkage and higher-level statistics (`trace.layers`, `trace.analytics`).
        #[arg(long)]
        analytics: bool,
        /// Multiple of the window-peak p50 above which a window is flagged. Requires `--analytics`.
        #[arg(long)]
        flow_outlier_k: Option<f64>,
        /// Output shape: the full JSON report, the per-window CSV, or the per-layer CSV.
        /// `layers-csv` implies `--analytics`, since the analytics pass is the only producer of rows.
        #[arg(long, value_enum, default_value_t = TraceFormatArg::Json)]
        format: TraceFormatArg,
    },
    /// Review a batch of slicer G-code files, emitting a per-file + aggregate `ReviewBatch`.
    ///
    /// Unlike `review-gcode`, an unreadable/unimportable file does not abort the run — it becomes an
    /// `errored` result and every other file is still reviewed. Exit `0` if every file passed, `1` if
    /// every file was inspected and at least one has an error-severity finding, `2` if at least one
    /// file could not be inspected at all (or on a usage error) — `2` outranks `1`.
    ReviewBatch {
        /// G-code files to review, in order.
        files: Vec<String>,
        /// Also read newline-separated paths from this file (`-` for stdin), appended after `files`.
        #[arg(long)]
        files_from: Option<String>,
        /// Machine/material profile JSON to supply import defaults and verifier contracts, shared by
        /// every file in the batch.
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
        /// Print the `ReviewBatch` envelope as JSON.
        #[arg(long)]
        json: bool,
        /// Write the output to a file instead of stdout.
        #[arg(long)]
        out: Option<String>,
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

/// Production license-signing keys, installed by the release key ceremony. `prod-1` was
/// generated 2026-08-03 (Ed25519, WebCrypto keygen; verifying key below, signing key held
/// offline by the owner — never in this repo). Rotation: add a new key id here, keep the old
/// entry so already-issued licenses keep verifying.
const PRODUCTION_KEYS: &[(&str, [u8; 32])] = &[(
    "prod-1",
    [
        0x4c, 0x0b, 0x77, 0xdc, 0x2f, 0x2d, 0xb6, 0x9f, 0xc5, 0xdf, 0xb5, 0xef, 0xf8, 0x41, 0x60,
        0x76, 0xfd, 0x5c, 0xd0, 0xfa, 0x69, 0x3b, 0x24, 0x3a, 0x31, 0x59, 0x66, 0x03, 0x5f, 0x37,
        0x7b, 0xcd,
    ],
)];

/// The verification keys `resolve_license`/`activate` accept: production keys always, plus —
/// in debug builds only, and only with the explicit `DRY_LICENSE_ALLOW_TEST_KEY=1` opt-in — the
/// committed TEST key.
///
/// Decision: release binaries trust only `PRODUCTION_KEYS`, full stop, regardless of any env
/// var. The committed test key (and its fixture tokens under `crates/license/tests/fixtures/`)
/// only ever verifies in a debug build with the explicit opt-in; there is no way to make a
/// release binary accept it. Testing licensing behavior against a release binary requires a
/// real key from the release key ceremony (see the release runbook) — not this fixture.
fn license_keys() -> Vec<(&'static str, [u8; 32])> {
    let mut keys: Vec<(&'static str, [u8; 32])> = PRODUCTION_KEYS.to_vec();
    let allow_test = cfg!(debug_assertions)
        && std::env::var("DRY_LICENSE_ALLOW_TEST_KEY").is_ok_and(|v| v == "1");
    if allow_test {
        keys.push((dry_license::TEST_KEY_ID, dry_license::TEST_VERIFYING_KEY));
    }
    keys
}

/// Where an activated license token is stored. `XDG_CONFIG_HOME` is honored first — this is
/// also what makes CLI tests hermetic on macOS, where `dirs::config_dir()` ignores that
/// variable — falling back to the platform config dir, then a temp dir as a last resort.
fn license_config_path() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(dirs::config_dir)
        .unwrap_or_else(std::env::temp_dir);
    base.join("dry").join("license.token")
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The result of resolving a license from the environment/config file: either a verified
/// license (possibly in its grace period) or evaluation mode, optionally with a reason.
enum LicenseResolution {
    Licensed(dry_license::VerifiedLicense),
    Eval { warning: Option<String> },
}

/// Resolve the active license: `DRY_LICENSE` env var first, then the stored token file at
/// [`license_config_path`]. Never exits — any parse/signature/expiry problem falls back to
/// evaluation mode with an explanatory warning, per the spec's "never a hard exit from a
/// report command" rule.
fn resolve_license() -> LicenseResolution {
    let token = match std::env::var("DRY_LICENSE") {
        Ok(t) if !t.trim().is_empty() => Some(t),
        _ => std::fs::read_to_string(license_config_path()).ok(),
    };
    let Some(token) = token else {
        return LicenseResolution::Eval { warning: None };
    };
    let keys = license_keys();
    match dry_license::verify_token(&token, &keys, now_unix()) {
        Ok(v) => match v.state {
            dry_license::LicenseState::Expired => LicenseResolution::Eval {
                warning: Some(format!(
                    "license for {} expired {} (past the 14-day grace) — running in evaluation mode",
                    v.payload.licensee, v.payload.expires
                )),
            },
            _ => LicenseResolution::Licensed(v),
        },
        Err(e) => LicenseResolution::Eval {
            warning: Some(format!("{e} — running in evaluation mode")),
        },
    }
}

/// The eval-mode notice every report-producing command prints once to stderr when running
/// without a license. Exact text per the license product spec — `tests/license.rs` pins it.
const EVAL_BANNER: &str =
    "EVALUATION — not for production gating. https://dry-public-docs.pages.dev/pricing";

/// Map a resolved license to the passive stamp embedded in report envelopes (`dry_core::LicenseStamp`,
/// Task 4). Never fails: eval mode stamps `mode: "evaluation"` with no licensee/tier.
fn license_stamp(res: &LicenseResolution) -> dry_core::LicenseStamp {
    match res {
        LicenseResolution::Licensed(v) => dry_core::LicenseStamp {
            mode: "licensed".to_string(),
            licensee: Some(v.payload.licensee.clone()),
            tier: Some(v.payload.tier.to_string()),
        },
        LicenseResolution::Eval { .. } => dry_core::LicenseStamp {
            mode: "evaluation".to_string(),
            licensee: None,
            tier: None,
        },
    }
}

/// Emit the once-per-run stderr notice for a report-producing command: in evaluation mode, any
/// specific reason `resolve_license` recorded (expired past grace, bad signature, unknown key
/// id, malformed token) followed by the eval banner; absent that, just the eval banner. Also
/// prints a grace-period warning when the active license is past its expiry but still inside the
/// 14-day grace window. Prints nothing for a comfortably valid license.
fn license_notice(res: &LicenseResolution) {
    match res {
        LicenseResolution::Eval { warning } => {
            if let Some(w) = warning {
                eprintln!("warning: {w}");
            }
            eprintln!("{EVAL_BANNER}");
        }
        LicenseResolution::Licensed(v) => {
            if let dry_license::LicenseState::Grace { days_left } = v.state {
                eprintln!(
                    "warning: license for {} is in its grace period ({days_left} day(s) left) — \
                     see https://dry-public-docs.pages.dev/pricing to renew",
                    v.payload.licensee
                );
            }
        }
    }
}

fn run_license(action: LicenseAction) -> ExitCode {
    match action {
        LicenseAction::Activate { token_or_file } => {
            let token = match std::fs::read_to_string(&token_or_file) {
                Ok(contents) => contents,
                Err(_) => token_or_file,
            };
            let token = token.trim();
            let keys = license_keys();
            let verified = dry_license::verify_token(token, &keys, now_unix())
                .unwrap_or_else(|e| die(e.to_string()));
            let path = license_config_path();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .unwrap_or_else(|e| die(format!("cannot create {}: {e}", parent.display())));
            }
            fs::write(&path, token)
                .unwrap_or_else(|e| die(format!("cannot write {}: {e}", path.display())));
            println!(
                "activated: {} ({}) expires {}",
                verified.payload.licensee, verified.payload.tier, verified.payload.expires
            );
            ExitCode::SUCCESS
        }
        LicenseAction::Status => {
            match resolve_license() {
                LicenseResolution::Licensed(v) => {
                    println!("licensee:  {}", v.payload.licensee);
                    println!("tier:      {}", v.payload.tier);
                    println!("machines:  {}", v.payload.machines);
                    println!("expires:   {}", v.payload.expires);
                    match v.state {
                        dry_license::LicenseState::Grace { days_left } => {
                            eprintln!(
                                "warning: license is in its grace period ({days_left} day(s) left)"
                            );
                            println!("state:     grace ({days_left} day(s) left)");
                        }
                        dry_license::LicenseState::Valid => println!("state:     valid"),
                        dry_license::LicenseState::Expired => unreachable!(
                            "resolve_license() maps Expired to LicenseResolution::Eval"
                        ),
                    }
                }
                LicenseResolution::Eval { warning } => {
                    if let Some(w) = &warning {
                        eprintln!("warning: {w}");
                    }
                    println!("mode:      evaluation");
                    println!("           see https://dry-public-docs.pages.dev/pricing to purchase a license");
                }
            }
            ExitCode::SUCCESS
        }
    }
}

/// Remove a leftover `.dry-partial` temp file. Never fails the command over it — the program's own
/// success/failure was already decided — but a surviving temp file is worth a warning rather than
/// silent disposal, since e.g. a permissions problem removing it would otherwise vanish unremarked.
fn cleanup_tmp(tmp: &str) {
    if let Err(e) = fs::remove_file(tmp) {
        if e.kind() != std::io::ErrorKind::NotFound {
            eprintln!("warning: could not remove leftover temp file {tmp}: {e}");
        }
    }
}

/// Write `path`'s content to a sibling `{path}.dry-partial` via `write`, flushing before returning.
/// Does not rename into place — see [`commit_atomic`] — so a caller can stage several files before
/// any of them touch their final path. On failure the temp file is cleaned up and never left behind.
fn stage_atomic(
    path: &str,
    write: impl FnOnce(&mut std::io::BufWriter<fs::File>) -> Result<(), String>,
) -> Result<String, String> {
    let tmp = format!("{path}.dry-partial");
    let file = fs::File::create(&tmp).map_err(|e| format!("cannot write {path}: {e}"))?;
    let mut writer = std::io::BufWriter::new(file);
    let result = write(&mut writer).and_then(|()| {
        writer
            .flush()
            .map_err(|e| format!("cannot write {path}: {e}"))
    });
    drop(writer);
    match result {
        Ok(()) => Ok(tmp),
        Err(msg) => {
            cleanup_tmp(&tmp);
            Err(msg)
        }
    }
}

/// Rename a temp file staged by [`stage_atomic`] into place at `path`.
fn commit_atomic(tmp: &str, path: &str) -> Result<(), String> {
    fs::rename(tmp, path).map_err(|e| format!("cannot write {path}: {e}"))
}

/// Stream a program to `path` through a sibling temporary file, renaming it into place only once
/// the whole program is on disk.
///
/// `emit_stream_to_writer` writes lines as it produces them and refuses on *segment content* — a
/// non-finite word, an endpointless arc — which it can only discover mid-program, and [`die`] exits
/// on the spot. Streaming straight into the destination therefore leaves a truncated but
/// syntactically valid g-code file exactly where the caller asked for a program; under RS-274 that
/// prefix has also lost its `M9`/`M5`/`M30` postamble. Nothing appears at `path` unless the whole
/// program emitted.
fn write_program(
    path: &str,
    emit: impl FnOnce(&mut std::io::BufWriter<fs::File>) -> Result<(), String>,
) {
    let tmp = match stage_atomic(path, |writer| {
        emit(writer)
            .and_then(|()| writeln!(writer).map_err(|e| format!("cannot write {path}: {e}")))
    }) {
        Ok(tmp) => tmp,
        Err(msg) => die(msg),
    };
    if let Err(msg) = commit_atomic(&tmp, path) {
        cleanup_tmp(&tmp);
        die(msg);
    }
}

/// Unwrap a shape-dependent optional flag or exit with a clap-style missing-argument error.
fn require(value: Option<f64>, flag: &str) -> f64 {
    value.unwrap_or_else(|| die(format!("{flag} is required for the selected --shape")))
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
    let license = resolve_license();
    match cli.cmd {
        Cmd::License { action } => run_license(action),
        Cmd::Auth { command, cloud_url } => {
            let cloud_url = cloud::resolve_cloud_url(cloud_url.as_deref());
            match command {
                AuthCmd::Login => {
                    cloud::login(&cloud_url).unwrap_or_else(|error| die(error.to_string()))
                }
                AuthCmd::Status => {
                    cloud::status(&cloud_url).unwrap_or_else(|error| die(error.to_string()))
                }
                AuthCmd::Logout => cloud::logout().unwrap_or_else(|error| die(error.to_string())),
            }
            ExitCode::SUCCESS
        }
        Cmd::Cloud { command, cloud_url } => {
            let cloud_url = cloud::resolve_cloud_url(cloud_url.as_deref());
            match command {
                CloudCmd::Verify {
                    file,
                    printer,
                    pack_version,
                    json,
                } => cloud::verify(&cloud_url, &file, &printer, pack_version.as_deref(), json)
                    .unwrap_or_else(|error| die(error.to_string())),
            }
        }
        Cmd::Printer { command, source } => run_printer(command, &source),
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
            profile,
            five_axis,
            format,
            rotary_axes,
            step_nc,
            out,
        } => {
            let stream =
                load_streaming(&file).unwrap_or_else(|e| die(format!("cannot stream {file}: {e}")));
            let profile = load_profile(profile.as_deref());
            let kinematics = rotary_axes
                .map(Into::into)
                .or_else(|| profile.as_ref().and_then(|p| p.machine.five_axis))
                .unwrap_or(REFERENCE_FIVE_AXIS_MACHINE);
            let mut flavor = profile
                .as_ref()
                .map(|p| p.emit_params().flavor)
                .unwrap_or(FirmwareFlavor::Marlin);
            match format {
                EmitOutputFormat::Rs274 => flavor = FirmwareFlavor::Rs274,
                EmitOutputFormat::Grbl => flavor = FirmwareFlavor::Grbl,
                EmitOutputFormat::RobotKrl => flavor = FirmwareFlavor::RobotKrl,
                EmitOutputFormat::Gcode => {}
            }
            let params = EmitParams {
                relative_e: !absolute_e,
                travel_g1_e0: false,
                five_axis,
                kinematics,
                flavor,
                cnc_frame: profile.as_ref().and_then(|p| p.emit_params().cnc_frame),
                // Not wired from `profile`: the profile schema has no KRL block yet, so the
                // program name and $TOOL/$BASE stay at the emitter's documented defaults
                // (see crates/core/src/emit/krl.rs).
                krl_frame: KrlFrame::default(),
            };
            if let Some(step_nc_path) = step_nc {
                let segments = stream
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap_or_else(|e| die(format!("cannot emit {file}: {e}")));
                let toolpath = Toolpath {
                    version: 0,
                    meta: None,
                    segments: segments.clone(),
                };
                // Render the sidecar before emitting, so a toolpath STEP-NC cannot represent is
                // refused before anything is written — but stage it (to a temp path, not the real
                // one) *before* the g-code emits, and commit both only at the end. A temp file at a
                // temp path is not a machining program, so this does not reintroduce the hazard the
                // ordering was chosen to avoid: the `.stpnc` still cannot appear at its real path
                // before the g-code is known to be emittable, and disk-full on the sidecar is now
                // caught before the g-code lands instead of after.
                let step_nc_text = emit_step_nc(&toolpath, &params)
                    .unwrap_or_else(|e| die(format!("cannot emit {step_nc_path}: {e}")));
                let step_nc_tmp = stage_atomic(&step_nc_path, |writer| {
                    writer
                        .write_all(step_nc_text.as_bytes())
                        .map_err(|e| format!("cannot write {step_nc_path}: {e}"))
                })
                .unwrap_or_else(|e| die(e));
                match out {
                    Some(path) => {
                        let gcode_tmp = stage_atomic(&path, |writer| {
                            emit_stream_to_writer(segments.into_iter().map(Ok), &params, writer)
                                .map_err(|e| format!("cannot emit {file}: {e}"))
                                .and_then(|()| {
                                    writeln!(writer)
                                        .map_err(|e| format!("cannot write {path}: {e}"))
                                })
                        })
                        .unwrap_or_else(|e| {
                            cleanup_tmp(&step_nc_tmp);
                            die(e)
                        });
                        if let Err(msg) = commit_atomic(&gcode_tmp, &path) {
                            cleanup_tmp(&gcode_tmp);
                            cleanup_tmp(&step_nc_tmp);
                            die(msg);
                        }
                        // The g-code is now the only thing on disk that must survive: if the
                        // sidecar's rename fails from here, `path` already holds a complete,
                        // usable program, so exit 2 no longer means "nothing usable was written".
                        if let Err(msg) = commit_atomic(&step_nc_tmp, &step_nc_path) {
                            cleanup_tmp(&step_nc_tmp);
                            die(format!(
                                "{msg} (a complete g-code program was already written to {path})"
                            ));
                        }
                    }
                    None => {
                        let stdout = std::io::stdout();
                        let mut writer = stdout.lock();
                        if let Err(e) = emit_stream_to_writer(
                            segments.into_iter().map(Ok),
                            &params,
                            &mut writer,
                        ) {
                            cleanup_tmp(&step_nc_tmp);
                            die(format!("cannot emit {file}: {e}"));
                        }
                        if let Err(e) = writeln!(writer) {
                            cleanup_tmp(&step_nc_tmp);
                            die(format!("cannot write stdout: {e}"));
                        }
                        if let Err(msg) = commit_atomic(&step_nc_tmp, &step_nc_path) {
                            cleanup_tmp(&step_nc_tmp);
                            die(format!(
                                "{msg} (the g-code program was already printed to stdout)"
                            ));
                        }
                    }
                }
            } else {
                match out {
                    Some(path) => write_program(&path, |writer| {
                        emit_stream_to_writer(stream, &params, writer)
                            .map_err(|e| format!("cannot emit {file}: {e}"))
                    }),
                    None => {
                        let stdout = std::io::stdout();
                        let mut writer = stdout.lock();
                        emit_stream_to_writer(stream, &params, &mut writer)
                            .unwrap_or_else(|e| die(format!("cannot emit {file}: {e}")));
                        writeln!(writer)
                            .unwrap_or_else(|e| die(format!("cannot write stdout: {e}")));
                    }
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
        Cmd::Generate {
            what:
                GenerateCmd::Pocket {
                    shape,
                    x,
                    y,
                    width,
                    height,
                    cx,
                    cy,
                    radius,
                    cut_mode,
                    tool_diameter,
                    stepover,
                    depth,
                    depth_per_pass,
                    z_top,
                    safe_z,
                    cut_feed,
                    plunge_feed,
                    profile,
                    out,
                },
        } => {
            let shape = match shape.as_str() {
                "rect" => PocketShape::Rect {
                    x: require(x, "--x"),
                    y: require(y, "--y"),
                    width: require(width, "--width"),
                    height: require(height, "--height"),
                },
                _ => PocketShape::Circle {
                    cx: require(cx, "--cx"),
                    cy: require(cy, "--cy"),
                    radius: require(radius, "--radius"),
                },
            };
            let options = PocketOptions {
                shape,
                mode: if cut_mode == "profile" {
                    CutMode::Profile
                } else {
                    CutMode::Pocket
                },
                tool_diameter,
                stepover,
                depth,
                depth_per_pass,
                z_top,
                safe_z,
                cut_feed,
                plunge_feed,
            };
            let design = try_pocket_design(&options)
                .unwrap_or_else(|e| die(format!("cannot generate pocket: {e}")));
            let params = load_profile(profile.as_deref())
                .map(|p| p.resolve_params())
                .unwrap_or_default();
            let toolpath = resolve_checked(&design, &params)
                .unwrap_or_else(|e| die(format!("cannot resolve pocket design: {e}")));
            let json = toolpath.to_json();
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
            review.license = Some(license_stamp(&license));
            license_notice(&license);

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
            analytics,
            flow_outlier_k,
            format,
        } => {
            // `--format layers-csv` is the only producer of layer rows, so it implies `--analytics`;
            // supplying `--flow-outlier-k` without either is a usage error rather than a silently
            // ignored flag.
            let analytics_requested = analytics || format == TraceFormatArg::LayersCsv;
            if flow_outlier_k.is_some() && !analytics_requested {
                die("--flow-outlier-k requires --analytics (or --format layers-csv)".to_string());
            }
            // Plain `--format csv` renders windows only — no layer rows, no analytics block — so
            // `--analytics` beside it would compute a whole statistical pass and discard it. Skipping
            // it is what keeps the two invocations byte-identical *and* equally cheap; `layers-csv`
            // and `json` both show something the pass produced, so they keep it.
            let compute_analytics = analytics_requested && format != TraceFormatArg::Csv;

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
            let trace = if compute_analytics {
                let mut options = TraceAnalyticsOptions::default();
                if let Some(k) = flow_outlier_k {
                    options.flow_outlier_k = k;
                }
                trace_summary_with_analytics(&imported.toolpath, window_s, &source_lines, &options)
                    .unwrap_or_else(|e| die(format!("cannot trace {file}: {e}")))
            } else {
                trace_summary_with_sources(&imported.toolpath, window_s, &source_lines)
                    .unwrap_or_else(|e| die(format!("cannot trace {file}: {e}")))
            };

            match format {
                TraceFormatArg::Csv => print!("{}", trace.to_csv()),
                TraceFormatArg::LayersCsv => print!("{}", trace.layers_to_csv()),
                TraceFormatArg::Json => {
                    let report = dry_core::TraceReport {
                        file: Some(file.clone()),
                        profile: profile_label(profile.as_ref()),
                        trace,
                    };
                    println!("{}", serde_json::to_string_pretty(&report).unwrap());
                }
            }
            ExitCode::SUCCESS
        }
        Cmd::ReviewBatch {
            files,
            files_from,
            profile,
            filament_diameter,
            line_width,
            layer_height,
            json,
            out,
        } => {
            let mut paths = files;
            if let Some(files_from) = files_from {
                let text = if files_from == "-" {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
                        .unwrap_or_else(|e| die(format!("cannot read stdin: {e}")));
                    buf
                } else {
                    fs::read_to_string(&files_from)
                        .unwrap_or_else(|e| die(format!("cannot read {files_from}: {e}")))
                };
                paths.extend(
                    text.lines()
                        .map(str::trim)
                        .filter(|l| !l.is_empty())
                        .map(str::to_string),
                );
            }
            if paths.is_empty() {
                die("review-batch: no files given (pass paths, or --files-from)".to_string());
            }

            let profile = load_profile(profile.as_deref());
            let params = gcode_review_params(
                profile.as_ref(),
                filament_diameter,
                line_width,
                layer_height,
            );
            let profile_label = profile_label(profile.as_ref());

            let mut results = Vec::with_capacity(paths.len());
            for path in &paths {
                let result = (|| -> Result<BatchFileResult, String> {
                    let input =
                        fs::File::open(path).map_err(|e| format!("cannot read {path}: {e}"))?;
                    let imported = import_gcode_reader_with_map(input, &params)
                        .map_err(|e| format!("cannot import {path}: {e}"))?;
                    let contracts =
                        contracts_from_inputs(profile.as_ref(), ContractOverrides::default());
                    let metrics = simulate(&imported.toolpath);
                    let verify_report = verify(&imported.toolpath, &contracts);
                    let mut review = dry_core::ReviewReport::build(
                        Some(path.clone()),
                        profile_label.clone(),
                        imported.toolpath.segments.len(),
                        metrics,
                        &verify_report,
                        |segment| imported.source_line_for_segment(segment),
                    );
                    review.add_unmodeled_gcode(&imported);
                    Ok(BatchFileResult::inspected(path.clone(), review))
                })()
                .unwrap_or_else(|e| BatchFileResult::errored(path.clone(), e));
                results.push(result);
            }

            let any_errored = results
                .iter()
                .any(|r| r.status == dry_core::BatchStatus::Errored);
            let mut batch = ReviewBatch::build(profile_label, results);
            batch.license = Some(license_stamp(&license));
            license_notice(&license);

            let rendered = if json {
                serde_json::to_string_pretty(&batch).unwrap()
            } else {
                render_batch_human(&batch)
            };
            match &out {
                Some(path) => fs::write(path, rendered + "\n")
                    .unwrap_or_else(|e| die(format!("cannot write {path}: {e}"))),
                None => println!("{rendered}"),
            }

            if any_errored {
                ExitCode::from(2)
            } else if batch.files_failed > 0 {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
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
            let mut a = assemble_explain(
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
            a.bundle.license = Some(license_stamp(&license));
            license_notice(&license);
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
            let mut delta = dry_core::compare_reports(&a.bundle.reports, &b.bundle.reports);
            delta.license = Some(license_stamp(&license));
            license_notice(&license);
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
            // cnc_frame is intentionally left None (not wired from `profile`) here: this path calls
            // `emit_source_preserving_spans`, which emits each span in isolation to line up with a
            // separately-emitted flattened reference (see `emit_normalized_span_lines` in
            // dry-core's gcode/lift.rs) to recover per-span line offsets. If cnc_frame were set,
            // every span — not just the first — would grow its own copy of the G21/G54/T../S../M3
            // preamble, which would either desync that line-count accounting (corrupting the
            // splice) or, if accounting happened to survive, duplicate the frame once per span and
            // strand `emit`'s M30 postamble mid-file instead of at the true end. Wiring this
            // correctly needs a design decision in dry-core (e.g. an internal frame-suppression
            // knob for non-leading spans), tracked as follow-up, not blindly copied from the `Emit`
            // arm's fix.
            let emit_params = EmitParams {
                relative_e: !absolute_e,
                travel_g1_e0: false,
                five_axis: false,
                kinematics: Kinematics::default(),
                flavor: profile
                    .as_ref()
                    .map(|p| p.emit_params().flavor)
                    .unwrap_or(FirmwareFlavor::Marlin),
                cnc_frame: None,
                // Unused on this path: `emit_source_preserving_spans` refuses a KRL flavor
                // outright, because a DEF/END module is not a spliceable motion span.
                krl_frame: KrlFrame::default(),
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
                let mut report = RewriteReport::build(
                    Some(file.clone()),
                    profile_label(profile.as_ref()),
                    mode_label.to_string(),
                    &before_tp,
                    &after_tp,
                    span_results,
                );
                report.license = Some(license_stamp(&license));
                license_notice(&license);

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
            // Signed, because `--reorder-travel` can *grow* the segment count: `z_hop` replaces one
            // travel with a lift/traverse/drop triple and `coasting` splits the tail off a run. The
            // `usize` subtraction this replaces panicked with "attempt to subtract with overflow" on
            // 20 of the 28 frozen gallery designs (and wrapped to ~1.8e19 in release).
            let delta = after as i64 - before as i64;
            eprintln!(
                "optimize: {file} — {before} → {after} segments ({delta:+}); \
                 travel {travel_before:.2}mm → {travel_after:.2}mm; \
                 volume {:.4}mm^3 (Δ{:.2e}), time {:.3}s (Δ{:.2e})",
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
        } => run_upload(
            UploadArgs {
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
            },
            &license,
        ),
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
            let mut report = verify_stream(stream, &contracts)
                .unwrap_or_else(|e| die(format!("cannot verify {file}: {e}")));
            report.license = Some(license_stamp(&license));
            license_notice(&license);
            if json {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else if report.findings.is_empty() {
                // Say what the pass covered, not just that it found nothing: "OK" over zero segments,
                // or under the structural rules alone, is a much weaker statement than "OK" under a
                // full machine profile, and the two used to print identically.
                println!(
                    "verify: {file} — OK (no findings; {} segment(s) inspected, {} rule(s) in force)",
                    report.segments_inspected,
                    report.rules_evaluated.len()
                );
            } else {
                for f in &report.findings {
                    let seg = f.segment.map(|i| format!(" seg {i}")).unwrap_or_default();
                    println!("  [{:?}] {}{seg}: {}", f.severity, f.rule, f.message);
                }
                println!(
                    "verify: {file} — {} finding(s), {} error(s); {} segment(s) inspected, {} rule(s) in force",
                    report.findings.len(),
                    report.error_count(),
                    report.segments_inspected,
                    report.rules_evaluated.len()
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

fn run_printer(command: PrinterCmd, source: &str) -> ExitCode {
    match command {
        PrinterCmd::Search {
            query,
            vendor,
            firmware,
            kinematics,
            material,
            nozzle,
            build_x,
            build_y,
            build_z,
            macro_ids,
            hardware_categories,
            first,
            json,
        } => {
            let connection = printer_registry::search(
                source,
                printer_registry::SearchFilter {
                    text: query,
                    vendor,
                    firmware,
                    kinematics,
                    material,
                    nozzle_diameter_mm: nozzle,
                    build_x_mm: build_x,
                    build_y_mm: build_y,
                    build_z_mm: build_z,
                    macro_ids,
                    hardware_categories,
                },
                first.into(),
            )
            .unwrap_or_else(|error| die(format!("printer search failed: {error}")));
            if json {
                println!("{}", serde_json::to_string_pretty(&connection).unwrap());
                return ExitCode::SUCCESS;
            }
            let total = connection
                .get("totalCount")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            println!("{total} matching printer(s)");
            for printer in connection
                .get("nodes")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
            {
                let id = printer
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?");
                let name = printer
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(id);
                let version = first_version(printer);
                let version_label = version
                    .and_then(|value| value.get("version"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?");
                println!("{id}  {name}  v{version_label}");
                if let Some(capabilities) = version.and_then(|value| value.get("capabilities")) {
                    let firmware = strings_at(capabilities, &["firmware"], "flavor");
                    let machine = capabilities.get("machine");
                    let kinematics = machine
                        .and_then(|value| value.get("kinematics"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?");
                    let volume = machine
                        .and_then(|value| value.get("buildVolume"))
                        .map(format_build_volume)
                        .unwrap_or_else(|| "?×?×? mm".into());
                    let materials = strings_at(capabilities, &["materials"], "family");
                    println!(
                        "  {} | {} | {} | {}",
                        if firmware.is_empty() {
                            "?".into()
                        } else {
                            firmware.join(", ")
                        },
                        kinematics,
                        volume,
                        if materials.is_empty() {
                            "materials ?".into()
                        } else {
                            materials.join(", ")
                        }
                    );
                }
            }
            ExitCode::SUCCESS
        }
        PrinterCmd::Inspect { id, version, json } => {
            let printer = printer_registry::inspect(source, &id, version.as_deref())
                .unwrap_or_else(|error| die(format!("printer inspect failed: {error}")))
                .unwrap_or_else(|| die(format!("printer not found: {id}")));
            if json {
                println!("{}", serde_json::to_string_pretty(&printer).unwrap());
                return ExitCode::SUCCESS;
            }
            let name = printer
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(&id);
            println!("{name} ({id})");
            println!(
                "  kind:      {}",
                printer
                    .get("kind")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?")
            );
            for version in printer
                .get("versions")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
            {
                println!(
                    "  version:   {} [{} / {}]",
                    version
                        .get("version")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?"),
                    version
                        .get("trustLevel")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?"),
                    version
                        .get("supportStatus")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?"),
                );
                if let Some(capabilities) = version.get("capabilities") {
                    let firmware = strings_at(capabilities, &["firmware"], "flavor");
                    let machine = capabilities.get("machine");
                    println!(
                        "  machine:   {} | {}",
                        machine
                            .and_then(|value| value.get("kinematics"))
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("?"),
                        machine
                            .and_then(|value| value.get("buildVolume"))
                            .map(format_build_volume)
                            .unwrap_or_else(|| "?×?×? mm".into())
                    );
                    println!("  firmware:  {}", firmware.join(", "));
                    println!(
                        "  graph:     {} hardware, {} materials, {} macros, {} profiles",
                        array_len(capabilities, "hardware"),
                        array_len(capabilities, "materials"),
                        array_len(capabilities, "macroBindings"),
                        array_len(version, "profiles"),
                    );
                }
                if let Some(url) = version.get("packUrl").and_then(serde_json::Value::as_str) {
                    println!("  pack:      {url}");
                }
            }
            ExitCode::SUCCESS
        }
        PrinterCmd::Resolve {
            id,
            version,
            material,
            nozzle,
            profile,
            out,
        } => {
            let resolved = printer_registry::resolve_profile(
                source,
                &id,
                &printer_registry::ProfileSelector {
                    version,
                    material_id: material,
                    nozzle_diameter_mm: nozzle,
                    profile_id: profile,
                },
            )
            .unwrap_or_else(|error| die(format!("profile resolution failed: {error}")))
            .unwrap_or_else(|| die(format!("no matching profile for printer {id}")));
            let bytes = printer_registry::download_profile(
                &resolved,
                out.as_deref().map(std::path::Path::new),
            )
            .unwrap_or_else(|error| die(format!("profile download failed: {error}")));
            if let Some(path) = out {
                eprintln!(
                    "resolved {} → {path} ({} bytes, SHA-256 verified)",
                    resolved
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("profile"),
                    bytes.len()
                );
            } else {
                std::io::stdout()
                    .write_all(&bytes)
                    .unwrap_or_else(|error| die(format!("cannot write stdout: {error}")));
                if !bytes.ends_with(b"\n") {
                    println!();
                }
            }
            ExitCode::SUCCESS
        }
    }
}

fn first_version(printer: &serde_json::Value) -> Option<&serde_json::Value> {
    printer
        .get("versions")
        .and_then(serde_json::Value::as_array)
        .and_then(|versions| versions.first())
}

fn array_len(value: &serde_json::Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn strings_at(value: &serde_json::Value, path: &[&str], field: &str) -> Vec<String> {
    let mut current = value;
    for key in path {
        let Some(next) = current.get(key) else {
            return Vec::new();
        };
        current = next;
    }
    current
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.get(field).and_then(serde_json::Value::as_str))
        .map(str::to_owned)
        .collect()
}

fn format_build_volume(volume: &serde_json::Value) -> String {
    let axis = |name: &str| {
        volume
            .get(name)
            .and_then(|value| value.get("sizeMm"))
            .and_then(serde_json::Value::as_f64)
            .map(|value| {
                if value.fract() == 0.0 {
                    format!("{value:.0}")
                } else {
                    value.to_string()
                }
            })
            .unwrap_or_else(|| "?".into())
    };
    format!("{}×{}×{} mm", axis("x"), axis("y"), axis("z"))
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

/// Human-readable `review-batch` output, rendered **once** from the finished envelope: one line per
/// file in input order, then the aggregate footer — `dry_core::BatchStatus`/`ReviewBatch` carry the
/// same numbers `--json` does. Not streamed: nothing is printed until every file has been reviewed, so
/// a long batch shows no progress. Per-file progress would mean printing from the review loop instead,
/// which is a change to the loop rather than to this function.
fn render_batch_human(batch: &dry_core::ReviewBatch) -> String {
    use dry_core::BatchStatus;

    let mut out = String::new();
    out.push_str(&format!("review-batch: {} file(s)", batch.files_total));
    if let Some(label) = &batch.profile {
        out.push_str(&format!(", profile {label}"));
    }
    out.push('\n');

    for result in &batch.results {
        match &result.status {
            BatchStatus::Errored => {
                let error = result.error.as_deref().unwrap_or("unknown error");
                out.push_str(&format!("  ERROR  {:<12} {error}\n", result.file));
            }
            status => {
                let review = result
                    .review
                    .as_ref()
                    .expect("passed/failed results carry a review");
                let tag = if *status == BatchStatus::Passed {
                    "PASS"
                } else {
                    "FAIL"
                };
                let warnings = review.findings.len() - review.error_count;
                let detail = if review.findings.is_empty() {
                    "no findings".to_string()
                } else {
                    format!("{} error(s), {warnings} warning(s)", review.error_count)
                };
                out.push_str(&format!(
                    "  {tag:<6} {:<12} {} segments, {detail}\n",
                    result.file, review.segments
                ));
            }
        }
    }

    out.push_str("  --\n");
    out.push_str(&format!(
        "  {} file(s): {} passed, {} failed, {} errored\n",
        batch.files_total, batch.files_passed, batch.files_failed, batch.files_errored
    ));
    if !batch.findings_by_rule.is_empty() {
        let parts: Vec<String> = batch
            .findings_by_rule
            .iter()
            .map(|tally| {
                let mut segments = Vec::new();
                if tally.errors > 0 {
                    segments.push(format!("{} error(s)", tally.errors));
                }
                if tally.warnings > 0 {
                    segments.push(format!("{} warning(s)", tally.warnings));
                }
                format!(
                    "{} {} in {} file(s)",
                    tally.rule,
                    segments.join(", "),
                    tally.files
                )
            })
            .collect();
        out.push_str(&format!("  by rule: {}\n", parts.join("; ")));
    }
    out.trim_end().to_string()
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
fn run_upload(_: UploadArgs, _license: &LicenseResolution) -> std::process::ExitCode {
    die(
        "this build was compiled without moonraker support; rebuild with `cargo build --features moonraker`"
            .into(),
    )
}

#[cfg(feature = "moonraker")]
fn run_upload(args: UploadArgs, license: &LicenseResolution) -> std::process::ExitCode {
    use std::io::Cursor;
    use std::path::Path;
    use std::time::Duration;

    // License gate: refuse BEFORE any network contact, on parsed args alone.
    if matches!(license, LicenseResolution::Eval { .. }) {
        die("dry upload requires a license — see https://dry-public-docs.pages.dev/pricing".into());
    }
    license_notice(license);

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
        // cnc_frame is intentionally left None here for the same reason as the `RewriteGcode` arm:
        // this path also calls `emit_source_preserving_spans`, which emits each span in isolation
        // to recover per-span line offsets against a separately-emitted flattened reference. Setting
        // cnc_frame would desync that accounting (or duplicate the preamble per span and strand the
        // M30 postamble mid-file). Needs a dry-core design decision, tracked as follow-up.
        let emit_params = EmitParams {
            relative_e: true,
            travel_g1_e0: false,
            five_axis: false,
            kinematics: Kinematics::default(),
            flavor: profile
                .as_ref()
                .map(|p| p.emit_params().flavor)
                .unwrap_or(FirmwareFlavor::Marlin),
            cnc_frame: None,
            // Unused on this path: `emit_source_preserving_spans` refuses a KRL flavor outright,
            // because a DEF/END module is not a spliceable motion span.
            krl_frame: KrlFrame::default(),
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

//! `dry` — the toolpath compiler CLI. Operates on a Dry IR file (`{version, segments}`, or a fixture
//! wrapping it under an `ir` key). Phase-0 surface: `inspect` / `simulate` / `emit` (`docs/04-tasks.md`).

use clap::{Parser, Subcommand, ValueEnum};
use dry_core::{
    emit_stream_to_writer, import_gcode_reader, import_gcode_reader_with_map,
    optimize_aggressive_pipeline, optimize_pipeline, parse_bounds_csv, parse_speed_range_csv,
    simulate, simulate_stream, trace_summary_with_sources, verify, verify_stream, Contracts,
    EmitParams, FirmwareFlavor, GcodeImportParams, Kinematics, Profile, Toolpath,
};
use std::fs;
use std::io::Write;
use std::process::ExitCode;

/// CLI surface for [`Kinematics`]: the rotary kinematics selectable on `dry emit`.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum KinematicsArg {
    Ab,
    Ac,
    Bc,
}

impl From<KinematicsArg> for Kinematics {
    fn from(k: KinematicsArg) -> Self {
        match k {
            KinematicsArg::Ab => Kinematics::Ab {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            },
            KinematicsArg::Ac => Kinematics::Ac {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            },
            KinematicsArg::Bc => Kinematics::Bc {
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
        /// Rotary kinematics for 5-axis words (ab|ac|bc).
        #[arg(long, value_enum, default_value_t = KinematicsArg::Ab)]
        kinematics: KinematicsArg,
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
    /// Check a Dry IR file against machine-safety contracts; exits 1 if any errors are found.
    /// Flags: `--bounds`, `--max-flow`, `--monotonic-z`, `--min-temp`, `--json`.
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
        /// Print findings as JSON.
        #[arg(long)]
        json: bool,
    },
}

fn die(msg: String) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(2);
}

/// Load a Dry IR `Toolpath` from a file that is either a bare `{version, segments}` or a fixture with
/// an `ir` key.
fn load(file: &str) -> Toolpath {
    let text = fs::read_to_string(file).unwrap_or_else(|e| die(format!("cannot read {file}: {e}")));
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
            kinematics,
            out,
        } => {
            let stream =
                load_streaming(&file).unwrap_or_else(|e| die(format!("cannot stream {file}: {e}")));
            let params = EmitParams {
                relative_e: !absolute_e,
                travel_g1_e0: false,
                five_axis,
                kinematics: kinematics.into(),
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
                bounds.as_deref(),
                max_flow,
                speed_range.as_deref(),
                monotonic_z,
                min_temp,
            );
            let report = verify(&imported.toolpath, &contracts);

            if json {
                let findings: Vec<_> = report
                    .findings
                    .iter()
                    .map(|finding| {
                        let source_line = finding
                            .segment
                            .and_then(|segment| imported.source_line_for_segment(segment));
                        serde_json::json!({
                            "rule": &finding.rule,
                            "severity": finding.severity,
                            "segment": finding.segment,
                            "source_line": source_line,
                            "message": &finding.message,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "file": file,
                        "profile": profile_label(profile.as_ref()),
                        "segments": imported.toolpath.segments.len(),
                        "metrics": metrics,
                        "findings": findings,
                        "error_count": report.error_count(),
                    }))
                    .unwrap()
                );
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
                if report.findings.is_empty() {
                    println!("  verify:    OK (no findings)");
                } else {
                    for finding in &report.findings {
                        let seg = finding
                            .segment
                            .map(|i| format!(" seg {i}"))
                            .unwrap_or_default();
                        let line = finding
                            .segment
                            .and_then(|segment| imported.source_line_for_segment(segment))
                            .map(|line| format!(" line {line}"))
                            .unwrap_or_default();
                        println!(
                            "  [{:?}] {}{line}{seg}: {}",
                            finding.severity, finding.rule, finding.message
                        );
                    }
                    println!(
                        "  verify:    {} finding(s), {} error(s)",
                        report.findings.len(),
                        report.error_count()
                    );
                }
            }

            if report.ok() {
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
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "file": file,
                    "profile": profile_label(profile.as_ref()),
                    "trace": trace,
                }))
                .unwrap()
            );
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
            let span_toolpaths = imported
                .motion_spans()
                .into_iter()
                .map(|span| {
                    let range = span.segment_range();
                    let span_toolpath = Toolpath {
                        version: imported.toolpath.version,
                        meta: imported.toolpath.meta.clone(),
                        segments: imported.toolpath.segments[range].to_vec(),
                    };
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
        Cmd::Verify {
            file,
            profile,
            max_flow,
            bounds,
            monotonic_z,
            min_temp,
            speed_range,
            json,
        } => {
            let stream =
                load_streaming(&file).unwrap_or_else(|e| die(format!("cannot stream {file}: {e}")));
            let profile = load_profile(profile.as_deref());
            let contracts = contracts_from_inputs(
                profile.as_ref(),
                bounds.as_deref(),
                max_flow,
                speed_range.as_deref(),
                monotonic_z,
                min_temp,
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

fn contracts_from_inputs(
    profile: Option<&Profile>,
    bounds: Option<&str>,
    max_flow: Option<f64>,
    speed_range: Option<&str>,
    monotonic_z: bool,
    min_temp: Option<f64>,
) -> Contracts {
    let mut contracts = profile.map(Profile::contracts).unwrap_or_default();
    if let Some(bounds) = bounds {
        contracts.bounds = Some(parse_bounds(bounds));
    }
    if let Some(max_flow) = max_flow {
        contracts.max_flow = Some(max_flow);
    }
    if let Some(speed_range) = speed_range {
        contracts.speed_range = Some(parse_speed_range(speed_range));
    }
    if monotonic_z {
        contracts.monotonic_z = true;
    }
    if let Some(min_temp) = min_temp {
        contracts.min_temp = Some(min_temp);
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

fn main() -> ExitCode {
    run(Cli::parse())
}

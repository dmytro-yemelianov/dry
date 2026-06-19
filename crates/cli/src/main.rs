//! `dry` — the toolpath compiler CLI. Operates on a Dry IR file (`{version, segments}`, or a fixture
//! wrapping it under an `ir` key). Phase-0 surface: `inspect` / `simulate` / `emit` (`docs/04-tasks.md`).

use clap::{Parser, Subcommand, ValueEnum};
use dry_core::{
    arc_fit, emit, merge_collinear, simulate, travel_reorder, verify, Contracts, EmitParams,
    Kinematics, Toolpath,
};
use std::fs;
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
            KinematicsArg::Ab => Kinematics::Ab,
            KinematicsArg::Ac => Kinematics::Ac,
            KinematicsArg::Bc => Kinematics::Bc,
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
    /// Encode a Dry IR (JSON) file to the compact columnar binary form.
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
    /// Optimise a Dry IR file (merge collinear, fit arcs, reorder travel) and report the before/after.
    Optimize {
        file: String,
        /// Write the optimised IR JSON to a file.
        #[arg(short, long)]
        out: Option<String>,
    },
    /// Check a Dry IR file against machine-safety contracts; exits 1 if any errors are found.
    /// Flags: `--bounds`, `--max-flow`, `--monotonic-z`, `--min-temp`, `--json`.
    Verify {
        file: String,
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
            let m = simulate(&load(&file));
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
            let tp = load(&file);
            let params = EmitParams {
                relative_e: !absolute_e,
                travel_g1_e0: false,
                five_axis,
                kinematics: kinematics.into(),
            };
            let gcode = emit(&tp, &params).join("\n");
            match out {
                Some(path) => fs::write(&path, gcode + "\n")
                    .unwrap_or_else(|e| die(format!("cannot write {path}: {e}"))),
                None => println!("{gcode}"),
            }
            ExitCode::SUCCESS
        }
        Cmd::Pack { file, out } => {
            let bytes = load(&file).to_bytes();
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
        Cmd::Optimize { file, out } => {
            let tp = load(&file);
            let before = tp.segments.len();
            // run the three L2 passes in sequence: collinear merge, then fit arcs to circular runs,
            // then reorder independent extrusion runs to shorten total travel.
            let opt = travel_reorder(&arc_fit(&merge_collinear(&tp)));
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
                 volume {:.4}mm^3 (Δ{:.2e}), time {:.3}s (Δ{:.2e}) preserved",
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
            max_flow,
            bounds,
            monotonic_z,
            min_temp,
            speed_range,
            json,
        } => {
            let tp = load(&file);
            let contracts = Contracts {
                bounds: bounds.as_deref().map(parse_bounds),
                max_flow,
                speed_range: speed_range.as_deref().map(parse_speed_range),
                monotonic_z,
                min_temp,
            };
            let report = verify(&tp, &contracts);
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

/// Parse `x0,x1,y0,y1,z0,z1` into a build volume; exits 2 on a malformed value.
fn parse_bounds(s: &str) -> [[f64; 2]; 3] {
    let v: Vec<f64> = s
        .split(',')
        .map(|t| {
            t.trim()
                .parse()
                .unwrap_or_else(|_| die(format!("bad --bounds value {t:?}")))
        })
        .collect();
    if v.len() != 6 {
        die("--bounds needs 6 comma-separated numbers: x0,x1,y0,y1,z0,z1".into());
    }
    [[v[0], v[1]], [v[2], v[3]], [v[4], v[5]]]
}

/// Parse `min,max` into a speed range; exits 2 on a malformed value.
fn parse_speed_range(s: &str) -> [f64; 2] {
    let v: Vec<f64> = s
        .split(',')
        .map(|t| {
            t.trim()
                .parse()
                .unwrap_or_else(|_| die(format!("bad --speed-range value {t:?}")))
        })
        .collect();
    if v.len() != 2 {
        die("--speed-range needs 2 comma-separated numbers: min,max".into());
    }
    [v[0], v[1]]
}

fn main() -> ExitCode {
    run(Cli::parse())
}

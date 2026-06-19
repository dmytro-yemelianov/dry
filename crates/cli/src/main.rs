//! `dry` — the toolpath compiler CLI. Operates on a Dry IR file (`{version, segments}`, or a fixture
//! wrapping it under an `ir` key). Phase-0 surface: `inspect` / `simulate` / `emit` (`docs/04-tasks.md`).

use clap::{Parser, Subcommand};
use dry_core::{emit, simulate, EmitParams, Toolpath};
use std::fs;
use std::process::ExitCode;

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
            out,
        } => {
            let tp = load(&file);
            let params = EmitParams {
                relative_e: !absolute_e,
                travel_g1_e0: false,
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
    }
}

fn main() -> ExitCode {
    run(Cli::parse())
}

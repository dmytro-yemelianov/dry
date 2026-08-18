//! ISO 14649 STEP-NC manufacturing workingstep parser & importer (D1.8, `docs/20-dry-ir-ecosystem-implementation-plan.md` §6.5).
//!
//! Ingests enterprise CAD/CAM STEP-NC manufacturing features (drilling, pocketing, planar facing)
//! and lowers them directly into Dry L1 operations.

use crate::generate::{pocket_ops, CutMode, PocketOptions, PocketShape};
use crate::resolve::Op;
use serde::{Deserialize, Serialize};

/// High-level manufacturing feature defined in an ISO 14649 STEP-NC document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepNcFeature {
    PlanarFace {
        x: f64,
        y: f64,
        length: f64,
        width: f64,
    },
    RoundHole {
        x: f64,
        y: f64,
        diameter: f64,
        depth: f64,
    },
    ClosedPocket {
        x: f64,
        y: f64,
        length: f64,
        width: f64,
        depth: f64,
    },
}

/// An executable machining workingstep with process parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepNcWorkingstep {
    pub id: String,
    pub tool_id: Option<String>,
    pub feature: StepNcFeature,
    pub feedrate: Option<f64>,
    pub spindle_rpm: Option<f64>,
}

/// Parse STEP-NC / ISO 14649 document text (XML or JSON format).
pub fn parse_step_nc(text: &str) -> Result<Vec<StepNcWorkingstep>, &'static str> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("empty STEP-NC document");
    }

    // JSON format support
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        if let Ok(steps) = serde_json::from_str::<Vec<StepNcWorkingstep>>(trimmed) {
            return Ok(steps);
        }
        if let Ok(step) = serde_json::from_str::<StepNcWorkingstep>(trimmed) {
            return Ok(vec![step]);
        }
    }

    // XML-based ISO 14649 workingstep parser
    let mut steps = Vec::new();
    for line in trimmed.lines() {
        let l = line.trim();
        if l.starts_with("<workingstep ") || l.starts_with("<workingstep>") {
            let id = extract_attr(l, "id").unwrap_or_else(|| format!("ws-{}", steps.len() + 1));
            let step_type = extract_attr(l, "type").unwrap_or_default();
            let x = extract_attr_f64(l, "x").unwrap_or(0.0);
            let y = extract_attr_f64(l, "y").unwrap_or(0.0);
            let depth = extract_attr_f64(l, "depth").unwrap_or(5.0);
            let diam = extract_attr_f64(l, "diameter").unwrap_or(6.0);
            let width = extract_attr_f64(l, "width").unwrap_or(20.0);
            let length = extract_attr_f64(l, "length").unwrap_or(30.0);
            let feed = extract_attr_f64(l, "feed");
            let rpm = extract_attr_f64(l, "rpm");

            let feature = if step_type == "hole" || step_type == "drilling" {
                StepNcFeature::RoundHole {
                    x,
                    y,
                    diameter: diam,
                    depth,
                }
            } else if step_type == "pocket" {
                StepNcFeature::ClosedPocket {
                    x,
                    y,
                    length,
                    width,
                    depth,
                }
            } else {
                StepNcFeature::PlanarFace {
                    x,
                    y,
                    length,
                    width,
                }
            };

            steps.push(StepNcWorkingstep {
                id,
                tool_id: extract_attr(l, "tool"),
                feature,
                feedrate: feed,
                spindle_rpm: rpm,
            });
        }
    }

    if steps.is_empty() {
        // Fallback default hole step if no structured lines matched
        return Err("no valid workingsteps found in document");
    }

    Ok(steps)
}

fn extract_attr(line: &str, attr: &str) -> Option<String> {
    let key = format!("{attr}=\"");
    let pos = line.find(&key)?;
    let rem = &line[pos + key.len()..];
    let end = rem.find('"')?;
    Some(rem[..end].to_string())
}

fn extract_attr_f64(line: &str, attr: &str) -> Option<f64> {
    extract_attr(line, attr)?.parse::<f64>().ok()
}

/// Lower an ISO 14649 workingstep into Dry L1 operations.
pub fn lower_workingstep_to_ops(step: &StepNcWorkingstep) -> Vec<Op> {
    let mut ops = Vec::new();
    let feed = step.feedrate.unwrap_or(1200.0);

    match &step.feature {
        StepNcFeature::RoundHole {
            x,
            y,
            diameter: _,
            depth,
        } => {
            // Rapid above hole location
            ops.push(Op::Extruder { on: false });
            ops.push(Op::Move {
                x: Some(*x),
                y: Some(*y),
                z: Some(5.0),
            });
            // Plunge drilling cut
            ops.push(Op::Extruder { on: true });
            ops.push(Op::Speed { print: feed * 0.4 });
            ops.push(Op::Move {
                x: Some(*x),
                y: Some(*y),
                z: Some(-depth.abs()),
            });
            // Retract to clearance
            ops.push(Op::Extruder { on: false });
            ops.push(Op::Speed { print: feed });
            ops.push(Op::Move {
                x: Some(*x),
                y: Some(*y),
                z: Some(5.0),
            });
        }
        StepNcFeature::ClosedPocket {
            x,
            y,
            length,
            width,
            depth,
        } => {
            let opts = PocketOptions {
                shape: PocketShape::Rect {
                    x: *x,
                    y: *y,
                    width: *length,
                    height: *width,
                },
                mode: CutMode::Pocket,
                tool_diameter: 6.0,
                stepover: Some(0.5),
                depth: depth.abs(),
                depth_per_pass: Some(2.0),
                z_top: Some(0.0),
                safe_z: Some(5.0),
                cut_feed: Some(feed),
                plunge_feed: Some(feed * 0.4),
            };
            let pocket_ops = pocket_ops(&opts);
            ops.extend(pocket_ops);
        }
        StepNcFeature::PlanarFace {
            x,
            y,
            length,
            width,
        } => {
            // Facing raster passes across rectangular boundary
            ops.push(Op::Extruder { on: false });
            ops.push(Op::Move {
                x: Some(*x),
                y: Some(*y),
                z: Some(5.0),
            });
            ops.push(Op::Extruder { on: true });
            ops.push(Op::Speed { print: feed });
            ops.push(Op::Move {
                x: Some(*x),
                y: Some(*y),
                z: Some(0.0),
            });
            ops.push(Op::Move {
                x: Some(x + length),
                y: Some(*y),
                z: Some(0.0),
            });
            ops.push(Op::Move {
                x: Some(x + length),
                y: Some(y + width),
                z: Some(0.0),
            });
            ops.push(Op::Move {
                x: Some(*x),
                y: Some(y + width),
                z: Some(0.0),
            });
            ops.push(Op::Extruder { on: false });
            ops.push(Op::Move {
                x: Some(*x),
                y: Some(*y),
                z: Some(5.0),
            });
        }
    }

    ops
}

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
    Slot {
        x_start: f64,
        y_start: f64,
        x_end: f64,
        y_end: f64,
        depth: f64,
        width: f64,
    },
    PeckHole {
        x: f64,
        y: f64,
        diameter: f64,
        depth: f64,
        peck_depth: f64,
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
pub fn parse_step_nc(text: &str) -> Result<Vec<StepNcWorkingstep>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("empty STEP-NC document".to_string());
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
            // Every geometric attribute is required for the feature that uses it. Anything present
            // but unparseable or non-finite is refused wherever it appears, including on attributes
            // this feature type ignores — a malformed number is a malformed document.
            let feed = read_attr_f64(l, "feed", &id)?;
            let rpm = read_attr_f64(l, "rpm", &id)?;

            let feature = if step_type == "hole" || step_type == "drilling" {
                StepNcFeature::RoundHole {
                    x: require_attr_f64(l, "x", &id, "hole")?,
                    y: require_attr_f64(l, "y", &id, "hole")?,
                    diameter: require_attr_f64(l, "diameter", &id, "hole")?,
                    depth: require_attr_f64(l, "depth", &id, "hole")?,
                }
            } else if step_type == "peck_hole" || step_type == "peck_drilling" {
                StepNcFeature::PeckHole {
                    x: require_attr_f64(l, "x", &id, "peck_hole")?,
                    y: require_attr_f64(l, "y", &id, "peck_hole")?,
                    diameter: require_attr_f64(l, "diameter", &id, "peck_hole")?,
                    depth: require_attr_f64(l, "depth", &id, "peck_hole")?,
                    peck_depth: read_attr_f64(l, "peck", &id)?
                        .or_else(|| read_attr_f64(l, "q", &id).ok().flatten())
                        .unwrap_or(2.0),
                }
            } else if step_type == "slot" || step_type == "groove" {
                StepNcFeature::Slot {
                    x_start: read_attr_f64(l, "x_start", &id)?
                        .or_else(|| read_attr_f64(l, "x1", &id).ok().flatten())
                        .unwrap_or(0.0),
                    y_start: read_attr_f64(l, "y_start", &id)?
                        .or_else(|| read_attr_f64(l, "y1", &id).ok().flatten())
                        .unwrap_or(0.0),
                    x_end: require_attr_f64(l, "x_end", &id, "slot")
                        .or_else(|_| require_attr_f64(l, "x2", &id, "slot"))?,
                    y_end: require_attr_f64(l, "y_end", &id, "slot")
                        .or_else(|_| require_attr_f64(l, "y2", &id, "slot"))?,
                    depth: require_attr_f64(l, "depth", &id, "slot")?,
                    width: require_attr_f64(l, "width", &id, "slot")?,
                }
            } else if step_type == "pocket" {
                StepNcFeature::ClosedPocket {
                    x: require_attr_f64(l, "x", &id, "pocket")?,
                    y: require_attr_f64(l, "y", &id, "pocket")?,
                    length: require_attr_f64(l, "length", &id, "pocket")?,
                    width: require_attr_f64(l, "width", &id, "pocket")?,
                    depth: require_attr_f64(l, "depth", &id, "pocket")?,
                }
            } else {
                StepNcFeature::PlanarFace {
                    x: require_attr_f64(l, "x", &id, "face")?,
                    y: require_attr_f64(l, "y", &id, "face")?,
                    length: require_attr_f64(l, "length", &id, "face")?,
                    width: require_attr_f64(l, "width", &id, "face")?,
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
        return Err("no valid workingsteps found in document".to_string());
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

/// Read a numeric attribute, distinguishing "absent" from "present but unusable".
///
/// The previous reader collapsed the two: `extract_attr(..).parse().ok()` returned `None` for a
/// missing attribute and for `depth="12,5"` alike, and every caller then substituted a hard-coded
/// default. A European decimal comma, a unit suffix, or a typo therefore produced a valid-looking
/// program machined to the wrong depth in the wrong place, with nothing reported. `.parse::<f64>()`
/// also accepts `NaN` and `inf`, which reached the geometry unchallenged.
fn read_attr_f64(line: &str, attr: &str, id: &str) -> Result<Option<f64>, String> {
    let Some(raw) = extract_attr(line, attr) else {
        return Ok(None);
    };
    let value: f64 = raw
        .trim()
        .parse()
        .map_err(|_| format!("workingstep '{id}': attribute {attr}=\"{raw}\" is not a number"))?;
    if !value.is_finite() {
        return Err(format!(
            "workingstep '{id}': attribute {attr}=\"{raw}\" is not finite"
        ));
    }
    Ok(Some(value))
}

/// Read an attribute the feature's geometry cannot be placed without.
///
/// There is no safe default for a position or a depth: substituting one machines a real part at
/// coordinates nobody wrote.
fn require_attr_f64(line: &str, attr: &str, id: &str, feature: &str) -> Result<f64, String> {
    read_attr_f64(line, attr, id)?.ok_or_else(|| {
        format!("workingstep '{id}': a {feature} requires attribute {attr}, which is absent")
    })
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
                helical_entry: None,
                trochoidal: None,
                chip_thinning: None,
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
        StepNcFeature::PeckHole {
            x,
            y,
            diameter: _,
            depth,
            peck_depth,
        } => {
            let total_depth = depth.abs();
            let step_down = if *peck_depth <= 0.0 { 2.0 } else { *peck_depth };
            let mut current_z = 0.0;

            ops.push(Op::Extruder { on: false });
            ops.push(Op::Move {
                x: Some(*x),
                y: Some(*y),
                z: Some(5.0),
            });

            while current_z < total_depth {
                let target_z = (current_z + step_down).min(total_depth);
                if current_z > 0.0 {
                    ops.push(Op::Extruder { on: false });
                    ops.push(Op::Move {
                        x: Some(*x),
                        y: Some(*y),
                        z: Some(-current_z + 0.5),
                    });
                }
                ops.push(Op::Extruder { on: true });
                ops.push(Op::Speed { print: feed * 0.4 });
                ops.push(Op::Move {
                    x: Some(*x),
                    y: Some(*y),
                    z: Some(-target_z),
                });
                ops.push(Op::Extruder { on: false });
                ops.push(Op::Speed { print: feed });
                ops.push(Op::Move {
                    x: Some(*x),
                    y: Some(*y),
                    z: Some(5.0),
                });
                current_z = target_z;
            }
        }
        StepNcFeature::Slot {
            x_start,
            y_start,
            x_end,
            y_end,
            depth,
            width: _,
        } => {
            let total_depth = depth.abs();
            let step_down = 1.0;
            let mut current_z = 0.0;

            ops.push(Op::Extruder { on: false });
            ops.push(Op::Move {
                x: Some(*x_start),
                y: Some(*y_start),
                z: Some(5.0),
            });

            while current_z < total_depth {
                let next_z = (current_z + step_down).min(total_depth);
                ops.push(Op::Extruder { on: true });
                ops.push(Op::Speed { print: feed * 0.4 });
                ops.push(Op::Move {
                    x: Some(*x_start),
                    y: Some(*y_start),
                    z: Some(-next_z),
                });
                ops.push(Op::Speed { print: feed });
                ops.push(Op::Move {
                    x: Some(*x_end),
                    y: Some(*y_end),
                    z: Some(-next_z),
                });
                ops.push(Op::Extruder { on: false });
                ops.push(Op::Move {
                    x: Some(*x_end),
                    y: Some(*y_end),
                    z: Some(5.0),
                });
                if next_z < total_depth {
                    ops.push(Op::Move {
                        x: Some(*x_start),
                        y: Some(*y_start),
                        z: Some(5.0),
                    });
                }
                current_z = next_z;
            }
        }
    }

    ops
}

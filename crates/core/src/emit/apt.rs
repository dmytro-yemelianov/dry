//! ISO 4343 APT-CL Emitter.
//!
//! Generates standard APT-CL text with `PARTNO`, `MACHIN`, `MULTAX/ON`, `RAPID`, `FEDRAT`,
//! `MOVARC`, `GOTO`, `DELAY`, `LOADTL`, `SPINDL`, and `FINI` per `docs/28-apt-irbcam-dialects.md`.

use super::kinematics::unit_orientation;
use super::EmitParams;
use crate::codec::CodecError;
use crate::ir::{Segment, SegmentKind};
use serde::{Deserialize, Serialize};
use std::io::Write;

/// Configuration frame for APT-CL emission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AptFrame {
    /// Optional part number for `PARTNO / <name>`.
    pub partno: Option<String>,
    /// Optional machine specification for `MACHIN / <name>`.
    pub machin: Option<String>,
    /// Decimals for numeric fields (default 6).
    pub decimals: u8,
}

impl Default for AptFrame {
    fn default() -> Self {
        Self {
            partno: None,
            machin: None,
            decimals: 6,
        }
    }
}

fn signed_sweep(start: f64, end: f64, clockwise: bool) -> f64 {
    let tau = std::f64::consts::TAU;
    let forward = (end - start).rem_euclid(tau);
    if clockwise {
        if forward == 0.0 {
            -tau
        } else {
            forward - tau
        }
    } else if forward == 0.0 {
        tau
    } else {
        forward
    }
}

fn apt_num(v: f64, decimals: u8) -> Result<String, CodecError> {
    if !v.is_finite() {
        return Err(CodecError::Other(format!(
            "cannot emit non-finite value {v} to APT-CL program"
        )));
    }
    let v = if v == 0.0 { 0.0 } else { v };
    Ok(format!("{v:.decimals$}", decimals = decimals as usize))
}

/// Emits an APT-CL program from a stream of IR segments.
pub fn emit_apt_to_writer<I, W>(
    segments: I,
    params: &EmitParams,
    writer: &mut W,
) -> Result<(), CodecError>
where
    I: IntoIterator<Item = Result<Segment, CodecError>>,
    W: Write,
{
    let decimals = params.apt_frame.decimals;
    let mut materialized = Vec::new();
    let mut has_multax = false;

    let flattener = super::SplineFlatteningIterator::new(segments.into_iter());
    for seg_res in flattener {
        let seg = seg_res?;
        if seg.kind == SegmentKind::ManualGcode {
            return Err(CodecError::Other(
                "[apt-manual-gcode] manual G-code is not representable in APT-CL".to_string(),
            ));
        }
        if matches!(
            seg.kind,
            SegmentKind::Retract | SegmentKind::Unretract | SegmentKind::Deposit
        ) {
            return Err(CodecError::Other(
                "[apt-poseless] poseless extrusion segments cannot be emitted to APT-CL"
                    .to_string(),
            ));
        }
        if seg.orientation.is_some() {
            has_multax = true;
        }
        materialized.push(seg);
    }

    if let Some(ref partno) = params.apt_frame.partno {
        writeln!(writer, "PARTNO / {partno}")?;
    }
    if let Some(ref machin) = params.apt_frame.machin {
        writeln!(writer, "MACHIN / {machin}")?;
    }
    writeln!(writer, "PPRINT / dry {}", env!("CARGO_PKG_VERSION"))?;
    writeln!(writer, "UNITS / MM")?;

    if has_multax {
        writeln!(writer, "MULTAX / ON")?;
    }

    let mut current_pos = [0.0, 0.0, 0.0];
    let mut prev_tool: Option<u32> = None;
    let mut prev_power: Option<f64> = None;
    let mut last_feed: Option<f64> = None;

    for (seg_idx, seg) in materialized.into_iter().enumerate() {
        if let Some(tool) = seg.tool {
            if prev_tool != Some(tool) {
                writeln!(writer, "LOADTL / {tool}")?;
                prev_tool = Some(tool);
            }
        }

        if let Some(power) = seg.power {
            if !power.is_finite() || power < 0.0 {
                return Err(CodecError::Other(format!(
                    "spindle power must be finite and >= 0, got {power}"
                )));
            }
            if prev_power != Some(power) {
                if power == 0.0 {
                    writeln!(writer, "SPINDL / OFF")?;
                } else {
                    writeln!(writer, "SPINDL / RPM, {}, CLW", apt_num(power, decimals)?)?;
                }
                prev_power = Some(power);
            }
        }

        if seg.kind == SegmentKind::Dwell {
            if let Some(dwell_s) = seg.dwell_s {
                if dwell_s > 0.0 {
                    writeln!(writer, "DELAY / {}", apt_num(dwell_s, decimals)?)?;
                }
            }
            continue;
        }

        if seg.travel {
            writeln!(writer, "RAPID")?;
        } else {
            let speed = seg.speed.value();
            if speed <= 0.0 {
                return Err(CodecError::Other(format!(
                    "[apt-feed-unstated] segment {seg_idx}: feed move speed must be > 0 (got {speed})"
                )));
            }
            if last_feed != Some(speed) {
                writeln!(writer, "FEDRAT / MMPM, {}", apt_num(speed, decimals)?)?;
                last_feed = Some(speed);
            }
        }

        let mut start = current_pos;
        for (index, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
            if let Some(value) = seg.start[index] {
                let value = value.value();
                if !value.is_finite() {
                    return Err(CodecError::Other(format!(
                        "cannot emit non-finite start {axis} ({value})"
                    )));
                }
                start[index] = value;
            }
        }
        let mut end = start;
        for (index, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
            if let Some(value) = seg.end[index] {
                let value = value.value();
                if !value.is_finite() {
                    return Err(CodecError::Other(format!(
                        "cannot emit non-finite end {axis} ({value})"
                    )));
                }
                end[index] = value;
            }
        }
        let [sx, sy, sz] = start;
        let [ex, ey, ez] = end;

        let orientation_vec = if has_multax {
            let [i, j, k] = unit_orientation(seg.orientation).map_err(|e| {
                CodecError::Other(format!(
                    "[apt-orientation-degenerate] segment {seg_idx}: {e}"
                ))
            })?;
            Some([i, j, k])
        } else {
            None
        };

        if seg.kind == SegmentKind::Arc {
            if ex == sx && ey == sy {
                return Err(CodecError::Other(format!(
                    "[apt-arc-full-turn] segment {seg_idx} is a full turn arc: MOVARC requires distinct endpoints"
                )));
            }
            let [cx, cy] = seg
                .centre
                .map(|[x, y]| [x.value(), y.value()])
                .ok_or_else(|| {
                    CodecError::Other(format!("segment {seg_idx} is missing arc center"))
                })?;
            let nz = if seg.clockwise { -1.0 } else { 1.0 };
            let radius = libm::hypot(sx - cx, sy - cy);
            let start_angle = libm::atan2(sy - cy, sx - cx);
            let end_angle = libm::atan2(ey - cy, ex - cx);
            let sweep = signed_sweep(start_angle, end_angle, seg.clockwise);
            let sweep_deg = sweep.abs().to_degrees();
            if sweep_deg <= 0.0 || sweep_deg >= 360.0 {
                return Err(CodecError::Other(format!(
                    "[apt-arc-full-turn] segment {seg_idx} sweep {sweep_deg} deg violates 0 < theta < 360"
                )));
            }

            writeln!(
                writer,
                "MOVARC / {}, {}, {}, 0, 0, {}, {}, {}",
                apt_num(cx, decimals)?,
                apt_num(cy, decimals)?,
                apt_num(sz, decimals)?,
                nz as i32,
                apt_num(radius, decimals)?,
                apt_num(sweep_deg, decimals)?
            )?;
        }

        if let Some([i, j, k]) = orientation_vec {
            writeln!(
                writer,
                "GOTO / {}, {}, {}, {}, {}, {}",
                apt_num(ex, decimals)?,
                apt_num(ey, decimals)?,
                apt_num(ez, decimals)?,
                apt_num(i, decimals)?,
                apt_num(j, decimals)?,
                apt_num(k, decimals)?
            )?;
        } else {
            writeln!(
                writer,
                "GOTO / {}, {}, {}",
                apt_num(ex, decimals)?,
                apt_num(ey, decimals)?,
                apt_num(ez, decimals)?
            )?;
        }

        current_pos = [ex, ey, ez];
    }

    writeln!(writer, "FINI")?;
    Ok(())
}

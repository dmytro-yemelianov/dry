//! IRBCAM Target List Dialect Lowering (JSON and CSV).
//!
//! Generates modular IRBCAM target lists per `docs/28-apt-irbcam-dialects.md`.
//! Supports 6-DOF robot target points with Euler ZYZ orientations, planar arcs (via midpoint
//! type 1 targets), and modal sparse vectors for tool number and spindle speed.

use super::kinematics::unit_orientation;
use super::EmitParams;
use crate::codec::CodecError;
use crate::ir::{Segment, SegmentKind};
use serde::{Deserialize, Serialize};
use std::io::Write;

/// Angle unit representation for orientation angles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AngleUnit {
    #[default]
    Deg,
    Rad,
}

/// Encoding policy for rapid travel moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RapidEncoding {
    #[default]
    MinusOne,
    Explicit,
}

/// Policy for handling dwell segments in IRBCAM emission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DwellPolicy {
    #[default]
    Refuse,
    Drop,
}

/// Policy for handling extrusion and 3D printing channels (Flow B).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum ExtrusionCarry {
    #[default]
    Refuse,
    MotionOnly,
    ToolToggle {
        extrude_tool: u32,
        travel_tool: u32,
    },
    SpindleRate,
}

/// Layout format for IRBCAM output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum IrbcamLayout {
    #[default]
    Json,
    Csv,
}

/// Configuration frame for IRBCAM emission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrbcamFrame {
    /// Constant spin around tool axis (rz2) in degrees (default 0.0).
    pub spin_deg: f64,
    /// Angle unit for orientation (degrees or radians).
    pub angle_unit: AngleUnit,
    /// Whether the tool Z axis points toward the workpiece (default false).
    pub tool_z_toward_work: bool,
    /// Rapid move encoding policy (MinusOne or Explicit).
    pub rapid: RapidEncoding,
    /// Dwell handling policy (Refuse or Drop).
    pub dwell: DwellPolicy,
    /// Extrusion channel lowering policy.
    pub extrusion: ExtrusionCarry,
    /// Number of decimals in rendered output (6..=17, default 6).
    pub decimals: u8,
    /// Target layout format (JSON or CSV).
    pub layout: IrbcamLayout,
}

impl Default for IrbcamFrame {
    fn default() -> Self {
        Self {
            spin_deg: 0.0,
            angle_unit: AngleUnit::Deg,
            tool_z_toward_work: false,
            rapid: RapidEncoding::MinusOne,
            dwell: DwellPolicy::Refuse,
            extrusion: ExtrusionCarry::Refuse,
            decimals: 6,
            layout: IrbcamLayout::Json,
        }
    }
}

impl IrbcamFrame {
    /// Validates the configuration frame parameters.
    pub fn validate(&self) -> Result<(), String> {
        if !self.spin_deg.is_finite() {
            return Err("spin_deg must be finite".to_string());
        }
        if !(6..=17).contains(&self.decimals) {
            return Err(format!(
                "decimals must be between 6 and 17, got {}",
                self.decimals
            ));
        }
        if let ExtrusionCarry::ToolToggle {
            extrude_tool,
            travel_tool,
        } = self.extrusion
        {
            if extrude_tool == travel_tool {
                return Err("ToolToggle extrude_tool and travel_tool must be distinct".to_string());
            }
        }
        Ok(())
    }
}

/// Emission statistics for IRBCAM lowering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IrbcamEmitStats {
    pub targets: usize,
    pub arcs: usize,
    pub dropped_dwells: usize,
    pub dropped_poseless: usize,
    pub extrusion_segments_lowered_motion_only: usize,
}

#[derive(Debug, Clone)]
struct TargetPoint {
    x: f64,
    y: f64,
    z: f64,
    rz1: f64,
    ry: f64,
    rz2: f64,
    velocity: f64,
    kind: u8, // 0 for linear, 1 for midpoint
    tool: Option<u32>,
    power: Option<f64>,
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

fn irb_num(v: f64, decimals: u8) -> Result<String, CodecError> {
    if !v.is_finite() {
        return Err(CodecError::Other(format!(
            "cannot emit non-finite value {v} to IRBCAM target list"
        )));
    }
    let v = if v == 0.0 { 0.0 } else { v };
    Ok(format!("{v:.decimals$}", decimals = decimals as usize))
}

/// Emits an IRBCAM target list in JSON or CSV format.
pub fn emit_irbcam_to_writer<I, W>(
    segments: I,
    params: &EmitParams,
    writer: &mut W,
) -> Result<IrbcamEmitStats, CodecError>
where
    I: IntoIterator<Item = Result<Segment, CodecError>>,
    W: Write,
{
    if params.cnc_frame.is_some() {
        return Err(CodecError::Other(
            "IRBCAM emit cannot carry cnc_frame: CNC WCS/tool/spindle/coolant fields are not IRBCAM target frames"
                .to_string(),
        ));
    }

    params.irbcam_frame.validate().map_err(CodecError::Other)?;

    let layout = if params.flavor == crate::emit::FirmwareFlavor::IrbcamCsv {
        IrbcamLayout::Csv
    } else {
        params.irbcam_frame.layout
    };
    let decimals = params.irbcam_frame.decimals;

    let mut stats = IrbcamEmitStats::default();
    let mut targets: Vec<TargetPoint> = Vec::new();
    let mut current_pos = [0.0, 0.0, 0.0];

    let segments = super::SplineFlatteningIterator::new(segments.into_iter());

    for (seg_idx, segment) in segments.enumerate() {
        let segment = segment?;

        let is_poseless = matches!(
            segment.kind,
            SegmentKind::Retract | SegmentKind::Unretract | SegmentKind::Deposit
        );
        let has_extrusion_fields = segment.volume.value() != 0.0
            || segment.filament.value() != 0.0
            || segment.width.is_some()
            || segment.height.is_some()
            || segment.flow.is_some()
            || segment.temperature.is_some()
            || segment.fan.is_some();

        if is_poseless || has_extrusion_fields {
            match &params.irbcam_frame.extrusion {
                ExtrusionCarry::Refuse => {
                    return Err(CodecError::Other(format!(
                        "[irbcam-extrusion-unrepresentable] segment {seg_idx}: extrusion fields/kinds cannot be represented in IRBCAM target list"
                    )));
                }
                ExtrusionCarry::MotionOnly => {
                    if is_poseless {
                        stats.dropped_poseless += 1;
                        continue;
                    }
                    stats.extrusion_segments_lowered_motion_only += 1;
                }
                ExtrusionCarry::ToolToggle { .. } | ExtrusionCarry::SpindleRate => {
                    if is_poseless {
                        stats.dropped_poseless += 1;
                        continue;
                    }
                }
            }
        }

        if segment.kind == SegmentKind::ManualGcode {
            return Err(CodecError::Other(format!(
                "[irbcam-manual-gcode] segment {seg_idx}: manual G-code is not representable in IRBCAM target list"
            )));
        }

        if segment.kind == SegmentKind::Dwell {
            match params.irbcam_frame.dwell {
                DwellPolicy::Refuse => {
                    return Err(CodecError::Other(format!(
                        "[irbcam-dwell-unrepresentable] segment {seg_idx}: IRBCAM has no dwell/wait target"
                    )));
                }
                DwellPolicy::Drop => {
                    stats.dropped_dwells += 1;
                    continue;
                }
            }
        }

        let mut seg_tool = segment.tool;
        if let ExtrusionCarry::ToolToggle {
            extrude_tool,
            travel_tool,
        } = params.irbcam_frame.extrusion
        {
            if seg_tool.is_some() {
                return Err(CodecError::Other(format!(
                    "[irbcam-channel-collision] segment {seg_idx}: ToolToggle policy cannot overwrite commanded tool {}",
                    seg_tool.unwrap()
                )));
            }
            seg_tool = Some(if segment.travel {
                travel_tool
            } else {
                extrude_tool
            });
        }

        let mut seg_power = segment.power;
        if let ExtrusionCarry::SpindleRate = params.irbcam_frame.extrusion {
            if seg_power.is_some() {
                return Err(CodecError::Other(format!(
                    "[irbcam-channel-collision] segment {seg_idx}: SpindleRate policy cannot overwrite commanded power {}",
                    seg_power.unwrap()
                )));
            }
            if segment.travel {
                seg_power = Some(0.0);
            } else {
                let len = segment.length.value();
                let spd = segment.speed.value();
                if !(len > 0.0 && spd > 0.0) {
                    return Err(CodecError::Other(format!(
                        "[irbcam-rate-undefined] segment {seg_idx}: SpindleRate requires length > 0 and speed > 0 on extruding segments"
                    )));
                }
                seg_power = Some(segment.volume.value() * spd / (60.0 * len));
            }
        }

        if let Some(pwr) = seg_power {
            if !pwr.is_finite() || pwr < 0.0 {
                return Err(CodecError::Other(format!(
                    "[spindle-direction-unsupported] segment {seg_idx} commands CCLW or non-finite power ({pwr}): IRBCAM only supports unidirectional non-negative spindle speeds"
                )));
            }
        }

        let speed = segment.speed.value();
        let velocity = if segment.travel {
            match params.irbcam_frame.rapid {
                RapidEncoding::MinusOne => -1.0,
                RapidEncoding::Explicit => {
                    if speed <= 0.0 {
                        return Err(CodecError::Other(format!(
                            "[irbcam-travel-speed-unstated] segment {seg_idx}: rapid encoding explicit requires travel speed > 0 (got {speed})"
                        )));
                    }
                    speed / 60.0
                }
            }
        } else {
            if speed <= 0.0 {
                return Err(CodecError::Other(format!(
                    "[irbcam-feed-unstated] segment {seg_idx}: feed move speed must be > 0 (got {speed})"
                )));
            }
            speed / 60.0
        };

        let [i, j, k] = unit_orientation(segment.orientation).map_err(|e| {
            CodecError::Other(format!(
                "[irbcam-orientation-degenerate] segment {seg_idx}: {e}"
            ))
        })?;
        let sigma = if params.irbcam_frame.tool_z_toward_work {
            -1.0
        } else {
            1.0
        };
        let (i_prime, j_prime, k_prime) = (sigma * i, sigma * j, (sigma * k).clamp(-1.0, 1.0));
        let ry_rad = libm::acos(k_prime);
        let rz1_rad = if i_prime * i_prime + j_prime * j_prime != 0.0 {
            libm::atan2(j_prime, i_prime)
        } else {
            0.0
        };
        let rz2_rad = params.irbcam_frame.spin_deg.to_radians();
        let (rz1, ry, rz2) = match params.irbcam_frame.angle_unit {
            AngleUnit::Deg => (
                rz1_rad.to_degrees(),
                ry_rad.to_degrees(),
                params.irbcam_frame.spin_deg,
            ),
            AngleUnit::Rad => (rz1_rad, ry_rad, rz2_rad),
        };

        let mut start = current_pos;
        for (index, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
            if let Some(value) = segment.start[index] {
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
            if let Some(value) = segment.end[index] {
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

        if segment.kind == SegmentKind::Arc {
            if ez != sz {
                return Err(CodecError::Other(format!(
                    "[irbcam-arc-helical] segment {seg_idx} rises from Z {sz} to Z {ez}: IRBCAM arc is planar XY and cannot climb"
                )));
            }
            if ex == sx && ey == sy {
                return Err(CodecError::Other(format!(
                    "[irbcam-arc-full-turn] segment {seg_idx} is a full turn arc: IRBCAM requires distinct endpoints"
                )));
            }
            let [cx, cy] = segment
                .centre
                .map(|[x, y]| [x.value(), y.value()])
                .ok_or_else(|| {
                    CodecError::Other(format!(
                        "[irbcam-arc-zero-radius] segment {seg_idx} is missing arc center"
                    ))
                })?;
            let rho = libm::hypot(sx - cx, sy - cy);
            if !(rho.is_finite() && rho > 0.0) {
                return Err(CodecError::Other(format!(
                    "[irbcam-arc-zero-radius] segment {seg_idx} arc start radius is not positive: {rho}"
                )));
            }

            let start_angle = libm::atan2(sy - cy, sx - cx);
            let end_angle = libm::atan2(ey - cy, ex - cx);
            let sweep = signed_sweep(start_angle, end_angle, segment.clockwise);
            let middle_angle = start_angle + sweep / 2.0;
            let mx = cx + rho * libm::cos(middle_angle);
            let my = cy + rho * libm::sin(middle_angle);
            let mz = sz;

            targets.push(TargetPoint {
                x: mx,
                y: my,
                z: mz,
                rz1,
                ry,
                rz2,
                velocity,
                kind: 1,
                tool: seg_tool,
                power: seg_power,
            });

            targets.push(TargetPoint {
                x: ex,
                y: ey,
                z: ez,
                rz1,
                ry,
                rz2,
                velocity,
                kind: 0,
                tool: seg_tool,
                power: seg_power,
            });

            stats.arcs += 1;
            stats.targets += 2;
            current_pos = [ex, ey, ez];
        } else {
            targets.push(TargetPoint {
                x: ex,
                y: ey,
                z: ez,
                rz1,
                ry,
                rz2,
                velocity,
                kind: 0,
                tool: seg_tool,
                power: seg_power,
            });
            stats.targets += 1;
            current_pos = [ex, ey, ez];
        }
    }

    // Modal sparse vector extraction and Some -> None transition refusal.
    let mut tool_number: Vec<(usize, u32)> = Vec::new();
    let mut spindle_speed: Vec<(usize, f64)> = Vec::new();
    let mut prev_tool: Option<u32> = None;
    let mut prev_power: Option<f64> = None;
    let mut saw_tool = false;
    let mut saw_power = false;

    for (target_idx, target) in targets.iter().enumerate() {
        if saw_tool && target.tool.is_none() {
            return Err(CodecError::Other(format!(
                "[irbcam-channel-uncommanded] target {target_idx}: tool channel transitions from Some to None, which is unrepresentable in IRBCAM"
            )));
        }
        if let Some(t) = target.tool {
            saw_tool = true;
            if target_idx == 0 || prev_tool != Some(t) {
                tool_number.push((target_idx, t));
                prev_tool = Some(t);
            }
        }

        if saw_power && target.power.is_none() {
            return Err(CodecError::Other(format!(
                "[irbcam-channel-uncommanded] target {target_idx}: spindle power channel transitions from Some to None, which is unrepresentable in IRBCAM"
            )));
        }
        if let Some(p) = target.power {
            saw_power = true;
            if target_idx == 0 || prev_power != Some(p) {
                spindle_speed.push((target_idx, p));
                prev_power = Some(p);
            }
        }
    }

    match layout {
        IrbcamLayout::Json => {
            writeln!(writer, "{{")?;
            writeln!(writer, "  \"targets\": [")?;
            for (i, t) in targets.iter().enumerate() {
                let comma = if i + 1 < targets.len() { "," } else { "" };
                writeln!(writer, "    {{")?;
                writeln!(writer, "      \"x\": {},", irb_num(t.x, decimals)?)?;
                writeln!(writer, "      \"y\": {},", irb_num(t.y, decimals)?)?;
                writeln!(writer, "      \"z\": {},", irb_num(t.z, decimals)?)?;
                writeln!(writer, "      \"rz1\": {},", irb_num(t.rz1, decimals)?)?;
                writeln!(writer, "      \"ry\": {},", irb_num(t.ry, decimals)?)?;
                writeln!(writer, "      \"rz2\": {},", irb_num(t.rz2, decimals)?)?;
                writeln!(
                    writer,
                    "      \"velocity\": {},",
                    irb_num(t.velocity, decimals)?
                )?;
                writeln!(writer, "      \"type\": {}", t.kind)?;
                writeln!(writer, "    }}{comma}")?;
            }
            writeln!(writer, "  ],")?;
            writeln!(writer, "  \"toolNumber\": [")?;
            for (i, (idx, tool)) in tool_number.iter().enumerate() {
                let comma = if i + 1 < tool_number.len() { "," } else { "" };
                writeln!(writer, "    {{")?;
                writeln!(writer, "      \"targetIndex\": {idx},")?;
                writeln!(writer, "      \"value\": {tool}")?;
                writeln!(writer, "    }}{comma}")?;
            }
            writeln!(writer, "  ],")?;
            writeln!(writer, "  \"spindleSpeed\": [")?;
            for (i, (idx, speed)) in spindle_speed.iter().enumerate() {
                let comma = if i + 1 < spindle_speed.len() { "," } else { "" };
                writeln!(writer, "    {{")?;
                writeln!(writer, "      \"targetIndex\": {idx},")?;
                writeln!(writer, "      \"value\": {}", irb_num(*speed, decimals)?)?;
                writeln!(writer, "    }}{comma}")?;
            }
            writeln!(writer, "  ]")?;
            writeln!(writer, "}}")?;
        }
        IrbcamLayout::Csv => {
            writeln!(writer, "x,y,z,rz1,ry,rz2,velocity,type")?;
            for t in &targets {
                writeln!(
                    writer,
                    "{},{},{},{},{},{},{},{}",
                    irb_num(t.x, decimals)?,
                    irb_num(t.y, decimals)?,
                    irb_num(t.z, decimals)?,
                    irb_num(t.rz1, decimals)?,
                    irb_num(t.ry, decimals)?,
                    irb_num(t.rz2, decimals)?,
                    irb_num(t.velocity, decimals)?,
                    t.kind,
                )?;
            }
        }
    }

    Ok(stats)
}

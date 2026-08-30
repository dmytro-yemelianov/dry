//! Reversing pass (`reverse(toolpath) -> Design`)
//!
//! Reconstructs a structured L1 `Design` operation list from an L2 resolved `Toolpath`.
//! Channel state updates (`Temperature`, `Fan`, `Flow`, `Tool`, `Power`, `Orient`) are emitted only when they change from the running state.

use crate::ir::{SegmentKind, Toolpath};
use crate::resolve::{Design, Op};

#[derive(Debug, Clone, PartialEq)]
pub struct ReverseError {
    pub message: String,
}

impl std::fmt::Display for ReverseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "reverse error: {}", self.message)
    }
}

impl std::error::Error for ReverseError {}

/// Reconstructs an L1 [`Design`] from an L2 [`Toolpath`].
pub fn reverse(toolpath: &Toolpath) -> Result<Design, ReverseError> {
    let mut ops = Vec::new();
    let mut current_temp: Option<f64> = None;
    let mut current_fan: Option<f64> = None;
    let mut current_flow: f64 = 1.0;
    let mut current_tool: Option<u32> = None;
    let mut current_power: Option<f64> = None;
    let mut current_orient: Option<[f64; 3]> = None;
    let mut current_extruder_on: Option<bool> = None;
    let mut current_speed: Option<f64> = None;

    for (idx, seg) in toolpath.segments.iter().enumerate() {
        // Temperature channel
        if seg.temperature != current_temp {
            if let Some(t) = seg.temperature {
                ops.push(Op::Temperature { nozzle: t });
                current_temp = Some(t);
            }
        }

        // Fan channel
        if seg.fan != current_fan {
            if let Some(f) = seg.fan {
                ops.push(Op::Fan { speed: f });
                current_fan = Some(f);
            }
        }

        // Flow channel (default 1.0)
        let seg_flow = seg.flow.unwrap_or(1.0);
        if (seg_flow - current_flow).abs() > 1e-9 {
            ops.push(Op::Flow { ratio: seg_flow });
            current_flow = seg_flow;
        }

        // Tool channel
        if seg.tool != current_tool {
            if let Some(tool_idx) = seg.tool {
                ops.push(Op::Tool { index: tool_idx });
                current_tool = Some(tool_idx);
            }
        }

        // Power channel
        if seg.power != current_power {
            if let Some(level) = seg.power {
                ops.push(Op::Power { level });
                current_power = Some(level);
            }
        }

        // Orientation channel
        if seg.orientation != current_orient {
            if let Some([i, j, k]) = seg.orientation {
                ops.push(Op::Orient { i, j, k });
                current_orient = Some([i, j, k]);
            }
        }

        // Extruder state
        let is_extruding = !seg.travel || matches!(seg.kind, SegmentKind::Deposit);
        if current_extruder_on != Some(is_extruding) {
            ops.push(Op::Extruder { on: is_extruding });
            current_extruder_on = Some(is_extruding);
        }

        // Speed / Feedrate
        let speed_val = seg.speed.0;
        if seg.kind != SegmentKind::Dwell
            && speed_val > 0.0
            && current_speed
                .map(|s| (s - speed_val).abs() > 1e-6)
                .unwrap_or(true)
        {
            ops.push(Op::Speed { print: speed_val });
            current_speed = Some(speed_val);
        }

        // Motion / Dwell ops
        match seg.kind {
            SegmentKind::Dwell => {
                let seconds = seg.dwell_s.ok_or_else(|| ReverseError {
                    message: format!("segment[{idx}] of kind Dwell missing dwell_s"),
                })?;
                ops.push(Op::Dwell { seconds });
            }
            SegmentKind::Line
            | SegmentKind::Deposit
            | SegmentKind::Retract
            | SegmentKind::Unretract => {
                let x = seg.end[0].map(|l| l.0);
                let y = seg.end[1].map(|l| l.0);
                let z = seg.end[2].map(|l| l.0);
                ops.push(Op::Move { x, y, z });
            }
            SegmentKind::Arc => {
                let cx = seg.centre.map(|c| c[0].0).ok_or_else(|| ReverseError {
                    message: format!("segment[{idx}] of kind Arc missing centre.x"),
                })?;
                let cy = seg.centre.map(|c| c[1].0).ok_or_else(|| ReverseError {
                    message: format!("segment[{idx}] of kind Arc missing centre.y"),
                })?;
                let x = seg.end[0].map(|l| l.0);
                let y = seg.end[1].map(|l| l.0);
                let z = seg.end[2].map(|l| l.0);
                ops.push(Op::Arc {
                    cx,
                    cy,
                    x,
                    y,
                    z,
                    clockwise: seg.clockwise,
                });
            }
            SegmentKind::Spline => {
                let points = seg
                    .control_points
                    .as_ref()
                    .map(|pts| {
                        pts.iter()
                            .map(|p| [Some(p[0].0), Some(p[1].0), Some(p[2].0)])
                            .collect()
                    })
                    .unwrap_or_default();
                ops.push(Op::Spline { points });
            }
            SegmentKind::ManualGcode => {
                let text = seg.manual_gcode.clone().unwrap_or_default();
                ops.push(Op::ManualGcode { text });
            }
        }
    }

    Ok(Design { ops })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{resolve_checked, ResolveParams};

    #[test]
    fn reverse_round_trips_simple_moves_and_channels() {
        let original = Design {
            ops: vec![
                Op::Temperature { nozzle: 210.0 },
                Op::Fan { speed: 1.0 },
                Op::Flow { ratio: 1.1 },
                Op::Move {
                    x: Some(10.0),
                    y: Some(0.0),
                    z: Some(0.0),
                },
                Op::Move {
                    x: Some(20.0),
                    y: Some(0.0),
                    z: Some(0.0),
                },
            ],
        };

        let toolpath = resolve_checked(&original, &ResolveParams::default()).unwrap();
        let reversed = reverse(&toolpath).unwrap();
        let re_resolved = resolve_checked(&reversed, &ResolveParams::default()).unwrap();

        assert_eq!(toolpath.segments.len(), re_resolved.segments.len());
        for (s1, s2) in toolpath.segments.iter().zip(&re_resolved.segments) {
            assert_eq!(s1.end, s2.end);
            assert_eq!(s1.temperature, s2.temperature);
            assert_eq!(s1.fan, s2.fan);
            assert_eq!(s1.flow, s2.flow);
        }
    }
}

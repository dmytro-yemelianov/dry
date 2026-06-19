//! `resolve` — lower an L1 *design* (a sequence of authoring ops) to an L2 [`Toolpath`]
//! (`docs/01-architecture.md` §1/§4). This is the deposition + state-propagation pass: it tracks the
//! running toolframe, the extrusion geometry, the extruder on/off and the speed, and computes each
//! move's length, deposited volume and filament. The math is Dry's independent reimplementation of
//! FullControl's *behaviour* (clean-room), gated byte-for-output against the oracle.
//!
//! Deposition (the rectangle bead model, reproduced from the oracle): `volume = length·width·height`,
//! `filament = volume / (π·(dia/2)²)`; a travel move deposits nothing. Arc length is
//! `hypot(radius·swept_angle, Δz)` (planar arc length, with the helical rise).

use crate::ir::{Segment, Toolpath};
use crate::units::{Angle, Area, Feedrate, Length, Volume};
use serde::Deserialize;
use std::f64::consts::TAU;

/// One L1 authoring op (the resolution-independent design layer). The Python/TS/Rust SDKs emit these
/// (serialised internally-tagged: `{"op":"move","x":..,"y":..,"z":..}`).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum Op {
    /// Set the extrusion bead cross-section (mm).
    Geometry { width: f64, height: f64 },
    /// Turn the extruder on/off (off ⇒ subsequent moves are travels).
    Extruder { on: bool },
    /// Set the print feedrate (mm/min).
    Speed { print: f64 },
    /// Set the nozzle temperature channel (°C).
    Temperature { nozzle: f64 },
    /// Set the part-cooling fan channel (0..1).
    Fan { speed: f64 },
    /// Set the flow multiplier channel (scales deposited volume; default 1.0).
    Flow { ratio: f64 },
    /// Set the active tool channel.
    Tool { index: u32 },
    /// Set the toolframe orientation: the tool-direction vector `(i, j, k)` (§2). Identity is `+Z`.
    Orient { i: f64, j: f64, k: f64 },
    /// Pause in place for `seconds` (emits a `G4` dwell; adds to the simulated time).
    Dwell { seconds: f64 },
    /// Move to a point; an axis left `None` is inherited from the running position.
    Move {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
    },
    /// A circular arc about `(cx, cy)` to an end point (inheriting `None` axes); `clockwise` ⇒ G2.
    Arc {
        cx: f64,
        cy: f64,
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
        clockwise: bool,
    },
}

/// A design: an ordered list of L1 ops.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Design {
    pub ops: Vec<Op>,
}

/// Machine/material defaults the lowering needs (from the device profile).
#[derive(Debug, Clone, Deserialize)]
pub struct ResolveParams {
    pub print_speed: f64,
    pub travel_speed: f64,
    pub dia: f64,
}

impl Default for ResolveParams {
    fn default() -> Self {
        // the "generic" printer's defaults.
        ResolveParams {
            print_speed: 1000.0,
            travel_speed: 8000.0,
            dia: 1.75,
        }
    }
}

fn dist(a: [Option<Length>; 3], b: [Option<Length>; 3]) -> Length {
    let mut sq = Area::ZERO;
    for i in 0..3 {
        if let (Some(p), Some(q)) = (a[i], b[i]) {
            let d = q - p;
            sq = sq + d * d;
        }
    }
    sq.sqrt()
}

/// Lower an L1 design to an L2 toolpath.
pub fn resolve(design: &Design, p: &ResolveParams) -> Toolpath {
    // bead cross-section of the round filament: π·(dia/2)².
    let half = Length::mm(p.dia) / 2.0;
    let area = std::f64::consts::PI * (half * half);
    let travel_speed = Feedrate(p.travel_speed);
    let mut pos: [Option<Length>; 3] = [None, None, None];
    let (mut width, mut height) = (Length::ZERO, Length::ZERO);
    let mut on = false;
    let mut print = Feedrate(p.print_speed);
    // process channels (§3): defaulted, propagated forward, attached to each emitted segment.
    let mut temp: Option<f64> = None;
    let mut fan: Option<f64> = None;
    let mut flow = 1.0_f64;
    let mut tool: Option<u32> = None;
    let mut orientation: Option<[f64; 3]> = None;
    let mut segs: Vec<Segment> = Vec::new();

    for op in &design.ops {
        // a flow multiplier of exactly 1.0 is the default and is omitted from the segment (so the wire
        // form is unchanged for flow-free designs); `length·width·height·1.0` is exact, preserving bytes.
        let flow_field = if flow == 1.0 { None } else { Some(flow) };
        match *op {
            Op::Geometry {
                width: w,
                height: h,
            } => {
                width = Length::mm(w);
                height = Length::mm(h);
            }
            Op::Extruder { on: o } => on = o,
            Op::Speed { print: s } => print = Feedrate(s),
            Op::Temperature { nozzle } => temp = Some(nozzle),
            Op::Fan { speed } => fan = Some(speed),
            Op::Flow { ratio } => flow = ratio,
            Op::Tool { index } => tool = Some(index),
            Op::Orient { i, j, k } => orientation = Some([i, j, k]),
            Op::Dwell { seconds } => segs.push(Segment {
                start: pos,
                end: pos,
                travel: true,
                speed: Feedrate::ZERO,
                length: Length::ZERO,
                volume: Volume::ZERO,
                filament: Length::ZERO,
                width: None,
                height: None,
                kind: "dwell".to_string(),
                centre: None,
                clockwise: false,
                temperature: temp,
                fan,
                flow: None,
                tool,
                dwell_s: Some(seconds),
                orientation,
            }),
            Op::Move { x, y, z } => {
                let end = [
                    x.map(Length::mm).or(pos[0]),
                    y.map(Length::mm).or(pos[1]),
                    z.map(Length::mm).or(pos[2]),
                ];
                let length = dist(pos, end);
                let volume = if on {
                    length * width * height * flow
                } else {
                    Volume::ZERO
                };
                segs.push(Segment {
                    start: pos,
                    end,
                    travel: !on,
                    speed: if on { print } else { travel_speed },
                    length,
                    volume,
                    filament: volume / area,
                    width: Some(width),
                    height: Some(height),
                    kind: "line".to_string(),
                    centre: None,
                    clockwise: false,
                    temperature: temp,
                    fan,
                    flow: flow_field,
                    tool,
                    dwell_s: None,
                    orientation,
                });
                pos = end;
            }
            Op::Arc {
                cx,
                cy,
                x,
                y,
                z,
                clockwise,
            } => {
                let (cx, cy) = (Length::mm(cx), Length::mm(cy));
                let end = [
                    x.map(Length::mm).or(pos[0]),
                    y.map(Length::mm).or(pos[1]),
                    z.map(Length::mm).or(pos[2]),
                ];
                let (sx, sy) = (
                    pos[0].unwrap_or(Length::ZERO),
                    pos[1].unwrap_or(Length::ZERO),
                );
                let (ex, ey) = (end[0].unwrap_or(sx), end[1].unwrap_or(sy));
                let radius = (sx - cx).hypot(sy - cy);
                let start_a = (sy - cy).atan2(sx - cx);
                let end_a = (ey - cy).atan2(ex - cx);
                let mut swept = if clockwise {
                    start_a - end_a
                } else {
                    end_a - start_a
                } % TAU;
                if swept <= Angle::ZERO {
                    swept = swept + Angle(TAU);
                }
                let dz = match (pos[2], end[2]) {
                    (Some(a), Some(b)) => b - a,
                    _ => Length::ZERO,
                };
                let length = (radius * swept).hypot(dz);
                let volume = if on {
                    length * width * height * flow
                } else {
                    Volume::ZERO
                };
                segs.push(Segment {
                    start: pos,
                    end,
                    travel: !on,
                    speed: if on { print } else { travel_speed },
                    length,
                    volume,
                    filament: volume / area,
                    width: Some(width),
                    height: Some(height),
                    kind: "arc".to_string(),
                    centre: Some([cx, cy]),
                    clockwise,
                    temperature: temp,
                    fan,
                    flow: flow_field,
                    tool,
                    dwell_s: None,
                    orientation,
                });
                pos = end;
            }
        }
    }
    Toolpath {
        version: 0,
        segments: segs,
    }
}

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
use std::f64::consts::TAU;

/// One L1 authoring op (the resolution-independent design layer). The Python/TS/Rust SDKs emit these.
#[derive(Debug, Clone)]
pub enum Op {
    /// Set the extrusion bead cross-section (mm).
    Geometry { width: f64, height: f64 },
    /// Turn the extruder on/off (off ⇒ subsequent moves are travels).
    Extruder { on: bool },
    /// Set the print feedrate (mm/min).
    Speed { print: f64 },
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
#[derive(Debug, Clone, Default)]
pub struct Design {
    pub ops: Vec<Op>,
}

/// Machine/material defaults the lowering needs (from the device profile).
#[derive(Debug, Clone)]
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

fn dist(a: [Option<f64>; 3], b: [Option<f64>; 3]) -> f64 {
    let mut sq = 0.0;
    for i in 0..3 {
        if let (Some(p), Some(q)) = (a[i], b[i]) {
            sq += (q - p) * (q - p);
        }
    }
    sq.sqrt()
}

/// Lower an L1 design to an L2 toolpath.
pub fn resolve(design: &Design, p: &ResolveParams) -> Toolpath {
    let area = std::f64::consts::PI * (p.dia / 2.0).powi(2);
    let mut pos: [Option<f64>; 3] = [None, None, None];
    let (mut width, mut height) = (0.0_f64, 0.0_f64);
    let mut on = false;
    let mut print = p.print_speed;
    let mut segs: Vec<Segment> = Vec::new();

    for op in &design.ops {
        match *op {
            Op::Geometry {
                width: w,
                height: h,
            } => {
                width = w;
                height = h;
            }
            Op::Extruder { on: o } => on = o,
            Op::Speed { print: s } => print = s,
            Op::Move { x, y, z } => {
                let end = [x.or(pos[0]), y.or(pos[1]), z.or(pos[2])];
                let length = dist(pos, end);
                let volume = if on { length * width * height } else { 0.0 };
                segs.push(Segment {
                    start: pos,
                    end,
                    travel: !on,
                    speed: if on { print } else { p.travel_speed },
                    length,
                    volume,
                    filament: volume / area,
                    width: Some(width),
                    height: Some(height),
                    kind: "line".to_string(),
                    centre: None,
                    clockwise: false,
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
                let end = [x.or(pos[0]), y.or(pos[1]), z.or(pos[2])];
                let (sx, sy) = (pos[0].unwrap_or(0.0), pos[1].unwrap_or(0.0));
                let (ex, ey) = (end[0].unwrap_or(sx), end[1].unwrap_or(sy));
                let radius = (sx - cx).hypot(sy - cy);
                let start_a = (sy - cy).atan2(sx - cx);
                let end_a = (ey - cy).atan2(ex - cx);
                let mut swept = if clockwise {
                    start_a - end_a
                } else {
                    end_a - start_a
                } % TAU;
                if swept <= 0.0 {
                    swept += TAU;
                }
                let dz = match (pos[2], end[2]) {
                    (Some(a), Some(b)) => b - a,
                    _ => 0.0,
                };
                let length = (radius * swept).hypot(dz);
                let volume = if on { length * width * height } else { 0.0 };
                segs.push(Segment {
                    start: pos,
                    end,
                    travel: !on,
                    speed: if on { print } else { p.travel_speed },
                    length,
                    volume,
                    filament: volume / area,
                    width: Some(width),
                    height: Some(height),
                    kind: "arc".to_string(),
                    centre: Some([cx, cy]),
                    clockwise,
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

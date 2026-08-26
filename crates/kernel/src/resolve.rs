//! `resolve` — lower an L1 *design* (a sequence of authoring ops) to an L2 [`Toolpath`]
//! (`docs/01-architecture.md` §1/§4). This is the deposition + state-propagation pass: it tracks the
//! running toolframe, the extrusion geometry, the extruder on/off and the speed, and computes each
//! move's length, deposited volume and filament. The math is Dry's independent reimplementation of
//! FullControl's *behaviour* (clean-room), gated byte-for-output against the oracle.
//!
//! Deposition (the rectangle bead model, reproduced from the oracle): `volume = length·width·height`,
//! `filament = volume / (π·(dia/2)²)`; a travel move deposits nothing. Arc length is
//! `hypot(radius·swept_angle, Δz)` (planar arc length, with the helical rise).

use crate::ir::{Segment, SegmentKind, Toolpath};
use crate::units::{Angle, Area, Feedrate, Length, Volume};
// The L1 arc gate below and `verify`'s `arc-radius` rule are one policy, so they are one constant:
// `drymachina-contracts` owns the definition and this is that epsilon, not a second copy of its value.
// Published as `FM1.F64.VERIFY.ARC_RADIUS` in `proofs/verify-numeric-boundaries-v0.toml`, which pins
// the definition — a copy here would be outside that pin and could be retuned apart from it.
use drymachina_contracts::ARC_RADIUS_TOLERANCE_MM;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

/// A validation error found before lowering L1 ops to L2 motion.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolveError {
    message: String,
}

impl ResolveError {
    fn new(message: impl Into<String>) -> Self {
        ResolveError {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ResolveError {}

/// One L1 authoring op (the resolution-independent design layer). The Python/TS/Rust SDKs emit these
/// (serialised internally-tagged: `{"op":"move","x":..,"y":..,"z":..}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum Op {
    /// Set the extrusion bead cross-section (mm).
    Geometry {
        width: Option<f64>,
        height: Option<f64>,
    },
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
    /// Set the spindle/laser power channel (the target controller's `S` word value: RPM for a
    /// spindle, PWM counts for a laser). `0.0` means commanded off.
    Power { level: f64 },
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
    /// A Catmull-Rom spline starting at the running position (P0) and passing through each control
    /// point in `points` (a list of `[x, y, z]`, each axis `None` ⇒ inherited from the running
    /// position). Lowered to line segments in `resolve` (sampling `SAMPLES` points per span).
    Spline { points: Vec<[Option<f64>; 3]> },
    /// A clothoid (Euler-spiral) corner blend: travel from the running position toward the corner
    /// `(corner_x, corner_y)`, round it with a symmetric pair of Euler spirals consuming `blend` mm
    /// of tangent length from each leg, and continue to the end point (inheriting `None` axes).
    ///
    /// `(corner_x, corner_y)` is a *construction* point the path never visits, exactly like
    /// [`Op::Arc`]'s `(cx, cy)`, and Z rises linearly along the planar path the same way an arc's
    /// helical rise does. Unlike `Spline`, this lowers straight to line segments in `resolve`
    /// (`2·SAMPLES` for the blend plus the two straight legs), so no L2 consumer learns a new kind.
    /// See [`crate::clothoid`] for the geometry, the design rationale and the series tolerance.
    Clothoid {
        corner_x: f64,
        corner_y: f64,
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
        blend: f64,
    },
    /// Verbatim custom G-code injection.
    #[serde(rename = "manual_gcode")]
    ManualGcode { text: String },
    /// Explicit E-axis retraction.
    Retract {
        distance: Option<f64>,
        speed: Option<f64>,
    },
    /// Explicit E-axis unretraction/prime.
    Unretract {
        distance: Option<f64>,
        speed: Option<f64>,
    },
    /// Stationary extrusion of a set volume (mm³).
    Deposit { volume: f64, speed: f64 },
}

/// Intermediate samples emitted per Catmull-Rom span (between consecutive through-points).
pub const SAMPLES: usize = 16;

/// A design: an ordered list of L1 ops.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Design {
    pub ops: Vec<Op>,
}

/// Machine/material defaults the lowering needs (from the device profile).
///
/// `Serialize` is symmetric with the `Deserialize` (and gives the two `skip_serializing_if`
/// attributes below something to act on) so a caller can publish the exact parameters an IR was
/// resolved under instead of hand-mirroring the field list — `conformance/vectors/*/design.json`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResolveParams {
    pub print_speed: f64,
    pub travel_speed: f64,
    pub dia: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retraction_speed: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retraction_distance: Option<f64>,
}

impl Default for ResolveParams {
    fn default() -> Self {
        // the "generic" printer's defaults.
        ResolveParams {
            print_speed: 1000.0,
            travel_speed: 8000.0,
            dia: 1.75,
            retraction_speed: None,
            retraction_distance: None,
        }
    }
}

fn require_finite(name: &str, value: f64) -> Result<(), ResolveError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ResolveError::new(format!(
            "{name} must be finite, got {value}"
        )))
    }
}

fn require_positive(name: &str, value: f64) -> Result<(), ResolveError> {
    require_finite(name, value)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(ResolveError::new(format!(
            "{name} must be > 0, got {value}"
        )))
    }
}

fn require_non_negative(name: &str, value: f64) -> Result<(), ResolveError> {
    require_finite(name, value)?;
    if value >= 0.0 {
        Ok(())
    } else {
        Err(ResolveError::new(format!(
            "{name} must be >= 0, got {value}"
        )))
    }
}

fn require_unit_interval(name: &str, value: f64) -> Result<(), ResolveError> {
    require_finite(name, value)?;
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(ResolveError::new(format!(
            "{name} must be in the range 0..1, got {value}"
        )))
    }
}

fn require_optional_finite(name: &str, value: Option<f64>) -> Result<(), ResolveError> {
    if let Some(v) = value {
        require_finite(name, v)?;
    }
    Ok(())
}

/// Validate the lowering inputs before materializing L2 motion.
pub fn validate_design(design: &Design, p: &ResolveParams) -> Result<(), ResolveError> {
    require_positive("resolve_params.print_speed", p.print_speed)?;
    require_positive("resolve_params.travel_speed", p.travel_speed)?;
    require_positive("resolve_params.dia", p.dia)?;
    // A finite, positive diameter is not enough: the bead cross-section `π·(dia/2)²` underflows to
    // zero below `dia ≈ 4e-162` and overflows above `dia ≈ 1.5e154`, and every extruding op divides
    // by it (`filament = volume / area`). Zero made `Op::Deposit` yield `Length(inf)` — and
    // `Length(NaN)` for a travel, whose zero volume becomes `0.0 / 0.0`.
    let area = bead_area(p.dia);
    if !(area.value().is_finite() && area.value() > 0.0) {
        // `{:e}` for the same reason `gcode/lift.rs` uses it: `{}` never switches to exponent
        // notation, so the subnormal `dia` that reaches this arm prints its full decimal expansion
        // (`5e-324` gives a 407-character message).
        return Err(ResolveError::new(format!(
            "resolve_params.dia must give a finite non-zero bead cross-section, got {:e} for dia \
             {:e}",
            area.value(),
            p.dia
        )));
    }
    // The per-op `Retract`/`Unretract` distance and speed are checked positive below; these are the
    // fallbacks those ops use when they carry none, so the same guard has to apply here or it is
    // bypassed by omitting the field. A negative distance made `filament: Length::mm(-dist)`
    // *positive*, which `verify` reads as an unretract — the retraction limits then never applied.
    if let Some(distance) = p.retraction_distance {
        require_positive("resolve_params.retraction_distance", distance)?;
    }
    if let Some(speed) = p.retraction_speed {
        require_positive("resolve_params.retraction_speed", speed)?;
    }

    for (idx, op) in design.ops.iter().enumerate() {
        let prefix = |field: &str| format!("ops[{idx}].{field}");
        match op {
            Op::Geometry { width, height } => {
                if let Some(w) = width {
                    require_positive(&prefix("width"), *w)?;
                }
                if let Some(h) = height {
                    require_positive(&prefix("height"), *h)?;
                }
            }
            Op::Extruder { .. } | Op::Tool { .. } => {}
            Op::Speed { print } => require_positive(&prefix("print"), *print)?,
            Op::Temperature { nozzle } => require_non_negative(&prefix("nozzle"), *nozzle)?,
            Op::Fan { speed } => require_unit_interval(&prefix("speed"), *speed)?,
            Op::Flow { ratio } => require_positive(&prefix("ratio"), *ratio)?,
            // Finite and >= 0, the same domain `Op::Temperature` gets, and for the same reason: the
            // channel carries a commanded machine setpoint whose *ceiling* is a machine contract,
            // not a property of the IR. A negative `S` is meaningless on every controller Dry emits
            // for — GRBL and RS-274 both reject it — so it is refused here. Zero is legal and load-
            // bearing: `S0` is how a program commands the laser/spindle off without leaving the
            // channel unset, and refusing it would make "turn it off" inexpressible. There is
            // deliberately no upper bound: the real ceiling is GRBL's `$30` / the spindle's max RPM,
            // which lives in the machine profile (follow-up), and inventing a constant here would
            // either clamp a valid program (ADR 0002 §4 forbids clamping) or refuse one.
            Op::Power { level } => require_non_negative(&prefix("level"), *level)?,
            Op::Orient { i, j, k } => {
                require_finite(&prefix("i"), *i)?;
                require_finite(&prefix("j"), *j)?;
                require_finite(&prefix("k"), *k)?;
                let mag = libm::sqrt(i * i + j * j + k * k);
                if mag <= 0.0 {
                    return Err(ResolveError::new(format!(
                        "ops[{idx}].orient vector must have non-zero magnitude"
                    )));
                }
            }
            Op::Dwell { seconds } => require_non_negative(&prefix("seconds"), *seconds)?,
            Op::Move { x, y, z } => {
                require_optional_finite(&prefix("x"), *x)?;
                require_optional_finite(&prefix("y"), *y)?;
                require_optional_finite(&prefix("z"), *z)?;
            }
            Op::Arc {
                cx, cy, x, y, z, ..
            } => {
                require_finite(&prefix("cx"), *cx)?;
                require_finite(&prefix("cy"), *cy)?;
                require_optional_finite(&prefix("x"), *x)?;
                require_optional_finite(&prefix("y"), *y)?;
                require_optional_finite(&prefix("z"), *z)?;
            }
            Op::Spline { points } => {
                for (point_idx, point) in points.iter().enumerate() {
                    require_optional_finite(
                        &format!("ops[{idx}].points[{point_idx}][0]"),
                        point[0],
                    )?;
                    require_optional_finite(
                        &format!("ops[{idx}].points[{point_idx}][1]"),
                        point[1],
                    )?;
                    require_optional_finite(
                        &format!("ops[{idx}].points[{point_idx}][2]"),
                        point[2],
                    )?;
                }
            }
            Op::Clothoid {
                corner_x,
                corner_y,
                x,
                y,
                z,
                blend,
            } => {
                require_finite(&prefix("corner_x"), *corner_x)?;
                require_finite(&prefix("corner_y"), *corner_y)?;
                require_optional_finite(&prefix("x"), *x)?;
                require_optional_finite(&prefix("y"), *y)?;
                require_optional_finite(&prefix("z"), *z)?;
                require_positive(&prefix("blend"), *blend)?;
            }
            Op::ManualGcode { text } => {
                if text.is_empty() {
                    return Err(ResolveError::new(format!(
                        "ops[{idx}].manual_gcode text must not be empty"
                    )));
                }
            }
            Op::Retract { distance, speed } => {
                if let Some(d) = distance {
                    require_positive(&prefix("distance"), *d)?;
                }
                if let Some(s) = speed {
                    require_positive(&prefix("speed"), *s)?;
                }
            }
            Op::Unretract { distance, speed } => {
                if let Some(d) = distance {
                    require_positive(&prefix("distance"), *d)?;
                }
                if let Some(s) = speed {
                    require_positive(&prefix("speed"), *s)?;
                }
            }
            Op::Deposit { volume, speed } => {
                require_positive(&prefix("volume"), *volume)?;
                require_positive(&prefix("speed"), *speed)?;
            }
        }
    }
    validate_design_geometry(design)
}

fn validate_design_geometry(design: &Design) -> Result<(), ResolveError> {
    let mut pos: [Option<f64>; 3] = [None, None, None];
    for (idx, op) in design.ops.iter().enumerate() {
        match op {
            Op::Move { x, y, z } => {
                pos = [(*x).or(pos[0]), (*y).or(pos[1]), (*z).or(pos[2])];
            }
            Op::Arc {
                cx, cy, x, y, z, ..
            } => {
                let start_x = pos[0].unwrap_or(0.0);
                let start_y = pos[1].unwrap_or(0.0);
                let end = [(*x).or(pos[0]), (*y).or(pos[1]), (*z).or(pos[2])];
                let end_x = end[0].unwrap_or(start_x);
                let end_y = end[1].unwrap_or(start_y);
                let start_radius = libm::hypot(start_x - cx, start_y - cy);
                let end_radius = libm::hypot(end_x - cx, end_y - cy);
                if start_radius <= 0.0 || end_radius <= 0.0 {
                    return Err(ResolveError::new(format!(
                        "ops[{idx}].arc must have a non-zero radius"
                    )));
                }
                let tolerance = ARC_RADIUS_TOLERANCE_MM * start_radius.max(end_radius).max(1.0);
                let delta = (start_radius - end_radius).abs();
                if delta > tolerance {
                    return Err(ResolveError::new(format!(
                        "ops[{idx}].arc endpoint radius differs from start radius by {delta:.6} mm"
                    )));
                }
                pos = end;
            }
            Op::Clothoid {
                corner_x,
                corner_y,
                x,
                y,
                z,
                blend,
            } => {
                let start = [pos[0].unwrap_or(0.0), pos[1].unwrap_or(0.0)];
                let corner = [*corner_x, *corner_y];
                let end = [(*x).or(pos[0]), (*y).or(pos[1]), (*z).or(pos[2])];
                // The same solve `resolve_unchecked` runs, on the same numbers, so a corner that
                // lowers is exactly a corner that validates — the two cannot disagree about whether
                // the blend fits.
                crate::clothoid::corner_blend(
                    start,
                    corner,
                    [end[0].unwrap_or(start[0]), end[1].unwrap_or(start[1])],
                    *blend,
                )
                .map_err(|error| ResolveError::new(format!("ops[{idx}].clothoid: {error}")))?;
                pos = end;
            }
            Op::Spline { points } => {
                let mut running = [
                    pos[0].unwrap_or(0.0),
                    pos[1].unwrap_or(0.0),
                    pos[2].unwrap_or(0.0),
                ];
                for point in points {
                    running = [
                        point[0].unwrap_or(running[0]),
                        point[1].unwrap_or(running[1]),
                        point[2].unwrap_or(running[2]),
                    ];
                }
                if !points.is_empty() {
                    pos = [Some(running[0]), Some(running[1]), Some(running[2])];
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn dist(a: [Option<Length>; 3], b: [Option<Length>; 3]) -> Length {
    let mut sq = Area::ZERO;
    for i in 0..3 {
        if let (Some(p), Some(q)) = (a[i], b[i]) {
            let d = q - p;
            sq = sq + d * d;
        }
    }
    // A sum of squares is never negative, so the root exists for every finite input. A NaN
    // coordinate (refused by `validate_design`, and again by the emit gate) keeps propagating as
    // it did before `Area::sqrt` was made total, rather than collapsing to a plausible zero.
    sq.sqrt().unwrap_or(Length(f64::NAN))
}

/// Uniform Catmull-Rom interpolation of the span `p1 → p2` (phantom neighbours `p0`, `p3`) at
/// parameter `t ∈ [0, 1]`. The curve passes through its control points: `t = 0 ⇒ p1`, `t = 1 ⇒ p2`
/// (the basis is `[0,1,0,0]` at the endpoints), so span boundaries land exactly on the through-points.
pub fn catmull_rom(p0: [f64; 3], p1: [f64; 3], p2: [f64; 3], p3: [f64; 3], t: f64) -> [f64; 3] {
    let t2 = t * t;
    let t3 = t2 * t;
    let mut out = [0.0; 3];
    for a in 0..3 {
        out[a] = 0.5
            * ((2.0 * p1[a])
                + (-p0[a] + p2[a]) * t
                + (2.0 * p0[a] - 5.0 * p1[a] + 4.0 * p2[a] - p3[a]) * t2
                + (-p0[a] + 3.0 * p1[a] - 3.0 * p2[a] + p3[a]) * t3);
    }
    out
}

/// Lower an L1 design to an L2 toolpath after validating design and machine/material parameters.
pub fn resolve_checked(design: &Design, p: &ResolveParams) -> Result<Toolpath, ResolveError> {
    validate_design(design, p)?;
    let toolpath = resolve_unchecked(design, p);
    require_finite_toolpath(&toolpath)?;
    Ok(toolpath)
}

/// Every quantity the lowering *computes* must be finite — not merely every number it was handed.
///
/// `validate_design` bounds its inputs with `is_finite`, and that does not survive the arithmetic:
/// `dist` squares its deltas, so two ops 1e200 apart give `Length(inf)` from schema-valid JSON
/// (`Area::sqrt` returns `Some` for `inf`, since `inf >= 0.0`). That is the same seam H1.2 closed in
/// `gcode/lift.rs` — a gate on the parsed value does not establish an invariant on the constructed
/// one — and `resolve_*` is caller JSON on wasm and PyO3, so it is checked here as a postcondition
/// rather than argued site by site. Checking the produced IR once is total: it holds however the
/// lowering computes, including ops added later.
fn require_finite_toolpath(toolpath: &Toolpath) -> Result<(), ResolveError> {
    for (idx, s) in toolpath.segments.iter().enumerate() {
        let check = |name: &str, value: f64| -> Result<(), ResolveError> {
            if value.is_finite() {
                Ok(())
            } else {
                Err(ResolveError::new(format!(
                    "segments[{idx}].{name} resolved to {value}; the design is within range but the \
                     lowering is not"
                )))
            }
        };
        for (axis, name) in ["x", "y", "z"].iter().enumerate() {
            if let Some(v) = s.start[axis] {
                check(&format!("start.{name}"), v.value())?;
            }
            if let Some(v) = s.end[axis] {
                check(&format!("end.{name}"), v.value())?;
            }
        }
        check("speed", s.speed.value())?;
        check("length", s.length.value())?;
        check("volume", s.volume.value())?;
        check("filament", s.filament.value())?;
        for (name, value) in [
            ("width", s.width.map(|v| v.value())),
            ("height", s.height.map(|v| v.value())),
            ("temperature", s.temperature),
            ("fan", s.fan),
            ("flow", s.flow),
            ("power", s.power),
            ("dwell_s", s.dwell_s),
        ] {
            if let Some(value) = value {
                check(name, value)?;
            }
        }
        if let Some(centre) = s.centre {
            check("centre.x", centre[0].value())?;
            check("centre.y", centre[1].value())?;
        }
        if let Some(orientation) = s.orientation {
            for (axis, name) in ["i", "j", "k"].iter().enumerate() {
                check(&format!("orientation.{name}"), orientation[axis])?;
            }
        }
        if let Some(points) = &s.control_points {
            for (point_idx, point) in points.iter().enumerate() {
                for (axis, name) in ["x", "y", "z"].iter().enumerate() {
                    check(
                        &format!("control_points[{point_idx}].{name}"),
                        point[axis].value(),
                    )?;
                }
            }
        }
    }
    Ok(())
}

/// Lower an L1 design to an L2 toolpath.
///
/// This compatibility wrapper panics on invalid inputs. Bindings and other user-facing boundaries
/// should call [`resolve_checked`] so they can return a structured error.
pub fn resolve(design: &Design, p: &ResolveParams) -> Toolpath {
    resolve_checked(design, p).expect("valid Dry resolve inputs")
}

/// The bead cross-section of the round filament: `π·(dia/2)²`.
fn bead_area(dia: f64) -> Area {
    let half = Length::mm(dia) / 2.0;
    std::f64::consts::PI * (half * half)
}

fn resolve_unchecked(design: &Design, p: &ResolveParams) -> Toolpath {
    let area = bead_area(p.dia);
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
    let mut power: Option<f64> = None;
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
                if let Some(w_val) = w {
                    width = Length::mm(w_val);
                }
                if let Some(h_val) = h {
                    height = Length::mm(h_val);
                }
            }
            Op::Extruder { on: o } => on = o,
            Op::Speed { print: s } => print = Feedrate(s),
            Op::Temperature { nozzle } => temp = Some(nozzle),
            Op::Fan { speed } => fan = Some(speed),
            Op::Flow { ratio } => flow = ratio,
            Op::Tool { index } => tool = Some(index),
            Op::Power { level } => power = Some(level),
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
                kind: SegmentKind::Dwell,
                centre: None,
                clockwise: false,
                temperature: temp,
                fan,
                flow: None,
                tool,
                power,
                dwell_s: Some(seconds),
                manual_gcode: None,
                orientation,
                control_points: None,
            }),
            Op::Move { x, y, z } => {
                let end = [
                    x.map(Length::mm).or(pos[0]),
                    y.map(Length::mm).or(pos[1]),
                    z.map(Length::mm).or(pos[2]),
                ];
                if end != pos {
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
                        kind: SegmentKind::Line,
                        centre: None,
                        clockwise: false,
                        temperature: temp,
                        fan,
                        flow: flow_field,
                        tool,
                        power,
                        dwell_s: None,
                        manual_gcode: None,
                        orientation,
                        control_points: None,
                    });
                    pos = end;
                }
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
                    kind: SegmentKind::Arc,
                    centre: Some([cx, cy]),
                    clockwise,
                    temperature: temp,
                    fan,
                    flow: flow_field,
                    tool,
                    power,
                    dwell_s: None,
                    manual_gcode: None,
                    orientation,
                    control_points: None,
                });
                pos = end;
            }
            Op::Clothoid {
                corner_x,
                corner_y,
                x,
                y,
                z,
                blend,
            } => {
                let start = [
                    pos[0].map(|l| l.value()).unwrap_or(0.0),
                    pos[1].map(|l| l.value()).unwrap_or(0.0),
                ];
                let corner = [corner_x, corner_y];
                let end = [
                    x.map(Length::mm).or(pos[0]),
                    y.map(Length::mm).or(pos[1]),
                    z.map(Length::mm).or(pos[2]),
                ];
                let end_xy = [
                    end[0].map(|l| l.value()).unwrap_or(start[0]),
                    end[1].map(|l| l.value()).unwrap_or(start[1]),
                ];
                // `validate_design_geometry` ran this exact call on these exact numbers and refused
                // the design if it failed, and `resolve_unchecked` is private and only reachable
                // through `resolve_checked`, which validates first. Same reasoning as the public
                // `resolve` wrapper's own `expect`.
                let solved = crate::clothoid::corner_blend(start, corner, end_xy, blend)
                    .expect("clothoid corner validated by validate_design_geometry");

                // The XY polyline the op walks: onto the incoming leg, through the blend, out along
                // the outgoing leg. Z is then distributed over it linearly in XY arc length — the
                // same convention `Op::Arc` uses for a helical rise.
                let mut xy: Vec<[f64; 2]> = Vec::with_capacity(solved.points.len() + 2);
                xy.push(solved.enter);
                xy.extend_from_slice(&solved.points);
                xy.push(end_xy);
                let mut travelled = Vec::with_capacity(xy.len());
                let mut cumulative = 0.0;
                let mut previous = start;
                for point in &xy {
                    cumulative += libm::hypot(point[0] - previous[0], point[1] - previous[1]);
                    travelled.push(cumulative);
                    previous = *point;
                }
                let total = cumulative;
                let last = xy.len() - 1;

                let mut running = pos;
                for (index, (point, distance)) in xy.iter().zip(travelled.iter()).enumerate() {
                    // A zero-length step (an empty leg when the blend consumes a whole leg, or a
                    // repeated sample) is dropped rather than emitted, matching `Op::Move`.
                    let next = if index == last {
                        // The last point is the commanded end, used verbatim: `from + (to - from)*1`
                        // is not exactly `to` in binary64, and a corner blend must land on the point
                        // it was given, not near it.
                        end
                    } else {
                        let height_z = match (pos[2], end[2]) {
                            (Some(from), Some(to)) if total > 0.0 => {
                                Some(from + (to - from) * (distance / total))
                            }
                            _ => end[2],
                        };
                        [
                            Some(Length::mm(point[0])),
                            Some(Length::mm(point[1])),
                            height_z,
                        ]
                    };
                    if next == running {
                        continue;
                    }
                    let length = dist(running, next);
                    let volume = if on {
                        length * width * height * flow
                    } else {
                        Volume::ZERO
                    };
                    segs.push(Segment {
                        start: running,
                        end: next,
                        travel: !on,
                        speed: if on { print } else { travel_speed },
                        length,
                        volume,
                        filament: volume / area,
                        width: Some(width),
                        height: Some(height),
                        kind: SegmentKind::Line,
                        centre: None,
                        clockwise: false,
                        temperature: temp,
                        fan,
                        flow: flow_field,
                        tool,
                        power,
                        dwell_s: None,
                        manual_gcode: None,
                        orientation,
                        control_points: None,
                    });
                    running = next;
                }
                pos = running;
            }
            Op::Spline { ref points } => {
                // Build the through-point sequence as raw f64 triples, resolving each `None` axis from
                // the running position (like `Move`). P0 is the current running position.
                let cur = [
                    pos[0].map(|l| l.value()).unwrap_or(0.0),
                    pos[1].map(|l| l.value()).unwrap_or(0.0),
                    pos[2].map(|l| l.value()).unwrap_or(0.0),
                ];
                // through[0] = P0 (current pos); through[1..] = the resolved control points.
                let mut through: Vec<[f64; 3]> = Vec::with_capacity(points.len() + 1);
                through.push(cur);
                let mut running = cur;
                let mut control_points = Vec::with_capacity(points.len());
                for p in points {
                    let resolved = [
                        p[0].unwrap_or(running[0]),
                        p[1].unwrap_or(running[1]),
                        p[2].unwrap_or(running[2]),
                    ];
                    through.push(resolved);
                    control_points.push([
                        Length::mm(resolved[0]),
                        Length::mm(resolved[1]),
                        Length::mm(resolved[2]),
                    ]);
                    running = resolved;
                }

                // Compute the total length, volume, and filament by sampling the spline
                let n = through.len();
                let mut total_length = Length::ZERO;
                let mut temp_pos = pos;
                for i in 0..n - 1 {
                    let p0 = through[i.saturating_sub(1)];
                    let p1 = through[i];
                    let p2 = through[i + 1];
                    let p3 = through[(i + 2).min(n - 1)];
                    for step in 1..=SAMPLES {
                        let pt = if step == SAMPLES {
                            p2
                        } else {
                            catmull_rom(p0, p1, p2, p3, step as f64 / SAMPLES as f64)
                        };
                        let end = [
                            Some(Length::mm(pt[0])),
                            Some(Length::mm(pt[1])),
                            Some(Length::mm(pt[2])),
                        ];
                        total_length = total_length + dist(temp_pos, end);
                        temp_pos = end;
                    }
                }

                let volume = if on {
                    total_length * width * height * flow
                } else {
                    Volume::ZERO
                };
                let filament = volume / area;

                segs.push(Segment {
                    start: pos,
                    end: temp_pos,
                    travel: !on,
                    speed: if on { print } else { travel_speed },
                    length: total_length,
                    volume,
                    filament,
                    width: Some(width),
                    height: Some(height),
                    kind: SegmentKind::Spline,
                    centre: None,
                    clockwise: false,
                    temperature: temp,
                    fan,
                    flow: flow_field,
                    tool,
                    power,
                    dwell_s: None,
                    manual_gcode: None,
                    orientation,
                    control_points: Some(control_points),
                });
                pos = temp_pos;
            }
            Op::ManualGcode { ref text } => segs.push(Segment {
                start: pos,
                end: pos,
                travel: true,
                speed: Feedrate::ZERO,
                length: Length::ZERO,
                volume: Volume::ZERO,
                filament: Length::ZERO,
                width: None,
                height: None,
                kind: SegmentKind::ManualGcode,
                centre: None,
                clockwise: false,
                temperature: temp,
                fan,
                flow: None,
                tool,
                power,
                dwell_s: None,
                manual_gcode: Some(text.clone()),
                orientation,
                control_points: None,
            }),
            Op::Retract { distance, speed } => {
                let dist = distance.or(p.retraction_distance).unwrap_or(1.0);
                let sp = speed.or(p.retraction_speed).unwrap_or(1000.0);
                segs.push(Segment {
                    start: pos,
                    end: pos,
                    travel: true,
                    speed: Feedrate(sp),
                    length: Length::ZERO,
                    volume: Volume::ZERO,
                    filament: Length::mm(-dist),
                    width: None,
                    height: None,
                    kind: SegmentKind::Retract,
                    centre: None,
                    clockwise: false,
                    temperature: temp,
                    fan,
                    flow: None,
                    tool,
                    power,
                    dwell_s: None,
                    manual_gcode: None,
                    orientation,
                    control_points: None,
                });
            }
            Op::Unretract { distance, speed } => {
                let dist = distance.or(p.retraction_distance).unwrap_or(1.0);
                let sp = speed.or(p.retraction_speed).unwrap_or(1000.0);
                segs.push(Segment {
                    start: pos,
                    end: pos,
                    travel: true,
                    speed: Feedrate(sp),
                    length: Length::ZERO,
                    volume: Volume::ZERO,
                    filament: Length::mm(dist),
                    width: None,
                    height: None,
                    kind: SegmentKind::Unretract,
                    centre: None,
                    clockwise: false,
                    temperature: temp,
                    fan,
                    flow: None,
                    tool,
                    power,
                    dwell_s: None,
                    manual_gcode: None,
                    orientation,
                    control_points: None,
                });
            }
            Op::Deposit { volume, speed } => {
                let vol = Volume(volume);
                let fil = vol / area;
                segs.push(Segment {
                    start: pos,
                    end: pos,
                    travel: false,
                    speed: Feedrate(speed),
                    length: Length::ZERO,
                    volume: vol,
                    filament: fil,
                    width: None,
                    height: None,
                    kind: SegmentKind::Deposit,
                    centre: None,
                    clockwise: false,
                    temperature: temp,
                    fan,
                    flow: None,
                    tool,
                    power,
                    dwell_s: None,
                    manual_gcode: None,
                    orientation,
                    control_points: None,
                });
            }
        }
    }
    Toolpath {
        version: 0,
        meta: None,
        segments: segs,
    }
}

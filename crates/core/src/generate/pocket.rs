//! `pocket` — contour-parallel CNC pocket/profile generator (P5.3, spec
//! `docs/superpowers/specs/2026-07-30-cnc-pocket-profile-design.md`).
//!
//! Pure L1 sugar like [`super::tpms`]: validated options → `Vec<Op>`; resolve/verify/
//! simulate/emit are inherited unchanged.

use crate::resolve::{Design, Op};

/// Maximum total passes before rejecting as pathological input. No legitimate job reaches this; it
/// gates runaway pass counts from tiny step sizes. What binds differs by mode: a Pocket walks the
/// ring series inward, so its bound is depth passes × rings, while a Profile cuts one contour per
/// depth level, so depth passes alone bound it. The `total_passes` product check is therefore
/// *unreachable* in Profile mode — the depth-pass gate has already rejected anything that could
/// exceed it — and is kept only as the shared backstop for the Pocket path.
const MAX_TOTAL_PASSES: u32 = 100_000;

/// Smallest cut extent that survives emission, applied to the radius-shaped quantity of each shape:
/// a circle's wall-inset cut radius, and a rectangle's half-extents. `emit` rounds coordinates to 6
/// decimals, so a circle ring below this collapses to an arc with identical start/end and `I0 J0` —
/// a zero-radius arc real controllers reject — and a rectangle with both half-extents below it
/// collapses to a single point. Because the comparison is against a *half* extent, the smallest
/// full span that survives is 2e-5 mm: roughly 2× stricter than the rounding grid alone demands,
/// deliberately, since anything near that size is a mistake in the input rather than a cut.
const MIN_CUT_RADIUS: f64 = 1e-5;

/// Largest ring-to-ring inset that still clears a rectangle's corners. A ring's swath ends in a
/// *sharp* inner corner `tool_r` inside the ring, while the ring inward of it only reaches out to
/// that corner through a `tool_r` fillet; an inset above `tool_r·(1 + 1/√2)` therefore leaves an
/// uncut cusp in each of the three corners the ring-to-ring link move does not cross. Requested
/// stepovers above ≈0.854 are clamped to this — never rejected, and never silently *larger* than
/// asked for. Concentric circles need no such clamp: their swaths are annuli that overlap for any
/// inset ≤ the tool diameter.
fn rect_inset(step: f64, tool_r: f64) -> f64 {
    step.min(tool_r * (1.0 + std::f64::consts::FRAC_1_SQRT_2))
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "shape", rename_all = "camelCase")]
pub enum PocketShape {
    #[serde(rename = "rect")]
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    #[serde(rename = "circle")]
    Circle { cx: f64, cy: f64, radius: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CutMode {
    #[default]
    Pocket,
    Profile,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PocketOptions {
    #[serde(flatten)]
    pub shape: PocketShape,
    #[serde(default)]
    pub mode: CutMode,
    pub tool_diameter: f64,
    /// Ring-to-ring inset as a fraction of `tool_diameter`, in `(0, 1]` (default 0.5). For
    /// rectangular pockets the resulting inset is clamped to the largest corner-clearing value
    /// (`tool_r · (1 + 1/√2)`, ≈ 0.854 of the diameter) — see [`rect_inset`].
    pub stepover: Option<f64>,
    pub depth: f64,
    pub depth_per_pass: Option<f64>,
    pub z_top: Option<f64>,
    pub safe_z: Option<f64>,
    pub cut_feed: Option<f64>,
    pub plunge_feed: Option<f64>,
    #[serde(default)]
    pub helical_entry: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PocketError {
    pub message: String,
}

impl PocketError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PocketError {}

#[derive(Debug)]
struct Resolved {
    tool_r: f64,
    step: f64,
    depth: f64,
    depth_per_pass: f64,
    z_top: f64,
    safe_z: f64,
    cut_feed: f64,
    plunge_feed: f64,
    helical_entry: bool,
}

fn positive(name: &str, v: f64) -> Result<f64, PocketError> {
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(PocketError::new(format!(
            "{name} must be finite and > 0, got {v}"
        )))
    }
}

fn finite(name: &str, v: f64) -> Result<f64, PocketError> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(PocketError::new(format!("{name} must be finite")))
    }
}

fn validate(o: &PocketOptions) -> Result<Resolved, PocketError> {
    let d = positive("tool_diameter", o.tool_diameter)?;
    let stepover = o.stepover.unwrap_or(0.5);
    if !(stepover.is_finite() && stepover > 0.0 && stepover <= 1.0) {
        return Err(PocketError::new(format!(
            "stepover must be in (0, 1] (fraction of tool_diameter), got {stepover}"
        )));
    }
    let depth = positive("depth", o.depth)?;
    let depth_per_pass = positive("depth_per_pass", o.depth_per_pass.unwrap_or(depth))?;
    let z_top = finite("z_top", o.z_top.unwrap_or(0.0))?;
    let safe_z = finite("safe_z", o.safe_z.unwrap_or(z_top + 5.0))?;
    if safe_z <= z_top {
        return Err(PocketError::new(format!(
            "safe_z ({safe_z}) must be above z_top ({z_top})"
        )));
    }
    let cut_feed = positive("cut_feed", o.cut_feed.unwrap_or(300.0))?;
    let plunge_feed = positive("plunge_feed", o.plunge_feed.unwrap_or(cut_feed / 3.0))?;

    // Compute depth pass count and reject if pathological.
    let depth_passes = (depth / depth_per_pass).ceil() as u32;
    if depth_passes > MAX_TOTAL_PASSES {
        return Err(PocketError::new(format!(
            "depth_per_pass ({depth_per_pass}) too small relative to depth ({depth}): \
             would require {depth_passes} passes (max {MAX_TOTAL_PASSES})"
        )));
    }

    match o.shape {
        PocketShape::Rect {
            x,
            y,
            width,
            height,
        } => {
            finite("x", x)?;
            finite("y", y)?;
            positive("width", width)?;
            positive("height", height)?;
            if d > width || d > height {
                return Err(PocketError::new(format!(
                    "tool_diameter ({d}) does not fit the {width}x{height} rectangle"
                )));
            }
            let tool_r = d / 2.0;
            let hw = width / 2.0 - tool_r;
            let hh = height / 2.0 - tool_r;
            // Both half-extents below the emission resolution means the whole cutting region
            // rounds away: `rect_rings` yields no pass at all (or one that collapses to a point),
            // so the program would plunge and retract without cutting. One axis at tool size is
            // the legitimate slot case — the centre `Line` pass cuts it — so only reject when
            // BOTH collapse.
            if hw < MIN_CUT_RADIUS && hh < MIN_CUT_RADIUS {
                return Err(PocketError::new(format!(
                    "tool_diameter ({d}) leaves no machinable cutting region in the \
                     {width}x{height} rectangle: both extents are within the {MIN_CUT_RADIUS} mm \
                     emission resolution of the tool diameter (cut half-extents {hw}x{hh})"
                )));
            }
            // Profile cuts one contour per depth level whatever the outline measures; only Pocket
            // walks the ring series inward, so only Pocket needs the ring estimate.
            let ring_count = match o.mode {
                CutMode::Profile => 1,
                CutMode::Pocket => {
                    let step = rect_inset(stepover * d, tool_r);
                    let smaller = hw.min(hh);
                    // +2 for the final line pass and the possible extra innermost ring
                    let ring_count_f = (smaller / step).ceil() + 2.0;
                    if ring_count_f > MAX_TOTAL_PASSES as f64 {
                        return Err(PocketError::new(format!(
                            "tool_diameter ({d}) too small relative to pocket dimensions ({width}x{height}): \
                             would require ~{ring_count_f:.0} rings (max {MAX_TOTAL_PASSES} total passes)"
                        )));
                    }
                    ring_count_f as u32
                }
            };
            let total_passes = depth_passes.saturating_mul(ring_count);
            if total_passes > MAX_TOTAL_PASSES {
                return Err(PocketError::new(format!(
                    "tool_diameter ({d}) too small relative to pocket dimensions ({width}x{height}): \
                     would require {ring_count} rings × {depth_passes} depth passes = {total_passes} total passes (max {MAX_TOTAL_PASSES})"
                )));
            }
        }
        PocketShape::Circle { cx, cy, radius } => {
            finite("cx", cx)?;
            finite("cy", cy)?;
            positive("radius", radius)?;
            if d > 2.0 * radius {
                return Err(PocketError::new(format!(
                    "tool_diameter ({d}) does not fit the radius-{radius} circle"
                )));
            }
            let tool_r = d / 2.0;
            let outer_r = radius - tool_r;
            if outer_r < MIN_CUT_RADIUS {
                return Err(PocketError::new(format!(
                    "tool_diameter ({d}) leaves a cut radius of {outer_r} on the radius-{radius} \
                     circle: below the {MIN_CUT_RADIUS} mm emission resolution, so the ring would \
                     emit as a zero-radius arc"
                )));
            }
            // As for the rectangle: Profile is one contour per depth level. In Pocket mode
            // `circle_radii` pushes one radius per `step` from the wall-inset outer radius down to
            // (but not including) zero, plus one centre-clearing ring when the innermost one does
            // not reach the centre.
            let ring_count = match o.mode {
                CutMode::Profile => 1,
                CutMode::Pocket => {
                    let step = stepover * d;
                    let ring_count_f = (outer_r / step).ceil().max(1.0) + 1.0;
                    if ring_count_f > MAX_TOTAL_PASSES as f64 {
                        return Err(PocketError::new(format!(
                            "tool_diameter ({d}) too small relative to circle radius ({radius}): \
                             would require ~{ring_count_f:.0} rings (max {MAX_TOTAL_PASSES} total passes)"
                        )));
                    }
                    ring_count_f as u32
                }
            };
            let total_passes = depth_passes.saturating_mul(ring_count);
            if total_passes > MAX_TOTAL_PASSES {
                return Err(PocketError::new(format!(
                    "tool_diameter ({d}) too small relative to circle radius ({radius}): \
                     would require {ring_count} rings × {depth_passes} depth passes = {total_passes} total passes (max {MAX_TOTAL_PASSES})"
                )));
            }
        }
    }
    Ok(Resolved {
        tool_r: d / 2.0,
        step: stepover * d,
        depth,
        depth_per_pass,
        z_top,
        safe_z,
        cut_feed,
        plunge_feed,
        helical_entry: o.helical_entry.unwrap_or(false),
    })
}

#[derive(Debug, Clone, PartialEq)]
enum RectPass {
    Ring { hw: f64, hh: f64 },
    Line { half_len: f64, along_x: bool },
}

fn depth_levels(r: &Resolved) -> Vec<f64> {
    let mut levels = Vec::new();
    let bottom = r.z_top - r.depth;
    let mut z = r.z_top;
    loop {
        z -= r.depth_per_pass;
        if z <= bottom + 1e-12 {
            levels.push(bottom);
            return levels;
        }
        levels.push(z);
    }
}

/// Contour-parallel rectangle passes, innermost first. `hw`/`hh` are the OUTERMOST
/// ring's half-extents (wall already inset by the tool radius).
fn rect_rings(hw: f64, hh: f64, step: f64, tool_r: f64) -> Vec<RectPass> {
    let inset = rect_inset(step, tool_r);
    let mut out = Vec::new(); // built outermost-first, reversed at the end
    let mut k = 0u32;
    loop {
        let (sw, sh) = (hw - k as f64 * inset, hh - k as f64 * inset);
        if sw > 0.0 && sh > 0.0 {
            out.push(RectPass::Ring { hw: sw, hh: sh });
            k += 1;
            continue;
        }
        // the smaller dimension collapsed: one center pass along the dominant axis
        let half_len = sw.max(sh);
        if half_len > 0.0 {
            out.push(RectPass::Line {
                half_len,
                along_x: sw >= sh,
            });
        }
        break;
    }
    // A ring's swath leaves an uncut rectangle of half-extents (hw − tool_r, hh − tool_r) inside
    // it. When the series ends on a ring whose smaller half-extent still exceeds `tool_r` — only
    // reachable for stepover > 0.5, since otherwise `inset ≤ tool_r` — that rectangle is a real
    // island. One more ring, shrunk so the smaller half-extent is exactly `tool_r`, clears it: its
    // own interior collapses to zero extent, and the extra inset is < `tool_r`, so it opens no new
    // gap against the ring outside it. A centre *line* pass already reaches the centre, so the
    // rescue is only needed when the innermost pass is a ring.
    if let Some(&RectPass::Ring { hw: sw, hh: sh }) = out.last() {
        let shrink = sw.min(sh) - tool_r;
        if shrink > 0.0 {
            out.push(RectPass::Ring {
                hw: sw - shrink,
                hh: sh - shrink,
            });
        }
    }
    out.reverse();
    out
}

fn rect_passes(cx: f64, cy: f64, rings: &[RectPass], r: &Resolved) -> Vec<Op> {
    let mut ops = Vec::new();
    let entry_xy = match rings.first() {
        Some(RectPass::Ring { hw, hh }) => (cx - hw, cy - hh),
        Some(RectPass::Line {
            half_len,
            along_x: true,
        }) => (cx - half_len, cy),
        Some(RectPass::Line {
            half_len,
            along_x: false,
        }) => (cx, cy - half_len),
        None => (cx, cy),
    };
    for &z in &depth_levels(r) {
        // rapid to entry above the work, then plunge
        ops.push(Op::Extruder { on: false });
        ops.push(Op::Move {
            x: None,
            y: None,
            z: Some(r.safe_z),
        });
        ops.push(Op::Move {
            x: Some(entry_xy.0),
            y: Some(entry_xy.1),
            z: Some(r.safe_z),
        });
        ops.push(Op::Speed {
            print: r.plunge_feed,
        });
        ops.push(Op::Extruder { on: true });
        if r.helical_entry {
            let r_helix = (r.tool_r * 0.5).min(2.0).max(0.1);
            let steps = 16;
            for s in 1..=steps {
                let frac = s as f64 / steps as f64;
                let angle = frac * 2.0 * std::f64::consts::PI;
                let hz = r.z_top - frac * (r.z_top - z);
                let hx = entry_xy.0 + r_helix * (angle.cos() - 1.0);
                let hy = entry_xy.1 + r_helix * angle.sin();
                ops.push(Op::Move {
                    x: Some(hx),
                    y: Some(hy),
                    z: Some(hz),
                });
            }
        } else {
            ops.push(Op::Move {
                x: None,
                y: None,
                z: Some(z),
            });
        }
        ops.push(Op::Speed { print: r.cut_feed });
        for pass in rings {
            match *pass {
                RectPass::Ring { hw, hh } => {
                    // link into the ring's start corner (a cutting stepover move), then 4 sides
                    ops.push(Op::Move {
                        x: Some(cx - hw),
                        y: Some(cy - hh),
                        z: None,
                    });
                    ops.push(Op::Move {
                        x: Some(cx + hw),
                        y: Some(cy - hh),
                        z: None,
                    });
                    ops.push(Op::Move {
                        x: Some(cx + hw),
                        y: Some(cy + hh),
                        z: None,
                    });
                    ops.push(Op::Move {
                        x: Some(cx - hw),
                        y: Some(cy + hh),
                        z: None,
                    });
                    ops.push(Op::Move {
                        x: Some(cx - hw),
                        y: Some(cy - hh),
                        z: None,
                    });
                }
                RectPass::Line { half_len, along_x } => {
                    let (ax, ay, bx, by) = if along_x {
                        (cx - half_len, cy, cx + half_len, cy)
                    } else {
                        (cx, cy - half_len, cx, cy + half_len)
                    };
                    ops.push(Op::Move {
                        x: Some(ax),
                        y: Some(ay),
                        z: None,
                    });
                    ops.push(Op::Move {
                        x: Some(bx),
                        y: Some(by),
                        z: None,
                    });
                    ops.push(Op::Move {
                        x: Some(ax),
                        y: Some(ay),
                        z: None,
                    });
                }
            }
        }
        ops.push(Op::Extruder { on: false });
        ops.push(Op::Move {
            x: None,
            y: None,
            z: Some(r.safe_z),
        });
    }
    ops
}

/// Contour-parallel circle cut radii, innermost first. `outer_r` is the wall-inset
/// (by tool radius) outermost cut radius.
fn circle_radii(outer_r: f64, step: f64, tool_r: f64) -> Vec<f64> {
    let mut radii = Vec::new();
    let mut r = outer_r;
    while r > 0.0 {
        radii.push(r);
        r -= step;
    }
    // The innermost ring's swath reaches the centre only when its radius is ≤ `tool_r`; above that
    // it leaves an uncut centre disc (only reachable for stepover > 0.5). One extra ring at exactly
    // `tool_r` closes it, and since `step ≤ tool_diameter` the ring outside it sits at ≤ 2·tool_r,
    // so the added ring opens no new annular gap.
    if radii.last().is_some_and(|&r| r > tool_r) {
        radii.push(tool_r);
    }
    radii.reverse();
    radii
}

fn circle_passes(cx: f64, cy: f64, radii: &[f64], r: &Resolved) -> Vec<Op> {
    let mut ops = Vec::new();
    let entry = radii.first().map(|ri| (cx - ri, cy)).unwrap_or((cx, cy));
    for &z in &depth_levels(r) {
        ops.push(Op::Extruder { on: false });
        ops.push(Op::Move {
            x: None,
            y: None,
            z: Some(r.safe_z),
        });
        ops.push(Op::Move {
            x: Some(entry.0),
            y: Some(entry.1),
            z: Some(r.safe_z),
        });
        ops.push(Op::Speed {
            print: r.plunge_feed,
        });
        ops.push(Op::Extruder { on: true });
        if r.helical_entry {
            let r_helix = (r.tool_r * 0.5).min(2.0).max(0.1);
            let steps = 16;
            for s in 1..=steps {
                let frac = s as f64 / steps as f64;
                let angle = frac * 2.0 * std::f64::consts::PI;
                let hz = r.z_top - frac * (r.z_top - z);
                let hx = entry.0 + r_helix * (angle.cos() - 1.0);
                let hy = entry.1 + r_helix * angle.sin();
                ops.push(Op::Move {
                    x: Some(hx),
                    y: Some(hy),
                    z: Some(hz),
                });
            }
        } else {
            ops.push(Op::Move {
                x: None,
                y: None,
                z: Some(z),
            });
        }
        ops.push(Op::Speed { print: r.cut_feed });
        for &ri in radii {
            // stepover link to the ring start, then two half circles (G2/G3 exercised)
            ops.push(Op::Move {
                x: Some(cx - ri),
                y: Some(cy),
                z: None,
            });
            ops.push(Op::Arc {
                cx,
                cy,
                x: Some(cx + ri),
                y: Some(cy),
                z: None,
                clockwise: false,
            });
            ops.push(Op::Arc {
                cx,
                cy,
                x: Some(cx - ri),
                y: Some(cy),
                z: None,
                clockwise: false,
            });
        }
        ops.push(Op::Extruder { on: false });
        ops.push(Op::Move {
            x: None,
            y: None,
            z: Some(r.safe_z),
        });
    }
    ops
}

/// Generate the L1 ops. Structured failure on invalid options, never a panic.
pub fn try_pocket_ops(o: &PocketOptions) -> Result<Vec<Op>, PocketError> {
    let r = validate(o)?;
    let mut ops = vec![
        Op::Geometry {
            width: Some(o.tool_diameter),
            height: Some(r.depth_per_pass),
        },
        Op::Extruder { on: false },
        Op::Speed { print: r.cut_feed },
    ];
    ops.extend(passes(o, &r)?);
    Ok(ops)
}

/// Generate L1 ops for a deep pocket broken down into stepped constant-Z passes.
pub fn pocket_stepped_ops(
    o: &PocketOptions,
    total_depth: f64,
    max_stepdown: f64,
) -> Result<Vec<Op>, PocketError> {
    let mut stepped_opts = o.clone();
    stepped_opts.depth = total_depth;
    stepped_opts.depth_per_pass = Some(max_stepdown);
    try_pocket_ops(&stepped_opts)
}

fn passes(o: &PocketOptions, r: &Resolved) -> Result<Vec<Op>, PocketError> {
    Ok(match (&o.shape, o.mode) {
        (
            PocketShape::Rect {
                x,
                y,
                width,
                height,
            },
            CutMode::Pocket,
        ) => {
            let (cx, cy) = (x + width / 2.0, y + height / 2.0);
            let rings = rect_rings(
                width / 2.0 - r.tool_r,
                height / 2.0 - r.tool_r,
                r.step,
                r.tool_r,
            );
            rect_passes(cx, cy, &rings, r)
        }
        (
            PocketShape::Rect {
                x,
                y,
                width,
                height,
            },
            CutMode::Profile,
        ) => {
            let (cx, cy) = (x + width / 2.0, y + height / 2.0);
            let ring = vec![RectPass::Ring {
                hw: width / 2.0 - r.tool_r,
                hh: height / 2.0 - r.tool_r,
            }];
            rect_passes(cx, cy, &ring, r)
        }
        (PocketShape::Circle { cx, cy, radius }, CutMode::Pocket) => circle_passes(
            *cx,
            *cy,
            &circle_radii(radius - r.tool_r, r.step, r.tool_r),
            r,
        ),
        (PocketShape::Circle { cx, cy, radius }, CutMode::Profile) => {
            circle_passes(*cx, *cy, &[radius - r.tool_r], r)
        }
    })
}

/// Panicking convenience over [`try_pocket_ops`]; precondition: valid Dry pocket options.
pub fn pocket_ops(o: &PocketOptions) -> Vec<Op> {
    try_pocket_ops(o).expect("valid Dry pocket options")
}

pub fn try_pocket_design(o: &PocketOptions) -> Result<Design, PocketError> {
    Ok(Design {
        ops: try_pocket_ops(o)?,
    })
}

pub fn pocket_design(o: &PocketOptions) -> Design {
    Design { ops: pocket_ops(o) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect_opts() -> PocketOptions {
        PocketOptions {
            shape: PocketShape::Rect {
                x: 0.0,
                y: 0.0,
                width: 60.0,
                height: 40.0,
            },
            mode: CutMode::Pocket,
            tool_diameter: 6.0,
            stepover: None,
            depth: 5.0,
            depth_per_pass: None,
            z_top: None,
            safe_z: None,
            cut_feed: None,
            plunge_feed: None,
            helical_entry: None,
        }
    }

    fn circle_opts() -> PocketOptions {
        PocketOptions {
            shape: PocketShape::Circle {
                cx: 10.0,
                cy: 10.0,
                radius: 15.0,
            },
            ..rect_opts()
        }
    }

    fn dist_point_segment(px: f64, py: f64, s: &crate::ir::Segment) -> f64 {
        let start_x = s.start[0].map(|l| l.value());
        let start_y = s.start[1].map(|l| l.value());
        let end_x = s.end[0].map(|l| l.value());
        let end_y = s.end[1].map(|l| l.value());

        match (start_x, start_y, end_x, end_y) {
            (Some(sx), Some(sy), Some(ex), Some(ey)) => {
                // Compute closest point on segment to (px, py)
                let dx = ex - sx;
                let dy = ey - sy;
                let len_sq = dx * dx + dy * dy;

                if len_sq == 0.0 {
                    // Degenerate segment: point-to-point
                    return ((px - sx).powi(2) + (py - sy).powi(2)).sqrt();
                }

                let t = ((px - sx) * dx + (py - sy) * dy) / len_sq;
                let t = t.clamp(0.0, 1.0);

                let closest_x = sx + t * dx;
                let closest_y = sy + t * dy;

                ((px - closest_x).powi(2) + (py - closest_y).powi(2)).sqrt()
            }
            _ => f64::INFINITY, // Undefined coordinates
        }
    }

    /// Exact distance from a point to an `Arc` segment (a chord approximation would run straight
    /// through a ring's centre and hide exactly the uncut-island class this file guards).
    fn dist_point_arc(px: f64, py: f64, s: &crate::ir::Segment) -> f64 {
        let c = match s.centre {
            Some(c) => [c[0].value(), c[1].value()],
            None => return f64::INFINITY,
        };
        let (sx, sy) = match (s.start[0], s.start[1]) {
            (Some(a), Some(b)) => (a.value(), b.value()),
            _ => return f64::INFINITY,
        };
        let (ex, ey) = match (s.end[0], s.end[1]) {
            (Some(a), Some(b)) => (a.value(), b.value()),
            _ => return f64::INFINITY,
        };
        let tau = std::f64::consts::TAU;
        let norm = |a: f64| {
            let m = a % tau;
            if m < 0.0 {
                m + tau
            } else {
                m
            }
        };
        let ang = |x: f64, y: f64| (y - c[1]).atan2(x - c[0]);
        let radius = ((sx - c[0]).powi(2) + (sy - c[1]).powi(2)).sqrt();
        let (a0, a1) = (ang(sx, sy), ang(ex, ey));
        // `start == end` is resolve's full-circle convention, so a zero sweep means TAU.
        let raw = if s.clockwise { a0 - a1 } else { a1 - a0 };
        let sweep = if norm(raw) == 0.0 { tau } else { norm(raw) };
        let delta = norm(if s.clockwise {
            a0 - ang(px, py)
        } else {
            ang(px, py) - a0
        });
        if delta <= sweep {
            (((px - c[0]).powi(2) + (py - c[1]).powi(2)).sqrt() - radius).abs()
        } else {
            let d0 = ((px - sx).powi(2) + (py - sy).powi(2)).sqrt();
            let d1 = ((px - ex).powi(2) + (py - ey).powi(2)).sqrt();
            d0.min(d1)
        }
    }

    fn dist_point_path(px: f64, py: f64, s: &crate::ir::Segment) -> f64 {
        if s.kind == crate::ir::SegmentKind::Arc && s.centre.is_some() {
            dist_point_arc(px, py, s)
        } else {
            dist_point_segment(px, py, s)
        }
    }

    /// Floor points the tool centre must reach: the shape inset by the tool radius, on a `grid`
    /// lattice (same construction as [`rect_pocket_ops_resolve_and_cover`], finer and shape-generic).
    fn interior_samples(shape: &PocketShape, tool_r: f64, grid: f64) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        match *shape {
            PocketShape::Rect {
                x,
                y,
                width,
                height,
            } => {
                let (nx, ny) = (
                    ((width - 2.0 * tool_r) / grid).floor() as i32,
                    ((height - 2.0 * tool_r) / grid).floor() as i32,
                );
                for gx in 0..=nx {
                    for gy in 0..=ny {
                        out.push((x + tool_r + gx as f64 * grid, y + tool_r + gy as f64 * grid));
                    }
                }
            }
            PocketShape::Circle { cx, cy, radius } => {
                let inner = radius - tool_r;
                let n = (2.0 * inner / grid).floor() as i32;
                for gx in 0..=n {
                    for gy in 0..=n {
                        let (px, py) =
                            (cx - inner + gx as f64 * grid, cy - inner + gy as f64 * grid);
                        if (px - cx).powi(2) + (py - cy).powi(2) <= inner * inner + 1e-9 {
                            out.push((px, py));
                        }
                    }
                }
            }
        }
        out
    }

    /// Spec §5.1 full-coverage property: across the whole documented stepover range, every floor
    /// point the tool centre can reach lies within `tool_r` of some cutting move. A point farther
    /// than `tool_r` from every cut path is material the program never removes — an uncut island.
    #[test]
    fn pocket_interiors_have_no_uncut_islands_across_the_stepover_range() {
        let shapes = [
            PocketShape::Rect {
                x: 0.0,
                y: 0.0,
                width: 60.0,
                height: 40.0,
            },
            // square: hw == hh, so the ring series never degenerates into a centre line pass
            PocketShape::Rect {
                x: 0.0,
                y: 0.0,
                width: 62.0,
                height: 62.0,
            },
            PocketShape::Circle {
                cx: 10.0,
                cy: 10.0,
                radius: 15.0,
            },
        ];
        for stepover in [0.25, 0.5, 0.75, 1.0] {
            for shape in &shapes {
                let o = PocketOptions {
                    shape: shape.clone(),
                    stepover: Some(stepover),
                    ..rect_opts()
                };
                let tool_r = o.tool_diameter / 2.0;
                let d = Design {
                    ops: try_pocket_ops(&o).unwrap(),
                };
                let tp = crate::resolve::resolve(&d, &crate::resolve::ResolveParams::default());
                let cut: Vec<_> = tp
                    .segments
                    .iter()
                    .filter(|s| s.filament.value() > 0.0)
                    .collect();
                for (px, py) in interior_samples(shape, tool_r, 0.5) {
                    let covered = cut
                        .iter()
                        .any(|s| dist_point_path(px, py, s) <= tool_r + 1e-9);
                    assert!(
                        covered,
                        "uncut island at ({px}, {py}) for {shape:?} at stepover {stepover}"
                    );
                }
            }
        }
    }

    #[test]
    fn defaults_resolve() {
        let r = validate(&rect_opts()).unwrap();
        assert_eq!(r.tool_r, 3.0);
        assert_eq!(r.step, 3.0); // 0.5 * tool_diameter
        assert_eq!(r.depth_per_pass, 5.0); // defaults to depth (single pass)
        assert_eq!(r.z_top, 0.0);
        assert_eq!(r.safe_z, 5.0); // z_top + 5
        assert_eq!(r.cut_feed, 300.0);
        assert_eq!(r.plunge_feed, 100.0); // cut_feed / 3
    }

    #[test]
    fn tool_larger_than_pocket_is_rejected() {
        let mut o = rect_opts();
        o.tool_diameter = 41.0; // > height
        let err = try_pocket_ops(&o).unwrap_err();
        assert!(err.to_string().contains("tool_diameter"), "{err}");
    }

    #[test]
    fn stepover_out_of_range_is_rejected() {
        let mut o = rect_opts();
        o.stepover = Some(1.5);
        assert!(validate(&o).is_err());
        o.stepover = Some(0.0);
        assert!(validate(&o).is_err());
    }

    #[test]
    fn non_finite_and_non_positive_inputs_are_rejected() {
        for bad in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            let mut o = rect_opts();
            o.depth = bad;
            assert!(validate(&o).is_err(), "depth {bad} must be rejected");
            let mut o = rect_opts();
            o.tool_diameter = bad;
            assert!(
                validate(&o).is_err(),
                "tool_diameter {bad} must be rejected"
            );
        }
    }

    #[test]
    fn safe_z_below_z_top_is_rejected() {
        let mut o = rect_opts();
        o.safe_z = Some(-1.0);
        assert!(validate(&o).is_err());
    }

    #[test]
    fn pathological_tiny_depth_per_pass_is_rejected() {
        let mut o = rect_opts();
        o.depth = 100.0;
        o.depth_per_pass = Some(1e-9);
        let err = validate(&o).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("depth_per_pass") && msg.contains("100"),
            "error should mention depth_per_pass and approximate pass count: {msg}"
        );
    }

    #[test]
    fn pathological_tiny_tool_diameter_is_rejected() {
        let mut o = rect_opts();
        o.tool_diameter = 1e-9; // would generate millions of rings
        let err = validate(&o).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("tool_diameter") && (msg.contains("ring") || msg.contains("pass")),
            "error should mention tool_diameter and pass/ring count: {msg}"
        );
    }

    #[test]
    fn depth_levels_clamp_the_last_pass() {
        let o = PocketOptions {
            depth: 5.0,
            depth_per_pass: Some(2.0),
            ..rect_opts()
        };
        let r = validate(&o).unwrap();
        assert_eq!(depth_levels(&r), vec![-2.0, -4.0, -5.0]);
    }

    #[test]
    fn rect_rings_are_innermost_first_and_step_apart() {
        // 60x40 pocket, tool d=6 → outermost ring half-extents (27, 17); step 3.
        let rings = rect_rings(27.0, 17.0, 3.0, 3.0);
        // Innermost first; the smaller half-extent shrinks to <= 0 after 5 more steps
        // (17 - 6*3 = -1) so ring count along hh is 6 rings (17,14,11,8,5,2) then a line pass.
        match rings.first().unwrap() {
            RectPass::Line { along_x, half_len } => {
                assert!(*along_x); // width is the dominant axis
                assert!((half_len - (27.0 - 6.0 * 3.0)).abs() < 1e-12); // 9.0
            }
            other => panic!("innermost pass should be the center line, got {other:?}"),
        }
        match rings.last().unwrap() {
            RectPass::Ring { hw, hh } => {
                assert_eq!((*hw, *hh), (27.0, 17.0)); // outermost = wall inset by tool_r
            }
            other => panic!("outermost pass should be the wall ring, got {other:?}"),
        }
        // consecutive rings differ by exactly `step`
        let ring_hws: Vec<f64> = rings
            .iter()
            .filter_map(|p| match p {
                RectPass::Ring { hw, .. } => Some(*hw),
                _ => None,
            })
            .collect();
        for w in ring_hws.windows(2) {
            assert!((w[1] - w[0] - 3.0).abs() < 1e-12);
        }
    }

    #[test]
    fn rect_pocket_ops_resolve_and_cover() {
        let ops = try_pocket_ops(&rect_opts()).unwrap();
        let d = Design { ops };
        let tp = crate::resolve::resolve(&d, &crate::resolve::ResolveParams::default());
        assert!(tp.segments.len() > 10, "a 60x40 pocket needs many segments");
        // Max XY gap between adjacent cut paths must be <= step: sample the pocket interior
        // (inset by tool_r) on a 1mm grid and assert some cut segment passes within
        // step/2 + tool_r of every sample.
        let cut: Vec<_> = tp
            .segments
            .iter()
            .filter(|s| s.filament.value() > 0.0)
            .collect();
        for gx in 0..=54 {
            for gy in 0..=34 {
                let (px, py) = (3.0 + gx as f64, 3.0 + gy as f64);
                let near = cut.iter().any(|s| {
                    dist_point_segment(px, py, s) <= 1.5 + 3.0 + 1e-9 // step/2 + tool_r
                });
                assert!(near, "uncovered interior point ({px}, {py})");
            }
        }
    }

    #[test]
    fn pathological_tiny_tool_diameter_circle_is_rejected() {
        let mut o = circle_opts();
        o.shape = PocketShape::Circle {
            cx: 10.0,
            cy: 10.0,
            radius: 1000.0,
        };
        o.tool_diameter = 1e-9; // would generate billions of rings
        let err = validate(&o).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("tool_diameter") && (msg.contains("ring") || msg.contains("pass")),
            "error should mention tool_diameter and pass/ring count: {msg}"
        );
    }

    #[test]
    fn circle_radii_are_innermost_first() {
        // outer cut radius 12 (15 - tool_r 3), step 3 → radii 12,9,6,3 innermost-first. The
        // innermost equals tool_r, so no centre-clearing ring is added.
        assert_eq!(circle_radii(12.0, 3.0, 3.0), vec![3.0, 6.0, 9.0, 12.0]);
    }

    #[test]
    fn circle_radii_add_a_centre_ring_when_the_innermost_misses_the_centre() {
        // stepover 1.0 on a radius-15 pocket with a 6mm tool: 12, 6 — the 6mm ring's swath spans
        // radii 3..9, leaving a 3mm uncut centre post. The rescue ring at tool_r closes it.
        assert_eq!(circle_radii(12.0, 6.0, 3.0), vec![3.0, 6.0, 12.0]);
    }

    #[test]
    fn rect_rings_add_a_centre_ring_when_the_innermost_misses_the_centre() {
        // square pocket, stepover 1.0 clamped to the corner-safe inset (3·(1+1/√2) = 5.1213):
        // the series ends on a ring whose half-extents still exceed tool_r, so one shrunk ring is
        // added with the smaller half-extent at exactly tool_r.
        let rings = rect_rings(4.5, 4.5, 6.0, 3.0);
        match rings.first().unwrap() {
            RectPass::Ring { hw, hh } => assert_eq!((*hw, *hh), (3.0, 3.0)),
            other => panic!("innermost pass should be the centre-clearing ring, got {other:?}"),
        }
    }

    #[test]
    fn exact_tool_fit_circle_is_rejected_in_both_modes() {
        // d == 2·radius: the cut radius is 0, which would emit an arc with identical start/end
        // and `I0 J0`.
        for mode in [CutMode::Pocket, CutMode::Profile] {
            let o = PocketOptions {
                mode,
                tool_diameter: 30.0,
                ..circle_opts()
            };
            let err = try_pocket_ops(&o).unwrap_err();
            assert!(
                err.to_string().contains("zero-radius arc"),
                "{mode:?}: {err}"
            );
        }
    }

    #[test]
    fn sub_resolution_cut_radius_is_rejected() {
        // outer_r = 1e-9 rounds to nothing at emission's 6 decimals.
        let o = PocketOptions {
            tool_diameter: 30.0 - 2e-9,
            ..circle_opts()
        };
        let err = try_pocket_ops(&o).unwrap_err();
        assert!(err.to_string().contains("zero-radius arc"), "{err}");
    }

    #[test]
    fn exact_tool_fit_rect_is_rejected_in_both_modes() {
        // d == width == height: both half-extents collapse to zero, so `rect_rings` yields no
        // passes at all and the program would be plunge/retract with no cutting move.
        for mode in [CutMode::Pocket, CutMode::Profile] {
            let o = PocketOptions {
                shape: PocketShape::Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 6.0,
                    height: 6.0,
                },
                mode,
                ..rect_opts()
            };
            let err = try_pocket_ops(&o).unwrap_err();
            assert!(
                err.to_string().contains("no machinable cutting region"),
                "{mode:?}: {err}"
            );
        }
    }

    #[test]
    fn sub_resolution_rect_half_extents_are_rejected() {
        // Both half-extents 1e-9: above zero but below the emission grid, so the "rings" round
        // away to a point.
        for mode in [CutMode::Pocket, CutMode::Profile] {
            let o = PocketOptions {
                shape: PocketShape::Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 6.0 + 2e-9,
                    height: 6.0 + 2e-9,
                },
                mode,
                ..rect_opts()
            };
            let err = try_pocket_ops(&o).unwrap_err();
            assert!(
                err.to_string().contains("no machinable cutting region"),
                "{mode:?}: {err}"
            );
        }
    }

    #[test]
    fn slot_at_exact_tool_width_still_cuts() {
        // Exactly ONE axis at tool size: the cutting region is a zero-width slot down the centre,
        // which the centre `Line` pass cuts. This must keep validating and emitting real moves —
        // and, since Profile mode expresses the slot as a zero-width ring, the result must stay
        // verifier-clean rather than merely resolvable.
        let shape = PocketShape::Rect {
            x: 0.0,
            y: 0.0,
            width: 6.0,
            height: 40.0,
        };
        for mode in [CutMode::Pocket, CutMode::Profile] {
            let o = PocketOptions {
                shape: shape.clone(),
                mode,
                ..rect_opts()
            };
            let ops = try_pocket_ops(&o).expect("slot must validate");
            let d = Design { ops };
            let tp = crate::resolve::resolve_checked(&d, &crate::resolve::ResolveParams::default())
                .unwrap_or_else(|e| panic!("{mode:?}: slot must resolve cleanly: {e:?}"));
            let report = crate::verify::verify(&tp, &crate::verify::Contracts::default());
            assert!(report.ok(), "{mode:?}: slot must verify clean: {report:?}");
            // State what "clean" covers here: this claim is geometric, so name the rules that carry
            // it rather than leaning on `ok()`, which is also true of a pass that inspected nothing.
            assert!(report.segments_inspected > 0, "{mode:?}: nothing inspected");
            for rule in [
                crate::verify::RuleId::Continuity,
                crate::verify::RuleId::SegmentLength,
                crate::verify::RuleId::ArcLength,
                crate::verify::RuleId::NegativeQuantity,
                crate::verify::RuleId::FilamentConsistency,
            ] {
                assert!(
                    report.evaluated(rule),
                    "{mode:?}: {} was not in force",
                    rule.as_str()
                );
            }
            let cut: Vec<_> = tp
                .segments
                .iter()
                .filter(|s| s.filament.value() > 0.0)
                .collect();
            assert!(!cut.is_empty(), "{mode:?}: slot emitted no cutting moves");
            // coverage: every reachable floor point is within tool_r of a cut path
            let tool_r = o.tool_diameter / 2.0;
            for (px, py) in interior_samples(&shape, tool_r, 0.5) {
                assert!(
                    cut.iter()
                        .any(|s| dist_point_path(px, py, s) <= tool_r + 1e-9),
                    "{mode:?}: uncut island at ({px}, {py})"
                );
            }
        }
    }

    #[test]
    fn huge_profile_outline_is_accepted_but_the_same_pocket_is_not() {
        // 500x500 outline, 0.1 mm tool, depth 20 at 0.05/pass → 400 depth levels. Profile cuts one
        // contour per level (400 total passes), while Pocket would walk ~5001 rings per level.
        let base = PocketOptions {
            shape: PocketShape::Rect {
                x: 0.0,
                y: 0.0,
                width: 500.0,
                height: 500.0,
            },
            mode: CutMode::Profile,
            tool_diameter: 0.1,
            depth: 20.0,
            depth_per_pass: Some(0.05),
            ..rect_opts()
        };
        validate(&base).expect("a 500x500 profile at 400 depth levels is a legitimate job");
        let pocket = PocketOptions {
            mode: CutMode::Pocket,
            ..base
        };
        let err = validate(&pocket).unwrap_err();
        assert!(
            err.to_string().contains("rings"),
            "pocket rejection should name the ring count: {err}"
        );
    }

    #[test]
    fn max_total_passes_boundary_is_exact() {
        // rect_opts: 60x40, d=6, stepover 0.5 → step 3, smaller half-extent 17,
        // ring_count = ceil(17/3) + 2 = 8. 8 × 12500 = 100000 = MAX_TOTAL_PASSES.
        let at_limit = PocketOptions {
            depth: 12_500.0,
            depth_per_pass: Some(1.0),
            ..rect_opts()
        };
        validate(&at_limit).expect("exactly MAX_TOTAL_PASSES total passes must validate");
        let past_limit = PocketOptions {
            depth: 12_501.0,
            ..at_limit
        };
        let err = validate(&past_limit).unwrap_err();
        assert!(
            err.to_string().contains("100008") && err.to_string().contains("100000"),
            "boundary rejection should report the exact totals: {err}"
        );
    }

    #[test]
    fn depth_pass_boundary_is_exact() {
        // The depth-pass gate alone, isolated in Profile mode (one contour per level):
        // ceil(depth / depth_per_pass) == MAX_TOTAL_PASSES validates, one more rejects. The ratios
        // are deliberately non-integral so the `ceil` is load-bearing: under `floor`, `past_limit`
        // would come to 100000 and wrongly validate.
        let at_limit = PocketOptions {
            mode: CutMode::Profile,
            depth: MAX_TOTAL_PASSES as f64 - 0.5, // ceil(99_999.5 / 1) = 100_000
            depth_per_pass: Some(1.0),
            ..rect_opts()
        };
        validate(&at_limit).expect("exactly MAX_TOTAL_PASSES depth passes must validate");
        let past_limit = PocketOptions {
            depth: MAX_TOTAL_PASSES as f64 + 0.5, // ceil(100_000.5 / 1) = 100_001
            ..at_limit
        };
        let err = validate(&past_limit).unwrap_err();
        let msg = err.to_string();
        // Pin the *computed* count, not a bare number: `depth` is echoed in the same message, so
        // `contains("100001")` alone would pass on the echo whatever the arithmetic produced.
        assert!(
            msg.contains("depth_per_pass") && msg.contains("would require 100001 passes"),
            "boundary rejection should name depth_per_pass and the computed pass count: {msg}"
        );
    }

    #[test]
    fn circle_pocket_uses_arcs_and_resolves() {
        let ops = try_pocket_ops(&circle_opts()).unwrap();
        let arcs = ops.iter().filter(|op| matches!(op, Op::Arc { .. })).count();
        // two half-circle arcs per ring per depth pass: 4 rings * 2 = 8 (single depth pass)
        assert_eq!(arcs, 8);
        let d = Design { ops };
        let tp = crate::resolve::resolve(&d, &crate::resolve::ResolveParams::default());
        assert!(tp
            .segments
            .iter()
            .any(|s| s.kind == crate::ir::SegmentKind::Arc || s.centre.is_some()));
    }

    #[test]
    fn profile_mode_is_a_single_contour_per_pass() {
        let mut o = rect_opts();
        o.mode = CutMode::Profile;
        o.depth_per_pass = Some(2.5); // 2 passes
        let ops = try_pocket_ops(&o).unwrap();
        let moves = ops
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    Op::Move {
                        x: Some(_),
                        y: Some(_),
                        z: None
                    }
                )
            })
            .count();
        // Profile mode is a single Ring pass. Per depth level, rect_passes emits 5 XY-only
        // (z: None) moves for one ring: start corner, +width, +height, -width (closing back
        // to the start corner) — 4 corner-to-corner edges plus the closing move = 5 moves.
        // The initial rapid entry to the same corner has z: Some(safe_z), so it does not
        // match the z: None filter and is correctly excluded. 2 depth passes * 5 = 10.
        assert_eq!(moves, 2 * 5);
    }

    #[test]
    fn circle_profile_is_one_ring() {
        let mut o = circle_opts();
        o.mode = CutMode::Profile;
        let ops = try_pocket_ops(&o).unwrap();
        let arcs = ops.iter().filter(|op| matches!(op, Op::Arc { .. })).count();
        assert_eq!(arcs, 2); // one ring = two half circles, single depth pass
    }

    #[test]
    fn helical_entry_emits_ramp_moves() {
        let mut o = rect_opts();
        o.helical_entry = Some(true);
        let ops = try_pocket_ops(&o).unwrap();
        // Should have 16 ramp steps per depth level
        let ramp_moves = ops
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    Op::Move {
                        x: Some(_),
                        y: Some(_),
                        z: Some(z_val)
                    } if *z_val < 5.0 && *z_val >= -5.0
                )
            })
            .count();
        assert!(ramp_moves >= 16);
    }
}


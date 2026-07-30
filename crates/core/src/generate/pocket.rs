//! `pocket` — contour-parallel CNC pocket/profile generator (P5.3, spec
//! `docs/superpowers/specs/2026-07-30-cnc-pocket-profile-design.md`).
//!
//! Pure L1 sugar like [`super::tpms`]: validated options → `Vec<Op>`; resolve/verify/
//! simulate/emit are inherited unchanged.

use crate::resolve::{Design, Op};

/// Maximum total passes (depth × rings) before rejecting as pathological input.
/// No legitimate job reaches this; it gates infinite loops on tiny step sizes.
const MAX_TOTAL_PASSES: u32 = 100_000;

#[derive(Debug, Clone, PartialEq)]
pub enum PocketShape {
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    Circle {
        cx: f64,
        cy: f64,
        radius: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CutMode {
    #[default]
    Pocket,
    Profile,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PocketOptions {
    pub shape: PocketShape,
    pub mode: CutMode,
    pub tool_diameter: f64,
    pub stepover: Option<f64>,
    pub depth: f64,
    pub depth_per_pass: Option<f64>,
    pub z_top: Option<f64>,
    pub safe_z: Option<f64>,
    pub cut_feed: Option<f64>,
    pub plunge_feed: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PocketError {
    message: String,
}

impl PocketError {
    fn new(message: impl Into<String>) -> Self {
        PocketError {
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
            // Compute ring count: count how many rings fit before both dimensions collapse.
            let tool_r = d / 2.0;
            let step = stepover * d;
            let hw = width / 2.0 - tool_r;
            let hh = height / 2.0 - tool_r;
            let smaller = hw.min(hh);
            let ring_count_f = (smaller / step).ceil() + 1.0; // +1 for final line pass
            if ring_count_f > MAX_TOTAL_PASSES as f64 {
                return Err(PocketError::new(format!(
                    "tool_diameter ({d}) too small relative to pocket dimensions ({width}x{height}): \
                     would require ~{ring_count_f:.0} rings (max {MAX_TOTAL_PASSES} total passes)"
                )));
            }
            let ring_count = ring_count_f as u32;
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
            // Compute ring count: circle_radii pushes one radius per `step` from the
            // wall-inset outer radius down to (but not including) zero.
            let tool_r = d / 2.0;
            let step = stepover * d;
            let outer_r = radius - tool_r;
            let ring_count_f = (outer_r / step).ceil().max(1.0);
            if ring_count_f > MAX_TOTAL_PASSES as f64 {
                return Err(PocketError::new(format!(
                    "tool_diameter ({d}) too small relative to circle radius ({radius}): \
                     would require ~{ring_count_f:.0} rings (max {MAX_TOTAL_PASSES} total passes)"
                )));
            }
            let ring_count = ring_count_f as u32;
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
fn rect_rings(hw: f64, hh: f64, step: f64) -> Vec<RectPass> {
    let mut out = Vec::new(); // built outermost-first, reversed at the end
    let mut k = 0u32;
    loop {
        let (sw, sh) = (hw - k as f64 * step, hh - k as f64 * step);
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
        ops.push(Op::Move {
            x: None,
            y: None,
            z: Some(z),
        });
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
fn circle_radii(outer_r: f64, step: f64) -> Vec<f64> {
    let mut radii = Vec::new();
    let mut r = outer_r;
    while r > 0.0 {
        radii.push(r);
        r -= step;
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
        ops.push(Op::Move {
            x: None,
            y: None,
            z: Some(z),
        });
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
            let rings = rect_rings(width / 2.0 - r.tool_r, height / 2.0 - r.tool_r, r.step);
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
        (PocketShape::Circle { cx, cy, radius }, CutMode::Pocket) => {
            circle_passes(*cx, *cy, &circle_radii(radius - r.tool_r, r.step), r)
        }
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
        let rings = rect_rings(27.0, 17.0, 3.0);
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
        // outer cut radius 12 (15 - tool_r 3), step 3 → radii 12,9,6,3 innermost-first.
        assert_eq!(circle_radii(12.0, 3.0), vec![3.0, 6.0, 9.0, 12.0]);
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
}

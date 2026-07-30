# CNC Pocket/Profile Vertical Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close P5.3's acceptance ("a pocket/profile emits a valid CNC program") with a contour-parallel rect+circle pocket/profile generator and a profile-driven RS-274 program frame.

**Architecture:** A new L1 generator (`generate/pocket.rs`, same "pure L1 sugar" pattern as TPMS) emits `Vec<Op>`; a new `CncFrame` block on the profile flows through `Profile::emit_params()` into an additive `EmitParams.cnc_frame` field, which the rs274 flavor renders as preamble/postamble. No IR, codec, or spec-vector change.

**Tech Stack:** Rust (dry-core, dry-cli), clap, serde. No new dependencies. Spec: `docs/superpowers/specs/2026-07-30-cnc-pocket-profile-design.md`. Issue: #179.

## Global Constraints

- No new crate dependencies; geometry math is closed-form (`libm`-compatible; here plain arithmetic — no transcendental calls are needed beyond what `Op::Arc` lowering already does).
- `EmitParams` gains only `#[serde(default)]` additive fields; `EmitParams::default()` behavior is unchanged; when `cnc_frame` is `None`, rs274 output is byte-identical to today (existing fixtures must not drift).
- Generators never panic on user input: `try_*` returns structured errors; plain wrappers document their precondition (mirror `tpms.rs`).
- Every commit ends with `cargo fmt --all` clean and `cargo clippy --workspace --all-targets -- -D warnings` clean.
- Commit messages: conventional-commit style, `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>` trailer.

---

### Task 1: Pocket options, validation, and error type

**Files:**
- Create: `crates/core/src/generate/pocket.rs`
- Modify: `crates/core/src/generate/mod.rs` (add `pub mod pocket;` + re-exports)
- Modify: `crates/core/src/lib.rs` (extend `pub use generate::{...}`)
- Test: inline `#[cfg(test)]` in `crates/core/src/generate/pocket.rs`

**Interfaces:**
- Consumes: `crate::resolve::{Design, Op}` (existing).
- Produces (later tasks rely on these exact names):
  - `pub enum PocketShape { Rect { x: f64, y: f64, width: f64, height: f64 }, Circle { cx: f64, cy: f64, radius: f64 } }`
  - `pub enum CutMode { Pocket, Profile }` (`Default` = `Pocket`)
  - `pub struct PocketOptions { pub shape: PocketShape, pub mode: CutMode, pub tool_diameter: f64, pub stepover: Option<f64>, pub depth: f64, pub depth_per_pass: Option<f64>, pub z_top: Option<f64>, pub safe_z: Option<f64>, pub cut_feed: Option<f64>, pub plunge_feed: Option<f64> }`
  - `pub struct PocketError { message: String }` (Display + std::error::Error, like `TpmsError`)
  - `pub fn try_pocket_ops(o: &PocketOptions) -> Result<Vec<Op>, PocketError>`
  - `pub fn pocket_ops(o: &PocketOptions) -> Vec<Op>` / `try_pocket_design` / `pocket_design`
  - Internal, used by Tasks 2–3: `struct Resolved { tool_r: f64, step: f64, depth: f64, depth_per_pass: f64, z_top: f64, safe_z: f64, cut_feed: f64, plunge_feed: f64 }` and `fn validate(o: &PocketOptions) -> Result<Resolved, PocketError>`

- [ ] **Step 1: Write the failing validation tests** (inline test module):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn rect_opts() -> PocketOptions {
        PocketOptions {
            shape: PocketShape::Rect { x: 0.0, y: 0.0, width: 60.0, height: 40.0 },
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
            assert!(validate(&o).is_err(), "tool_diameter {bad} must be rejected");
        }
    }

    #[test]
    fn safe_z_below_z_top_is_rejected() {
        let mut o = rect_opts();
        o.safe_z = Some(-1.0);
        assert!(validate(&o).is_err());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p dry-core pocket -- --nocapture`
Expected: compile error (`validate` / types not defined).

- [ ] **Step 3: Implement types + validation** (no geometry yet — `try_pocket_ops` calls `validate` then `unimplemented sections return an empty prologue`; to keep the build honest, have `try_pocket_ops` return just the prologue ops for now):

```rust
//! `pocket` — contour-parallel CNC pocket/profile generator (P5.3, spec
//! `docs/superpowers/specs/2026-07-30-cnc-pocket-profile-design.md`).
//!
//! Pure L1 sugar like [`super::tpms`]: validated options → `Vec<Op>`; resolve/verify/
//! simulate/emit are inherited unchanged.

use crate::resolve::{Design, Op};

#[derive(Debug, Clone, PartialEq)]
pub enum PocketShape {
    Rect { x: f64, y: f64, width: f64, height: f64 },
    Circle { cx: f64, cy: f64, radius: f64 },
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
        PocketError { message: message.into() }
    }
}

impl std::fmt::Display for PocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PocketError {}

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
        Err(PocketError::new(format!("{name} must be finite and > 0, got {v}")))
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
    match o.shape {
        PocketShape::Rect { x, y, width, height } => {
            finite("x", x)?;
            finite("y", y)?;
            positive("width", width)?;
            positive("height", height)?;
            if d > width || d > height {
                return Err(PocketError::new(format!(
                    "tool_diameter ({d}) does not fit the {width}x{height} rectangle"
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

/// Generate the L1 ops. Structured failure on invalid options, never a panic.
pub fn try_pocket_ops(o: &PocketOptions) -> Result<Vec<Op>, PocketError> {
    let r = validate(o)?;
    let mut ops = vec![
        Op::Geometry { width: Some(o.tool_diameter), height: Some(r.depth_per_pass) },
        Op::Extruder { on: false },
        Op::Speed { print: r.cut_feed },
    ];
    ops.extend(passes(o, &r)?);
    Ok(ops)
}

// Filled in by the geometry tasks; keeping it separate keeps try_pocket_ops final.
fn passes(_o: &PocketOptions, _r: &Resolved) -> Result<Vec<Op>, PocketError> {
    Ok(Vec::new())
}

/// Panicking convenience over [`try_pocket_ops`]; precondition: valid Dry pocket options.
pub fn pocket_ops(o: &PocketOptions) -> Vec<Op> {
    try_pocket_ops(o).expect("valid Dry pocket options")
}

pub fn try_pocket_design(o: &PocketOptions) -> Result<Design, PocketError> {
    Ok(Design { ops: try_pocket_ops(o)? })
}

pub fn pocket_design(o: &PocketOptions) -> Design {
    Design { ops: pocket_ops(o) }
}
```

In `generate/mod.rs` add:

```rust
pub mod pocket;

pub use pocket::{
    pocket_design, pocket_ops, try_pocket_design, try_pocket_ops, CutMode, PocketError,
    PocketOptions, PocketShape,
};
```

In `lib.rs`, extend the existing `pub use generate::{...}` list with the same eight names.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p dry-core pocket -- --nocapture`
Expected: all Task-1 tests PASS.

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/core/src/generate/ crates/core/src/lib.rs
git commit -m "feat(core): pocket generator options, validation and error type (P5.3, #179)"
```

---

### Task 2: Rectangular pocket geometry (contour-parallel rings + depth passes)

**Files:**
- Modify: `crates/core/src/generate/pocket.rs` (replace the `passes` stub; add `rect_rings`, `depth_levels`)
- Test: inline test module of the same file

**Interfaces:**
- Consumes: `validate`/`Resolved` from Task 1.
- Produces (Task 3 reuses): `fn depth_levels(r: &Resolved) -> Vec<f64>` (top-down cut z per pass, last clamped to `z_top - depth`); `enum RectPass { Ring { hw: f64, hh: f64 }, Line { half_len: f64, along_x: bool } }`; `fn rect_rings(hw: f64, hh: f64, step: f64) -> Vec<RectPass>` returning innermost-first passes.

- [ ] **Step 1: Write the failing geometry tests**

```rust
#[test]
fn depth_levels_clamp_the_last_pass() {
    let o = PocketOptions { depth: 5.0, depth_per_pass: Some(2.0), ..rect_opts() };
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
    let ring_hws: Vec<f64> = rings.iter().filter_map(|p| match p {
        RectPass::Ring { hw, .. } => Some(*hw),
        _ => None,
    }).collect();
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
    let cut: Vec<_> = tp.segments.iter().filter(|s| s.filament.value() > 0.0).collect();
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
```

(Include the small `fn dist_point_segment(px: f64, py: f64, s: &crate::ir::Segment) -> f64` helper in the test module — standard point-to-segment distance over the segment's XY start/end, treating `None` Z as irrelevant.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p dry-core pocket -- --nocapture`
Expected: FAIL (`depth_levels`, `rect_rings`, `RectPass` not defined; coverage test fails on empty `passes`).

- [ ] **Step 3: Implement**

```rust
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
            out.push(RectPass::Line { half_len, along_x: sw >= sh });
        }
        break;
    }
    out.reverse();
    out
}

fn passes(o: &PocketOptions, r: &Resolved) -> Result<Vec<Op>, PocketError> {
    match (&o.shape, o.mode) {
        (PocketShape::Rect { x, y, width, height }, CutMode::Pocket) => {
            let (cx, cy) = (x + width / 2.0, y + height / 2.0);
            let rings = rect_rings(width / 2.0 - r.tool_r, height / 2.0 - r.tool_r, r.step);
            Ok(rect_passes(cx, cy, &rings, r))
        }
        _ => Ok(Vec::new()), // circle + profile modes land in Task 3
    }
}

fn rect_passes(cx: f64, cy: f64, rings: &[RectPass], r: &Resolved) -> Vec<Op> {
    let mut ops = Vec::new();
    let entry_xy = match rings.first() {
        Some(RectPass::Ring { hw, hh }) => (cx - hw, cy - hh),
        Some(RectPass::Line { half_len, along_x: true }) => (cx - half_len, cy),
        Some(RectPass::Line { half_len, along_x: false }) => (cx, cy - half_len),
        None => (cx, cy),
    };
    for &z in &depth_levels(r) {
        // rapid to entry above the work, then plunge
        ops.push(Op::Extruder { on: false });
        ops.push(Op::Move { x: None, y: None, z: Some(r.safe_z) });
        ops.push(Op::Move { x: Some(entry_xy.0), y: Some(entry_xy.1), z: Some(r.safe_z) });
        ops.push(Op::Speed { print: r.plunge_feed });
        ops.push(Op::Extruder { on: true });
        ops.push(Op::Move { x: None, y: None, z: Some(z) });
        ops.push(Op::Speed { print: r.cut_feed });
        for pass in rings {
            match *pass {
                RectPass::Ring { hw, hh } => {
                    // link into the ring's start corner (a cutting stepover move), then 4 sides
                    ops.push(Op::Move { x: Some(cx - hw), y: Some(cy - hh), z: None });
                    ops.push(Op::Move { x: Some(cx + hw), y: Some(cy - hh), z: None });
                    ops.push(Op::Move { x: Some(cx + hw), y: Some(cy + hh), z: None });
                    ops.push(Op::Move { x: Some(cx - hw), y: Some(cy + hh), z: None });
                    ops.push(Op::Move { x: Some(cx - hw), y: Some(cy - hh), z: None });
                }
                RectPass::Line { half_len, along_x } => {
                    let (ax, ay, bx, by) = if along_x {
                        (cx - half_len, cy, cx + half_len, cy)
                    } else {
                        (cx, cy - half_len, cx, cy + half_len)
                    };
                    ops.push(Op::Move { x: Some(ax), y: Some(ay), z: None });
                    ops.push(Op::Move { x: Some(bx), y: Some(by), z: None });
                    ops.push(Op::Move { x: Some(ax), y: Some(ay), z: None });
                }
            }
        }
        ops.push(Op::Extruder { on: false });
        ops.push(Op::Move { x: None, y: None, z: Some(r.safe_z) });
    }
    ops
}
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p dry-core pocket -- --nocapture`
Expected: PASS (including coverage).

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/core/src/generate/pocket.rs
git commit -m "feat(core): contour-parallel rectangular pocket passes (P5.3, #179)"
```

---

### Task 3: Circular pocket + Profile mode

**Files:**
- Modify: `crates/core/src/generate/pocket.rs`
- Test: inline test module

**Interfaces:**
- Consumes: `depth_levels`, `Resolved`, `passes` dispatch from Task 2.
- Produces: full `passes()` coverage of all four `(shape, mode)` combinations; internal `fn circle_radii(outer_r: f64, step: f64) -> Vec<f64>` (innermost-first cut radii).

- [ ] **Step 1: Write the failing tests**

```rust
fn circle_opts() -> PocketOptions {
    PocketOptions {
        shape: PocketShape::Circle { cx: 10.0, cy: 10.0, radius: 15.0 },
        ..rect_opts()
    }
}

#[test]
fn circle_radii_are_innermost_first() {
    // outer cut radius 12 (15 - tool_r 3), step 3 → radii 12,9,6,3 then center point
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
    assert!(tp.segments.iter().any(|s| s.kind == crate::ir::SegmentKind::Arc
        || s.centre.is_some()));
}

#[test]
fn profile_mode_is_a_single_contour_per_pass() {
    let mut o = rect_opts();
    o.mode = CutMode::Profile;
    o.depth_per_pass = Some(2.5); // 2 passes
    let ops = try_pocket_ops(&o).unwrap();
    // exactly 5 boundary moves (corner..corner..close) per pass, no inner rings
    let moves = ops.iter().filter(|op| matches!(op,
        Op::Move { x: Some(_), y: Some(_), z: None })).count();
    assert_eq!(moves, 2 * (5 + 1)); // per pass: 1 link into start corner + 5 ring moves — wait, entry IS the start corner, so 5 ring moves + the entry-linked first corner = 6 XY moves
}

#[test]
fn circle_profile_is_one_ring() {
    let mut o = circle_opts();
    o.mode = CutMode::Profile;
    let ops = try_pocket_ops(&o).unwrap();
    let arcs = ops.iter().filter(|op| matches!(op, Op::Arc { .. })).count();
    assert_eq!(arcs, 2); // one ring = two half circles, single depth pass
}
```

Note for the implementer: derive the exact expected `moves` count from your implementation *before* writing the assertion — the comment above shows the reasoning style; the committed test must assert the true count with a comment explaining it (the rect ring emits 1 link move + 5 ring moves = 6 XY-only moves per pass in the Task 2 code).

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p dry-core pocket -- --nocapture`
Expected: FAIL (`circle_radii` undefined; circle/profile arms return empty).

- [ ] **Step 3: Implement**

```rust
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
        ops.push(Op::Move { x: None, y: None, z: Some(r.safe_z) });
        ops.push(Op::Move { x: Some(entry.0), y: Some(entry.1), z: Some(r.safe_z) });
        ops.push(Op::Speed { print: r.plunge_feed });
        ops.push(Op::Extruder { on: true });
        ops.push(Op::Move { x: None, y: None, z: Some(z) });
        ops.push(Op::Speed { print: r.cut_feed });
        for &ri in radii {
            // stepover link to the ring start, then two half circles (G2/G3 exercised)
            ops.push(Op::Move { x: Some(cx - ri), y: Some(cy), z: None });
            ops.push(Op::Arc { cx, cy, x: Some(cx + ri), y: Some(cy), z: None, clockwise: false });
            ops.push(Op::Arc { cx, cy, x: Some(cx - ri), y: Some(cy), z: None, clockwise: false });
        }
        ops.push(Op::Extruder { on: false });
        ops.push(Op::Move { x: None, y: None, z: Some(r.safe_z) });
    }
    ops
}

fn passes(o: &PocketOptions, r: &Resolved) -> Result<Vec<Op>, PocketError> {
    Ok(match (&o.shape, o.mode) {
        (PocketShape::Rect { x, y, width, height }, CutMode::Pocket) => {
            let (cx, cy) = (x + width / 2.0, y + height / 2.0);
            let rings = rect_rings(width / 2.0 - r.tool_r, height / 2.0 - r.tool_r, r.step);
            rect_passes(cx, cy, &rings, r)
        }
        (PocketShape::Rect { x, y, width, height }, CutMode::Profile) => {
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
```

(Delete the Task-2 dispatch version of `passes`; keep `rect_passes`/`rect_rings` unchanged.)

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p dry-core pocket -- --nocapture`
Expected: PASS.

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/core/src/generate/pocket.rs
git commit -m "feat(core): circular pocket and profile cut modes (P5.3, #179)"
```

---

### Task 4: `CncFrame` — profile block + `EmitParams` plumbing

**Files:**
- Modify: `crates/core/src/emit/gcode.rs` (define `CncFrame`, add `EmitParams.cnc_frame`)
- Modify: `crates/core/src/emit/mod.rs` + `crates/core/src/lib.rs` (re-export `CncFrame`)
- Modify: `crates/core/src/profile/mod.rs` (`MachineProfile.cnc`, validation, `emit_params()` copy)
- Test: inline test modules in both files

**Interfaces:**
- Consumes: existing `EmitParams`, `MachineProfile`, `Profile::validate`, `Profile::emit_params`.
- Produces (Task 5 renders it; Task 6 flows it from `--profile`):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct CncFrame {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wcs: Option<u8>,          // 54..=59 → G54..G59; None ⇒ G54 line still emitted? NO — None ⇒ default 54
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coolant: Option<bool>,
}
// EmitParams gains: #[serde(default, skip_serializing_if = "Option::is_none")] pub cnc_frame: Option<CncFrame>
// MachineProfile gains: #[serde(default, skip_serializing_if = "Option::is_none")] pub cnc: Option<CncFrame>
```

- [ ] **Step 1: Write the failing tests**

In `profile/mod.rs` tests:

```rust
#[test]
fn cnc_frame_parses_and_flows_to_emit_params() {
    let profile: Profile = serde_json::from_str(r#"{
        "version": 1,
        "firmware": { "flavor": "rs274" },
        "machine": { "cnc": { "wcs": 55, "tool": 3, "spindle_rpm": 12000, "coolant": true } }
    }"#).unwrap();
    profile.validate().unwrap();
    let params = profile.emit_params();
    let frame = params.cnc_frame.expect("machine.cnc flows into EmitParams");
    assert_eq!(frame.wcs, Some(55));
    assert_eq!(frame.tool, Some(3));
    assert_eq!(frame.spindle_rpm, Some(12000.0));
    assert_eq!(frame.coolant, Some(true));
}

#[test]
fn cnc_frame_validation_rejects_bad_values() {
    for (field, json) in [
        ("wcs", r#"{"version":1,"machine":{"cnc":{"wcs":53}}}"#),
        ("wcs", r#"{"version":1,"machine":{"cnc":{"wcs":60}}}"#),
        ("spindle_rpm", r#"{"version":1,"machine":{"cnc":{"spindle_rpm":0}}}"#),
        ("spindle_rpm", r#"{"version":1,"machine":{"cnc":{"spindle_rpm":-100}}}"#),
    ] {
        let profile: Profile = serde_json::from_str(json).unwrap();
        let err = profile.validate().unwrap_err();
        assert!(err.to_string().contains(field), "expected {field} error, got: {err}");
    }
}

#[test]
fn profiles_without_cnc_are_unchanged() {
    let profile: Profile = serde_json::from_str(r#"{"version":1}"#).unwrap();
    assert!(profile.emit_params().cnc_frame.is_none());
}
```

In `emit/gcode.rs` tests:

```rust
#[test]
fn emit_params_json_without_cnc_frame_deserializes() {
    let p: EmitParams = serde_json::from_str(r#"{"relative_e":true}"#).unwrap();
    assert!(p.cnc_frame.is_none());
    assert!(EmitParams::default().cnc_frame.is_none());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p dry-core cnc_frame -- --nocapture && cargo test -p dry-core emit_params_json`
Expected: compile error (`cnc_frame`/`CncFrame` not defined).

- [ ] **Step 3: Implement**

In `emit/gcode.rs`: add the `CncFrame` struct exactly as in Interfaces, and the `cnc_frame` field on `EmitParams` with `#[serde(default, skip_serializing_if = "Option::is_none")]`. In `emit/mod.rs` and `lib.rs`, re-export `CncFrame` alongside `EmitParams`.

In `profile/mod.rs`:
- Add the `cnc` field to `MachineProfile` (see Interfaces).
- In `Profile::validate()`, after the existing `machine.five_axis` check:

```rust
if let Some(cnc) = &self.machine.cnc {
    if let Some(wcs) = cnc.wcs {
        if !(54..=59).contains(&wcs) {
            return Err(ProfileError::new(format!(
                "machine.cnc.wcs must be 54..=59 (G54..G59), got {wcs}"
            )));
        }
    }
    if let Some(rpm) = cnc.spindle_rpm {
        if !(rpm.is_finite() && rpm > 0.0) {
            return Err(ProfileError::new(format!(
                "machine.cnc.spindle_rpm must be finite and > 0, got {rpm}"
            )));
        }
    }
}
```

- In `Profile::emit_params()`, after the five_axis override: `params.cnc_frame = self.machine.cnc;`

(Match the exact `ProfileError` constructor used by the surrounding code — read the neighbors first; if the file uses a different error-construction idiom, mirror it.)

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p dry-core -- --nocapture` (full crate — the additive field must not break any existing profile/emit test)
Expected: PASS across the whole crate.

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/core/src/emit/ crates/core/src/profile/mod.rs crates/core/src/lib.rs
git commit -m "feat(core): CncFrame profile block flows into EmitParams (P5.3, #179)"
```

---

### Task 5: RS-274 preamble/postamble rendering

**Files:**
- Modify: `crates/core/src/emit/gcode.rs` (`emit_stream_to_writer`)
- Test: `crates/core/tests/cnc_frame_emit.rs` (new integration test)

**Interfaces:**
- Consumes: `CncFrame` on `EmitParams` (Task 4); `write_line`, `num`, `first_line` mechanics already in `emit_stream_to_writer`.
- Produces: rs274 emissions bracketed by the frame; **byte-identical output when `cnc_frame` is `None` or the flavor is not `Rs274`**.

- [ ] **Step 1: Write the failing test**

```rust
use dry_core::{emit, resolve, CncFrame, Design, EmitParams, FirmwareFlavor, ResolveParams};

fn tiny_design() -> Design {
    serde_json::from_str(r#"{"ops":[
        {"op":"geometry","width":6.0,"height":2.0},
        {"op":"extruder","on":true},
        {"op":"speed","print":300},
        {"op":"move","x":0,"y":0,"z":-1},
        {"op":"move","x":10,"y":0,"z":-1}
    ]}"#).unwrap()
}

fn frame() -> CncFrame {
    CncFrame { wcs: Some(55), tool: Some(3), spindle_rpm: Some(12000.0), coolant: Some(true) }
}

#[test]
fn rs274_frame_brackets_the_program() {
    let tp = resolve(&tiny_design(), &ResolveParams::default());
    let p = EmitParams {
        flavor: FirmwareFlavor::Rs274,
        cnc_frame: Some(frame()),
        ..EmitParams::default()
    };
    let lines = emit(&tp, &p);
    let head: Vec<&str> = lines.iter().take(5).map(String::as_str).collect();
    assert_eq!(head, vec!["G21 G17 G90", "G55", "T3 M6", "S12000 M3", "M8"]);
    let n = lines.len();
    assert_eq!(&lines[n - 3..], &["M9".to_string(), "M5".to_string(), "M30".to_string()]);
}

#[test]
fn minimal_frame_omits_optional_words() {
    let tp = resolve(&tiny_design(), &ResolveParams::default());
    let p = EmitParams {
        flavor: FirmwareFlavor::Rs274,
        cnc_frame: Some(CncFrame::default()),
        ..EmitParams::default()
    };
    let lines = emit(&tp, &p);
    assert_eq!(&lines[..2], &["G21 G17 G90".to_string(), "G54".to_string()]);
    assert!(!lines.iter().any(|l| l.starts_with('T') || l.starts_with('S') || l == "M8" || l == "M9" || l == "M5"));
    assert_eq!(lines.last().unwrap(), "M30");
}

#[test]
fn no_frame_or_non_rs274_flavor_is_byte_identical_to_before() {
    let tp = resolve(&tiny_design(), &ResolveParams::default());
    let bare = emit(&tp, &EmitParams { flavor: FirmwareFlavor::Rs274, ..EmitParams::default() });
    assert!(!bare.iter().any(|l| l == "G21 G17 G90" || l == "M30"), "None frame must not add lines");
    let marlin_with_frame = emit(&tp, &EmitParams {
        flavor: FirmwareFlavor::Marlin,
        cnc_frame: Some(frame()),
        ..EmitParams::default()
    });
    let marlin_bare = emit(&tp, &EmitParams::default());
    assert_eq!(marlin_with_frame, marlin_bare, "non-rs274 flavors ignore cnc_frame in this slice");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p dry-core --test cnc_frame_emit -- --nocapture`
Expected: FAIL (no preamble emitted).

- [ ] **Step 3: Implement** — in `emit_stream_to_writer`, immediately after the local variable declarations and before the segment loop:

```rust
let frame = match (p.flavor, &p.cnc_frame) {
    (FirmwareFlavor::Rs274, Some(f)) => Some(*f),
    _ => None,
};
if let Some(f) = frame {
    write_line(writer, &mut first_line, "G21 G17 G90")?;
    write_line(writer, &mut first_line, &format!("G{}", f.wcs.unwrap_or(54)))?;
    if let Some(tool) = f.tool {
        write_line(writer, &mut first_line, &format!("T{tool} M6"))?;
    }
    if let Some(rpm) = f.spindle_rpm {
        write_line(writer, &mut first_line, &format!("S{} M3", num(rpm)))?;
    }
    if f.coolant == Some(true) {
        write_line(writer, &mut first_line, "M8")?;
    }
}
```

And after the loop, before `Ok(())`:

```rust
if let Some(f) = frame {
    if f.coolant == Some(true) {
        write_line(writer, &mut first_line, "M9")?;
    }
    if f.spindle_rpm.is_some() {
        write_line(writer, &mut first_line, "M5")?;
    }
    write_line(writer, &mut first_line, "M30")?;
}
```

- [ ] **Step 4: Run the full core suite** (drift guard: existing rs274/GRBL/five-axis fixtures must not change)

Run: `cargo test -p dry-core`
Expected: PASS everywhere.

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/core/src/emit/gcode.rs crates/core/tests/cnc_frame_emit.rs
git commit -m "feat(emit): render the RS-274 program frame from CncFrame (P5.3, #179)"
```

---

### Task 6: CLI `dry generate pocket`

**Files:**
- Modify: `crates/cli/src/main.rs` (new `Generate` subcommand with a `Pocket` variant)
- Test: `crates/cli/tests/cli.rs` (regression)

**Interfaces:**
- Consumes: `dry_core::{try_pocket_design, PocketOptions, PocketShape, CutMode, resolve_checked, ResolveParams}`; the existing `--profile` loading helper in `main.rs` (find the function the `Emit` arm uses to load a profile and reuse it).
- Produces: `dry generate pocket … -o out.json` writing **resolved L2 Dry IR JSON** (`Toolpath::to_json`, the same serialization `unpack` writes — locate the existing helper in `main.rs` and reuse it; do not hand-roll).

- [ ] **Step 1: Write the failing CLI test** (follow the existing `assert_cmd`/process-spawn style used throughout `crates/cli/tests/cli.rs` — read two neighboring tests first and copy their harness idiom):

> [Editorial note, added post-review] The `--mode` flag in the sketches below shipped as
> `--cut-mode`, to keep it distinct from `--profile <machine.json>`. Plan text left as written.

```rust
#[test]
fn generate_pocket_emits_a_framed_rs274_program() {
    let dir = tempfile::tempdir().unwrap();
    let ir = dir.path().join("pocket.json");
    let profile = dir.path().join("cnc.json");
    std::fs::write(&profile, r#"{
        "version": 1,
        "firmware": { "flavor": "rs274" },
        "machine": { "cnc": { "tool": 1, "spindle_rpm": 10000 } }
    }"#).unwrap();

    dry_cmd() // the test file's existing helper for invoking the binary
        .args(["generate", "pocket", "--shape", "rect", "--x", "0", "--y", "0",
               "--width", "60", "--height", "40", "--tool-diameter", "6",
               "--depth", "5", "--depth-per-pass", "2.5",
               "-o", ir.to_str().unwrap()])
        .assert().success();

    let out = dry_cmd()
        .args(["emit", ir.to_str().unwrap(), "--format", "rs274",
               "--profile", profile.to_str().unwrap()])
        .assert().success();
    let gcode = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(gcode.starts_with("G21 G17 G90\nG54\nT1 M6\nS10000 M3\n"), "got head: {}",
        &gcode[..gcode.len().min(120)]);
    assert!(gcode.trim_end().ends_with("M30"));
    assert!(!gcode.contains(" E"), "rs274 must carry no extruder words");
    assert!(gcode.contains("G0"), "safe-Z rapids must emit as G0");
}

#[test]
fn generate_pocket_circle_profile_mode() {
    let dir = tempfile::tempdir().unwrap();
    let ir = dir.path().join("ring.json");
    dry_cmd()
        .args(["generate", "pocket", "--shape", "circle", "--cx", "0", "--cy", "0",
               "--radius", "15", "--tool-diameter", "6", "--depth", "3",
               "--mode", "profile", "-o", ir.to_str().unwrap()])
        .assert().success();
    let out = dry_cmd()
        .args(["emit", ir.to_str().unwrap(), "--format", "rs274"])
        .assert().success();
    let gcode = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(gcode.contains("G3") || gcode.contains("G2"), "circle profile must emit arcs");
}

#[test]
fn generate_pocket_rejects_oversized_tool() {
    dry_cmd()
        .args(["generate", "pocket", "--shape", "rect", "--x", "0", "--y", "0",
               "--width", "10", "--height", "10", "--tool-diameter", "12", "--depth", "1",
               "-o", "/dev/null"])
        .assert().failure();
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p dry-cli generate_pocket -- --nocapture`
Expected: FAIL ("unrecognized subcommand 'generate'").

- [ ] **Step 3: Implement** — add to the `Commands` enum in `main.rs`:

```rust
/// Generate a parametric design and write its resolved Dry IR.
Generate {
    #[command(subcommand)]
    what: GenerateCmd,
},
```

```rust
#[derive(Subcommand)]
enum GenerateCmd {
    /// Contour-parallel CNC pocket/profile (rect or circle). Writes resolved Dry IR JSON.
    Pocket {
        /// rect | circle
        #[arg(long, value_parser = ["rect", "circle"])]
        shape: String,
        #[arg(long, allow_hyphen_values = true)] x: Option<f64>,
        #[arg(long, allow_hyphen_values = true)] y: Option<f64>,
        #[arg(long)] width: Option<f64>,
        #[arg(long)] height: Option<f64>,
        #[arg(long, allow_hyphen_values = true)] cx: Option<f64>,
        #[arg(long, allow_hyphen_values = true)] cy: Option<f64>,
        #[arg(long)] radius: Option<f64>,
        /// pocket (clear the interior) | profile (single boundary contour)
        #[arg(long, default_value = "pocket", value_parser = ["pocket", "profile"])]
        mode: String,
        #[arg(long)] tool_diameter: f64,
        /// Stepover as a fraction of tool diameter in (0, 1].
        #[arg(long)] stepover: Option<f64>,
        #[arg(long)] depth: f64,
        #[arg(long)] depth_per_pass: Option<f64>,
        #[arg(long, allow_hyphen_values = true)] z_top: Option<f64>,
        #[arg(long, allow_hyphen_values = true)] safe_z: Option<f64>,
        /// Cutting feed, mm/min.
        #[arg(long)] cut_feed: Option<f64>,
        /// Plunge feed, mm/min (default cut_feed / 3).
        #[arg(long)] plunge_feed: Option<f64>,
        /// Machine/material profile JSON (supplies ResolveParams defaults).
        #[arg(long)] profile: Option<String>,
        /// Write the resolved Dry IR JSON here instead of stdout.
        #[arg(short, long)] out: Option<String>,
    },
}
```

Handler sketch (place beside the other `Cmd::` arms; reuse the file's existing profile-loading and IR-writing helpers — do not duplicate them):

```rust
Cmd::Generate { what: GenerateCmd::Pocket { shape, x, y, width, height, cx, cy, radius,
    mode, tool_diameter, stepover, depth, depth_per_pass, z_top, safe_z, cut_feed,
    plunge_feed, profile, out } } => {
    let shape = match shape.as_str() {
        "rect" => {
            let (x, y, width, height) = (require(x, "--x")?, require(y, "--y")?,
                require(width, "--width")?, require(height, "--height")?);
            PocketShape::Rect { x, y, width, height }
        }
        _ => {
            let (cx, cy, radius) = (require(cx, "--cx")?, require(cy, "--cy")?,
                require(radius, "--radius")?);
            PocketShape::Circle { cx, cy, radius }
        }
    };
    let options = PocketOptions {
        shape,
        mode: if mode == "profile" { CutMode::Profile } else { CutMode::Pocket },
        tool_diameter, stepover, depth, depth_per_pass, z_top, safe_z, cut_feed, plunge_feed,
    };
    let design = try_pocket_design(&options).map_err(|e| anyhow::anyhow!("{e}"))?;
    let params = load_resolve_params_or_default(profile.as_deref())?; // reuse the existing helper name found in main.rs
    let toolpath = resolve_checked(&design, &params).map_err(|e| anyhow::anyhow!("{e}"))?;
    write_ir_json(&toolpath, out.as_deref())?; // reuse the existing helper name found in main.rs
}
```

(`require` is a two-line local helper turning `Option<f64>` + flag name into a clap-style error; if `main.rs` doesn't use `anyhow`, mirror whatever error type its arms return.)

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p dry-cli -- --nocapture`
Expected: all CLI tests PASS, including the three new ones.

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/cli/src/main.rs crates/cli/tests/cli.rs
git commit -m "feat(cli): dry generate pocket writes resolved IR for the CNC slice (P5.3, #179)"
```

---

### Task 7: E2E acceptance, conformance golden, docs

**Files:**
- Create: `crates/core/tests/cnc_pocket_e2e.rs`
- Create: `conformance/reports/cnc/pocket-rect-rs274.ngc` (golden) + a drift-gate test inside `cnc_pocket_e2e.rs`
- Modify: `docs/15-cli-cookbook.md` (recipe), `docs/04-tasks.md` (P5.3 note), `docs/16-support-matrix.md` (CNC row)
- Regenerate: `docs/site/reference/generated` (`cd docs/site && npm run reference`)

**Interfaces:**
- Consumes: everything from Tasks 1–6 (`pocket_design`, `CncFrame`, rs274 frame emit, CLI).
- Produces: the P5.3 acceptance evidence.

- [ ] **Step 1: Write the failing e2e test**

```rust
//! P5.3 acceptance: a pocket/profile emits a valid CNC program (#179).
use dry_core::{
    emit, resolve_checked, verify, CncFrame, Contracts, CutMode, EmitParams, FirmwareFlavor,
    PocketOptions, PocketShape, ResolveParams,
};

fn opts() -> PocketOptions {
    PocketOptions {
        shape: PocketShape::Rect { x: 0.0, y: 0.0, width: 60.0, height: 40.0 },
        mode: CutMode::Pocket,
        tool_diameter: 6.0,
        stepover: None,
        depth: 5.0,
        depth_per_pass: Some(2.5),
        z_top: Some(0.0),
        safe_z: Some(5.0),
        cut_feed: Some(300.0),
        plunge_feed: Some(100.0),
    }
}

/// Every word LinuxCNC's RS-274/NGC dialect documents for this program class.
const ALLOWED_WORDS: &[&str] = &["G0", "G1", "G2", "G3", "G4", "G17", "G21", "G54", "G55",
    "G56", "G57", "G58", "G59", "G90", "M3", "M5", "M6", "M8", "M9", "M30"];

fn word_is_allowed(tok: &str) -> bool {
    ALLOWED_WORDS.contains(&tok)
        || tok.starts_with('X') || tok.starts_with('Y') || tok.starts_with('Z')
        || tok.starts_with('I') || tok.starts_with('J') || tok.starts_with('F')
        || tok.starts_with('S') || tok.starts_with('T')
}

#[test]
fn pocket_emits_a_framed_parseable_rs274_program() {
    let design = dry_core::pocket_design(&opts());
    let tp = resolve_checked(&design, &ResolveParams::default()).unwrap();
    let report = verify(&tp, &Contracts::default());
    assert!(report.findings.iter().all(|f| !f.is_error()), "clean verify: {report:?}");

    let params = EmitParams {
        flavor: FirmwareFlavor::Rs274,
        cnc_frame: Some(CncFrame {
            wcs: Some(54), tool: Some(1), spindle_rpm: Some(10000.0), coolant: Some(false),
        }),
        ..EmitParams::default()
    };
    let lines = emit(&tp, &params);
    assert_eq!(lines[0], "G21 G17 G90");
    assert_eq!(lines.last().unwrap(), "M30");
    assert_eq!(lines.iter().filter(|l| *l == "M30").count(), 1);
    for line in &lines {
        for tok in line.split_whitespace() {
            assert!(word_is_allowed(tok), "word outside the RS-274 vocabulary: {tok} in {line}");
        }
    }
    assert!(!lines.iter().any(|l| l.contains(" E")), "no extruder words on rs274");
    assert!(lines.iter().any(|l| l.starts_with("G0")), "rapids present");
}

#[test]
fn circle_pocket_emits_arc_words() {
    let design = dry_core::pocket_design(&PocketOptions {
        shape: PocketShape::Circle { cx: 30.0, cy: 20.0, radius: 15.0 },
        ..opts()
    });
    let tp = resolve_checked(&design, &ResolveParams::default()).unwrap();
    let lines = emit(&tp, &EmitParams { flavor: FirmwareFlavor::Rs274, ..EmitParams::default() });
    assert!(lines.iter().any(|l| (l.starts_with("G2 ") || l.starts_with("G3 "))
        && l.contains("I") && l.contains("J")), "arc words with I/J offsets expected");
}

#[test]
fn golden_rect_pocket_program_does_not_drift() {
    let design = dry_core::pocket_design(&opts());
    let tp = resolve_checked(&design, &ResolveParams::default()).unwrap();
    let params = EmitParams {
        flavor: FirmwareFlavor::Rs274,
        cnc_frame: Some(CncFrame {
            wcs: Some(54), tool: Some(1), spindle_rpm: Some(10000.0), coolant: Some(false),
        }),
        ..EmitParams::default()
    };
    let program = emit(&tp, &params).join("\n") + "\n";
    let golden_path = concat!(env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/reports/cnc/pocket-rect-rs274.ngc");
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::create_dir_all(std::path::Path::new(golden_path).parent().unwrap()).unwrap();
        std::fs::write(golden_path, &program).unwrap();
    }
    let golden = std::fs::read_to_string(golden_path)
        .expect("golden exists — generate once with UPDATE_GOLDEN=1");
    assert_eq!(program, golden, "rs274 pocket output drifted from the frozen golden");
}
```

(Before writing, check how existing drift-gated goldens under `conformance/reports/` regenerate — if the repo has an established `UPDATE_GOLDEN`-style env or a generator script, use *that* mechanism instead of this one; the pattern above is the fallback.)

- [ ] **Step 2: Run to verify failure, then seed the golden**

Run: `cargo test -p dry-core --test cnc_pocket_e2e -- --nocapture`
Expected: first two tests PASS (Tasks 1–6 delivered them); golden test FAILS with "golden exists".
Then: `UPDATE_GOLDEN=1 cargo test -p dry-core --test cnc_pocket_e2e golden -- --nocapture` and re-run without the env → PASS.

- [ ] **Step 3: Docs**

- `docs/15-cli-cookbook.md`: add a "CNC pocket → RS-274" recipe — the exact three commands from Task 6's first test (generate / verify / emit with a `cnc.json` profile showing the `machine.cnc` block).
- `docs/04-tasks.md` P5.3: flip to `[x]` for the v0 scope with a one-line *Landed* note (generator + frame + acceptance test + golden) and point remaining CNC ambitions at #180/#181.
- `docs/16-support-matrix.md`: CNC row → `Experimental (rect/circle pocket+profile via dry generate pocket; RS-274 program frame from machine.cnc; not validated against a physical controller)`.
- Regenerate the reference docs (the drift gate!): `cd docs/site && npm run reference` and commit whatever changes under `docs/site/reference/generated`.

- [ ] **Step 4: Full workspace gate**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: everything green.

- [ ] **Step 5: Commit**

```bash
git add crates/core/tests/cnc_pocket_e2e.rs conformance/reports/cnc/ docs/
git commit -m "test(core): P5.3 acceptance — framed RS-274 pocket program, golden + docs (#179)"
```

- [ ] **Step 6: Manual validation note** — load `conformance/reports/cnc/pocket-rect-rs274.ngc` into a LinuxCNC sim (or the linuxcnc.org online sim instructions) and record the outcome in the PR description. Not CI; documented evidence only.

---

## Self-review notes

- **Spec coverage:** §3.1 generator → Tasks 1–3; §3.2 frame → Tasks 4–5; §3.3 CLI → Task 6; §5 testing/golden/docs → Task 7; §4 error handling → Tasks 1, 4, 6. Spec's "word set ⊆ LinuxCNC vocabulary" → Task 7 `word_is_allowed`.
- **Type consistency:** `PocketShape/CutMode/PocketOptions/PocketError/CncFrame` names and field lists are identical across Tasks 1, 3, 4, 5, 6, 7. `try_pocket_ops`→`passes`→`rect_passes`/`circle_passes` call chain is defined once (Task 2/3).
- **Known judgment calls left to the implementer (explicitly):** exact XY-move count in Task 3's profile test (derive from the Task-2 code and comment it); reuse-not-duplicate of the CLI profile/IR helpers (Task 6); the repo's established golden-regen mechanism (Task 7).

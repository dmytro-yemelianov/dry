//! Gyroid TPMS infill generator (`docs/01-architecture.md` §1).
//!
//! A triply-periodic minimal surface (TPMS) is the zero level-set of a periodic implicit field. This
//! generator slices a selectable TPMS field (default **gyroid**, `sin(x)·cos(y) + sin(y)·cos(z) +
//! sin(z)·cos(x)`), with the world coordinates scaled by `2π / cell_size`, into horizontal layers,
//! extracts the iso-contours per layer with marching squares, stitches the raw line segments into
//! continuous polylines, orders them
//! nearest-neighbour, and emits travel/extrude [`Op`]s. The result is an ordinary L1 [`Design`]: it
//! lowers through [`crate::resolve`] like any hand-authored design.
//!
//! This is a clean-room port of the gyroid path in the TypeScript SDK's `generators/tpms.ts`. The math
//! uses [`libm`] (`sin`/`cos`) for native+wasm determinism, so the output differs sub-micron from the
//! JS-`Math` reference; there is **no** byte-identity contract between the two — correctness is validated
//! by geometric invariants (finite, in-bounds, balanced extruder state, verifies, deposits material).
//!
//! All ten surfaces from the TS SDK are implemented (see [`Surface`]); the PyO3 exposure and the
//! TS-SDK delegation (for P4 cross-SDK identity) remain deferred follow-ups.

use crate::resolve::{Design, Op};
use std::collections::HashMap;

/// Geometric tolerance shared with the TS reference: values inside `±EPS` are treated as on the surface.
const EPS: f64 = 1e-9;
/// Endpoints/coordinates are quantised to this many decimal places when stitching and when emitted.
const QUANTUM: f64 = 1e6;
/// Default base layer height (mm).
const DEFAULT_LAYER_HEIGHT: f64 = 0.28;
const DEFAULT_ADAPTIVE_MIN_LAYER_HEIGHT: f64 = 0.14;
const DEFAULT_ADAPTIVE_MAX_LAYER_HEIGHT: f64 = 0.32;
const DEFAULT_ADAPTIVE_MAX_LENGTH_DELTA: f64 = 0.35;
const DEFAULT_ADAPTIVE_MAX_POINT_DELTA: f64 = 0.45;
const DEFAULT_ADAPTIVE_MAX_DEPTH: u32 = 4;
/// Adaptive refinement bisects, so depth is an exponent: at 16 one base interval could carry 65,535
/// inserted layers. Past this the value is not a slicing request.
const MAX_ADAPTIVE_MAX_DEPTH: u32 = 16;
/// Field-sample budget guardrail for interactive/untrusted use.
const DEFAULT_MAX_FIELD_SAMPLES: f64 = 6_000_000.0;
/// The top base layer is clamped to the block height, so it gets whatever Z remains. A remainder
/// below this fraction of the nominal layer height is a slicing artifact rather than a printable
/// layer (1% of a 0.2 mm layer is 2 µm) and is merged into the layer below instead of stacked on it.
///
/// It stays a slicing rule now that every layer declares the height it occupies, because an honest
/// declaration does not make a degenerate layer emittable. The remainder has no lower bound, and
/// when it is smaller than half the coordinate step (`1 / QUANTUM`, so 5e-7 mm) the appended top Z
/// *rounds onto the Z of the layer below*: the two layers land at the same coordinate, the gap is
/// `0.0`, the declaration is `0.0`, and `resolve` refuses the whole program (`require_positive`,
/// `crates/core/src/resolve.rs` — `height must be > 0`). That needs a block height off the layer
/// grid to reach — measured with this constant deleted, `cellSize 12.0000003, layerHeight 2.0`
/// emits declared heights `[2.0, 0.0]` and is refused. A layer height with a sub-step remainder is
/// *not* a trigger (`layerHeight 1.99999995` on a 12 mm block emits `[1.99999995, 2.0]` and
/// resolves): every intermediate Z is rounded as it is pushed, so `zs.last()` already equals
/// `height` and the clamp branch never runs. Well above that floor the layer is still a full
/// re-trace of the contour beneath it at a bead no machine can lay down, so refusing to stack it is
/// a slicing decision, not a numeric compensation. Redistributing the remainder across all layers
/// instead (which would make this constant unnecessary) is the principled alternative and was
/// considered and declined for #187 — it is a slicing-policy change, not a declaration fix.
/// Pinned at both boundaries by `the_degenerate_top_layer_threshold_is_one_percent`.
const DEGENERATE_TOP_LAYER_FRACTION: f64 = 0.01;

/// A triply-periodic minimal surface, selecting which nodal field [`Tpms`] slices. The JSON wire form
/// is kebab-case (matching the TS SDK), e.g. `"schwarz-p"` or `"fischer-koch-s"`. Each formula is a
/// clean-room port of the corresponding entry in the TS SDK's `generators/tpms.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Surface {
    #[default]
    Gyroid,
    SchwarzP,
    SchwarzD,
    Iwp,
    Neovius,
    FischerKochS,
    FischerKochY,
    Frd,
    Lidinoid,
    SplitP,
}

impl Surface {
    /// The kebab-case wire name, so an error quotes the surface the way the caller spelled it.
    fn wire_name(self) -> &'static str {
        match self {
            Surface::Gyroid => "gyroid",
            Surface::SchwarzP => "schwarz-p",
            Surface::SchwarzD => "schwarz-d",
            Surface::Iwp => "iwp",
            Surface::Neovius => "neovius",
            Surface::FischerKochS => "fischer-koch-s",
            Surface::FischerKochY => "fischer-koch-y",
            Surface::Frd => "frd",
            Surface::Lidinoid => "lidinoid",
            Surface::SplitP => "split-p",
        }
    }
}

/// Options for [`tpms_ops`] / [`tpms_design`]. Every field is optional; an omitted field takes the
/// documented default. The JSON wire form is camelCase (matching the TS SDK), e.g. `{"cellSize":12}`.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TpmsOptions {
    /// The surface to slice; omitted defaults to [`Surface::Gyroid`]. Unknown names are a deserialize
    /// error (the binding surfaces it as a structured failure, never a panic).
    pub surface: Option<Surface>,
    /// Isosurface value: the contour traced is `field(x,y,z) = iso_level`.
    pub iso_level: Option<f64>,
    /// Cubic unit-cell size in mm.
    pub cell_size: Option<f64>,
    pub cells_x: Option<u32>,
    pub cells_y: Option<u32>,
    pub cells_z: Option<u32>,
    /// XY marching-squares resolution per unit cell (≥ 4).
    pub samples_per_cell: Option<u32>,
    pub layer_height: Option<f64>,
    pub z0: Option<f64>,
    pub center_x: Option<f64>,
    pub center_y: Option<f64>,
    pub bead_width: Option<f64>,
    pub bead_height: Option<f64>,
    pub nozzle_temp: Option<f64>,
    pub print_speed: Option<f64>,
    pub flow: Option<f64>,
    /// Phase offsets in unit-cell periods (move seams/features without moving the part).
    pub phase_x: Option<f64>,
    pub phase_y: Option<f64>,
    pub phase_z: Option<f64>,
    /// Add a single-wall rectangular perimeter around every sliced layer.
    pub perimeter: Option<bool>,
    pub perimeter_inset: Option<f64>,
    /// Drop stitched contours shorter than this. Defaults to one grid cell.
    pub min_path_length: Option<f64>,
    /// Insert extra Z slices where intervals are tall or change topology sharply.
    pub adaptive: Option<bool>,
    pub adaptive_min_layer_height: Option<f64>,
    pub adaptive_max_layer_height: Option<f64>,
    pub adaptive_max_length_delta: Option<f64>,
    pub adaptive_max_point_delta: Option<f64>,
    pub adaptive_max_depth: Option<u32>,
    /// Field-sample guardrail for interactive use; defaults to 6,000,000.
    ///
    /// **`0` means unlimited.** That is the sentinel `sdk/ts` encodes `Infinity` as
    /// (`sdk/ts/src/generators/tpms.ts`), so it is a wire contract, not an accident: trusted offline
    /// generation passes `0` rather than a large finite value. A non-finite value is also treated as
    /// unlimited. `NaN` and negative values are **refused** — nobody intends either, and accepting
    /// them silently switched the guardrail off.
    ///
    /// Accepted residual, recorded deliberately (H1.4 T4b): a caller who can send `0` can still
    /// disable the guard, and on wasm that is any browser page passing through user option JSON.
    /// That is a documented hole, not an oversight; revisit if the browser surface ever accepts
    /// untrusted option JSON.
    pub max_field_samples: Option<f64>,
}

/// An invalid-options error reported before any geometry is produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TpmsError {
    message: String,
}

impl TpmsError {
    fn new(message: impl Into<String>) -> Self {
        TpmsError {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for TpmsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TpmsError {}

// ---- validation helpers (mirror the TS guards) ----

fn finite(name: &str, value: f64) -> Result<f64, TpmsError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(TpmsError::new(format!("{name} must be finite")))
    }
}

fn positive(name: &str, value: f64) -> Result<f64, TpmsError> {
    finite(name, value)?;
    if value > 0.0 {
        Ok(value)
    } else {
        Err(TpmsError::new(format!("{name} must be > 0")))
    }
}

fn positive_or_zero(name: &str, value: f64) -> Result<f64, TpmsError> {
    finite(name, value)?;
    if value >= 0.0 {
        Ok(value)
    } else {
        Err(TpmsError::new(format!("{name} must be >= 0")))
    }
}

fn at_least(name: &str, value: u32, min: u32) -> Result<u32, TpmsError> {
    if value >= min {
        Ok(value)
    } else {
        Err(TpmsError::new(format!(
            "{name} must be an integer >= {min}"
        )))
    }
}

/// Round to the shared quantum (`1e-6`), matching the TS `round` helper.
fn round(value: f64) -> f64 {
    (value * QUANTUM).round() / QUANTUM
}

#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

struct Seg2 {
    a: Point,
    b: Point,
}

#[derive(Clone)]
struct Path {
    points: Vec<Point>,
}

#[derive(Clone)]
struct LayerSlice {
    z_local: f64,
    paths: Vec<Path>,
    path_count: usize,
    point_count: usize,
    length: f64,
    /// Contours stitched for this layer *before* the `min_path_length` filter, kept so a program that
    /// emits nothing can name whichever option emptied it (see [`Tpms::reject_vacuous`]).
    raw_path_count: usize,
    /// Length of the longest pre-filter contour, so the rejection can say what to lower the filter to.
    longest_raw_length: f64,
}

#[derive(Clone, Copy)]
struct AdaptiveOptions {
    min_layer_height: f64,
    max_layer_height: f64,
    max_length_delta: f64,
    max_point_delta: f64,
    max_depth: u32,
}

/// The resolved, validated generator: every option folded into a concrete number plus derived sizes.
struct Tpms {
    surface: Surface,
    iso_level: f64,
    cell_size: f64,
    layer_height: f64,
    z0: f64,
    center_x: f64,
    center_y: f64,
    bead_width: f64,
    bead_height: f64,
    nozzle_temp: f64,
    print_speed: f64,
    flow: f64,
    phase_x: f64,
    phase_y: f64,
    phase_z: f64,
    perimeter: bool,
    perimeter_inset: f64,
    min_path_length: f64,
    /// Whether the caller supplied `minPathLength`, or it was derived from the sample spacing.
    /// Only used to attribute blame in the vacuity error — naming a value nobody typed sends the
    /// user to the wrong knob (#189).
    min_path_length_explicit: bool,
    adaptive: Option<AdaptiveOptions>,
    // derived
    width: f64,
    depth: f64,
    height: f64,
    nx: usize,
    ny: usize,
    dx: f64,
    dy: f64,
}

impl Tpms {
    fn resolve(options: &TpmsOptions) -> Result<Tpms, TpmsError> {
        let surface = options.surface.unwrap_or_default();

        let iso_level = finite("isoLevel", options.iso_level.unwrap_or(0.0))?;
        let cell_size = positive("cellSize", options.cell_size.unwrap_or(12.0))?;
        let cells_x = at_least("cellsX", options.cells_x.unwrap_or(2), 1)?;
        let cells_y = at_least("cellsY", options.cells_y.unwrap_or(2), 1)?;
        let cells_z = at_least("cellsZ", options.cells_z.unwrap_or(2), 1)?;
        let samples_per_cell =
            at_least("samplesPerCell", options.samples_per_cell.unwrap_or(18), 4)?;
        let layer_height = positive(
            "layerHeight",
            options.layer_height.unwrap_or(DEFAULT_LAYER_HEIGHT),
        )?;
        let z0 = positive_or_zero("z0", options.z0.unwrap_or(0.2))?;
        let bead_width = positive("beadWidth", options.bead_width.unwrap_or(0.45))?;
        let bead_height = positive("beadHeight", options.bead_height.unwrap_or(layer_height))?;
        let center_x = finite("centerX", options.center_x.unwrap_or(50.0))?;
        let center_y = finite("centerY", options.center_y.unwrap_or(50.0))?;
        let nozzle_temp = positive("nozzleTemp", options.nozzle_temp.unwrap_or(210.0))?;
        let print_speed = positive("printSpeed", options.print_speed.unwrap_or(1200.0))?;
        let flow = positive("flow", options.flow.unwrap_or(1.0))?;
        let phase_x = finite("phaseX", options.phase_x.unwrap_or(0.0))?;
        let phase_y = finite("phaseY", options.phase_y.unwrap_or(0.0))?;
        let phase_z = finite("phaseZ", options.phase_z.unwrap_or(0.0))?;
        let perimeter = options.perimeter.unwrap_or(false);
        let adaptive_on = options.adaptive.unwrap_or(false);

        let adaptive = if adaptive_on {
            let min_layer_height = positive(
                "adaptiveMinLayerHeight",
                options
                    .adaptive_min_layer_height
                    .unwrap_or_else(|| layer_height.min(DEFAULT_ADAPTIVE_MIN_LAYER_HEIGHT)),
            )?;
            let max_layer_height = positive(
                "adaptiveMaxLayerHeight",
                options
                    .adaptive_max_layer_height
                    .unwrap_or_else(|| layer_height.min(DEFAULT_ADAPTIVE_MAX_LAYER_HEIGHT)),
            )?;
            let max_length_delta = positive(
                "adaptiveMaxLengthDelta",
                options
                    .adaptive_max_length_delta
                    .unwrap_or(DEFAULT_ADAPTIVE_MAX_LENGTH_DELTA),
            )?;
            let max_point_delta = positive(
                "adaptiveMaxPointDelta",
                options
                    .adaptive_max_point_delta
                    .unwrap_or(DEFAULT_ADAPTIVE_MAX_POINT_DELTA),
            )?;
            let max_depth = options
                .adaptive_max_depth
                .unwrap_or(DEFAULT_ADAPTIVE_MAX_DEPTH);
            // The only integer option with no validation until now. Refinement bisects, so depth is
            // an exponent: at 16 a single base interval could carry 65,535 inserted layers. Anything
            // beyond that is not a slicing request.
            if max_depth > MAX_ADAPTIVE_MAX_DEPTH {
                return Err(TpmsError::new(format!(
                    "adaptiveMaxDepth {max_depth} exceeds the maximum of {MAX_ADAPTIVE_MAX_DEPTH}: \
                     each level halves the layer spacing, so depth is an exponent"
                )));
            }
            if min_layer_height - max_layer_height > EPS {
                return Err(TpmsError::new(
                    "adaptiveMinLayerHeight must be <= adaptiveMaxLayerHeight",
                ));
            }
            Some(AdaptiveOptions {
                min_layer_height,
                max_layer_height,
                max_length_delta,
                max_point_delta,
                max_depth,
            })
        } else {
            None
        };

        let width = cells_x as f64 * cell_size;
        let depth = cells_y as f64 * cell_size;
        let height = cells_z as f64 * cell_size;
        let nx = cells_x as usize * samples_per_cell as usize;
        let ny = cells_y as usize * samples_per_cell as usize;

        // `0` is the documented "unlimited" sentinel and a non-finite value is treated the same, but
        // NaN and negative are refused: they were silently switching the guardrail off, and the raw
        // JSON path wasm and PyO3 use makes that reachable from untrusted input.
        if let Some(max) = options.max_field_samples {
            if max.is_nan() || max < 0.0 {
                return Err(TpmsError::new(format!(
                    "maxFieldSamples {max} must not be negative or NaN (use 0 for unlimited)"
                )));
            }
        }
        assert_budget(
            nx,
            ny,
            estimate_layer_count(height, layer_height, adaptive.as_ref()),
            options.max_field_samples,
        )?;

        let dx = width / nx as f64;
        let dy = depth / ny as f64;
        let min_path_length = positive_or_zero(
            "minPathLength",
            options.min_path_length.unwrap_or(dx.min(dy)),
        )?;
        let perimeter_inset = positive_or_zero(
            "perimeterInset",
            options.perimeter_inset.unwrap_or(bead_width),
        )?;
        // ADR 0002 §4: refuse, do not clamp. The old `.min((width/2 - EPS).max(0.0))` turned an
        // oversized inset into a rectangle spanning 2e-9 mm — 44 zero-length extrusions presented as
        // a perimeter wall. The inset is only read when `perimeter` is on, so only gate it there.
        //
        // The gate must be on the *surviving span*, not on an exact `2·inset >= width` boundary. An
        // inset one epsilon below that boundary passed while leaving a rectangle far below the 1e-6
        // emission grid, which rounds to a point: measured on a 12 mm block at `perimeterInset:
        // 5.9999999`, 176 coincident extruding moves with zero extruded length and a clean `verify`
        // — the exact artifact the gate was added to remove (#189).
        if perimeter {
            let span_x = width - 2.0 * perimeter_inset;
            let span_y = depth - 2.0 * perimeter_inset;
            let min_span = 1.0 / QUANTUM;
            if span_x < min_span || span_y < min_span {
                // `perimeterInset` defaults to `beadWidth`, so a caller who never set it can still
                // land here — on any block narrower than two beads. Name the option they actually
                // typed, or say the value was derived and from where.
                let source = if options.perimeter_inset.is_some() {
                    String::new()
                } else {
                    format!(" (defaulted from beadWidth {bead_width}, not set by you)")
                };
                return Err(TpmsError::new(format!(
                    "perimeterInset {perimeter_inset}{source} collapses the perimeter rectangle to \
                     {:.9} x {:.9} mm, below the {min_span} mm emission grid: it must leave a span \
                     of at least {min_span} mm in both width ({width}) and depth ({depth})",
                    span_x.max(0.0),
                    span_y.max(0.0)
                )));
            }
        }

        Ok(Tpms {
            surface,
            iso_level,
            cell_size,
            layer_height,
            z0,
            center_x,
            center_y,
            bead_width,
            bead_height,
            nozzle_temp,
            print_speed,
            flow,
            phase_x,
            phase_y,
            phase_z,
            perimeter,
            perimeter_inset,
            min_path_length,
            min_path_length_explicit: options.min_path_length.is_some(),
            adaptive,
            width,
            depth,
            height,
            nx,
            ny,
            dx,
            dy,
        })
    }

    /// The selected surface's nodal field, scaled so one period spans one unit cell. Each arm is a
    /// clean-room port of the matching `field` in the TS SDK's `generators/tpms.ts`.
    #[inline]
    fn field(&self, x: f64, y: f64, z: f64) -> f64 {
        use libm::{cos, sin};
        match self.surface {
            Surface::Gyroid => sin(x) * cos(y) + sin(y) * cos(z) + sin(z) * cos(x),
            Surface::SchwarzP => cos(x) + cos(y) + cos(z),
            Surface::SchwarzD => {
                sin(x) * sin(y) * sin(z)
                    + sin(x) * cos(y) * cos(z)
                    + cos(x) * sin(y) * cos(z)
                    + cos(x) * cos(y) * sin(z)
            }
            Surface::Iwp => {
                2.0 * (cos(x) * cos(y) + cos(y) * cos(z) + cos(z) * cos(x))
                    - (cos(2.0 * x) + cos(2.0 * y) + cos(2.0 * z))
            }
            Surface::Neovius => 3.0 * (cos(x) + cos(y) + cos(z)) + 4.0 * cos(x) * cos(y) * cos(z),
            Surface::FischerKochS => {
                cos(2.0 * x) * sin(y) * cos(z)
                    + cos(2.0 * y) * sin(z) * cos(x)
                    + cos(2.0 * z) * sin(x) * cos(y)
            }
            Surface::FischerKochY => {
                cos(x) * cos(y) * cos(z)
                    + sin(x) * sin(y) * sin(z)
                    + sin(2.0 * x) * sin(y)
                    + sin(2.0 * y) * sin(z)
                    + sin(x) * sin(2.0 * z)
                    + sin(2.0 * x) * cos(z)
                    + cos(x) * sin(2.0 * y)
                    + cos(y) * sin(2.0 * z)
            }
            Surface::Frd => {
                4.0 * cos(x) * cos(y) * cos(z)
                    - (cos(2.0 * x) * cos(2.0 * y)
                        + cos(2.0 * y) * cos(2.0 * z)
                        + cos(2.0 * z) * cos(2.0 * x))
            }
            Surface::Lidinoid => {
                sin(2.0 * x) * cos(y) * sin(z)
                    + sin(2.0 * y) * cos(z) * sin(x)
                    + sin(2.0 * z) * cos(x) * sin(y)
                    - cos(2.0 * x) * cos(2.0 * y)
                    - cos(2.0 * y) * cos(2.0 * z)
                    - cos(2.0 * z) * cos(2.0 * x)
                    + 0.3
            }
            Surface::SplitP => {
                1.1 * (sin(2.0 * x) * sin(z) * cos(y)
                    + sin(2.0 * y) * sin(x) * cos(z)
                    + sin(2.0 * z) * sin(y) * cos(x))
                    - 0.2
                        * (cos(2.0 * x) * cos(2.0 * y)
                            + cos(2.0 * y) * cos(2.0 * z)
                            + cos(2.0 * z) * cos(2.0 * x))
                    - 0.4 * (cos(2.0 * x) + cos(2.0 * y) + cos(2.0 * z))
            }
        }
    }

    fn emit(&self) -> Result<Vec<Op>, TpmsError> {
        let mut ops = vec![
            Op::Geometry {
                width: Some(self.bead_width),
                height: Some(self.bead_height),
            },
            Op::Temperature {
                nozzle: self.nozzle_temp,
            },
            Op::Speed {
                print: self.print_speed,
            },
        ];
        if (self.flow - 1.0).abs() > EPS {
            ops.push(Op::Flow { ratio: self.flow });
        }

        let slices = self.build_layer_slices();
        self.reject_vacuous(&slices)?;
        let mut previous_local: Option<Point> = None;
        // `resolve` reads `Op::Geometry`'s height as the deposited bead (`length × width × height ×
        // flow`), so the declaration has to track the gap each layer actually occupies — on every
        // path, not just the adaptive one. Adaptive refinement is the loud case (measured gaps of
        // [0.05, 0.1, 0.2, 0.4] against a single `height: 0.4` is 8x over-extrusion on the thinnest
        // layer), but the clamped top layer is the common one: at the generator defaults it spans
        // 0.2 mm and used to declare 0.28. No verifier can see either, because the IR records the
        // wrong bead self-consistently. A uniform job still declares once, so the common path only
        // gains the one op its clamped layer needs — at a `layerHeight` on the 1e-6 mm coordinate
        // grid. One off it (e.g. 1/3) makes the quantized gap alternate, so most layers re-declare:
        // 2463 -> 2488 ops on a 12 mm block. That is honest, and a tolerance to suppress it would
        // reintroduce the declared-vs-actual divergence this tracking exists to remove.
        let mut previous_z: Option<f64> = None;
        let mut skipped_since_deposit = false;
        let mut declared_height = self.bead_height;
        for slice in slices {
            let z = round(self.z0 + slice.z_local);
            // A layer with no contours emits nothing (as before), so it is also not the layer the
            // next bead rests on — the gap is measured against the last layer that deposited, and
            // the skip is remembered because a gap that spans nothing deposited is not a bead.
            if !self.perimeter && slice.paths.is_empty() {
                skipped_since_deposit = true;
                continue;
            }
            if let Some(previous) = previous_z {
                // The gap is only a bead height when it is material actually stacked beneath. When
                // the intervening layers traced nothing, the nozzle still lays one bead onto
                // whatever is below — it does not extrude a layer-tall column into air — so such a
                // layer declares the nominal bead (`self.bead_height` is exactly what
                // `declared_bead_height` yields for a nominal gap). Measured at `neovius,
                // isoLevel 1.0, layerHeight 0.2`: the layer after fifteen empty ones spans 3.2 mm,
                // and declaring the span inflated the job 47.27 -> 56.73 mm of filament (+20%).
                let declared = if skipped_since_deposit {
                    self.bead_height
                } else {
                    self.declared_bead_height(round(z - previous))
                };
                if declared != declared_height {
                    ops.push(Op::Geometry {
                        width: Some(self.bead_width),
                        height: Some(declared),
                    });
                    declared_height = declared;
                }
            }
            previous_z = Some(z);
            skipped_since_deposit = false;
            if self.perimeter {
                let rect_local = rectangle_path(self.width, self.depth, self.perimeter_inset);
                let rect: Vec<Point> = rect_local.iter().map(|p| self.to_world(*p)).collect();
                append_path(&mut ops, &rect, z);
                previous_local = rect_local.last().copied();
            }
            let paths = order_paths(slice.paths, previous_local);
            for path in &paths {
                let world: Vec<Point> = path.points.iter().map(|p| self.to_world(*p)).collect();
                append_path(&mut ops, &world, z);
                previous_local = path.points.last().copied();
            }
        }
        ops.push(Op::Extruder { on: false });
        Ok(ops)
    }

    /// The bead height to declare for a layer that occupies `gap` mm of Z.
    ///
    /// `beadHeight` is preserved as a *ratio* rather than as an absolute: a layer that occupies half
    /// the nominal `layerHeight` declares half the configured bead. That keeps a deliberate squish
    /// (`beadHeight` ≠ `layerHeight`, a documented option — `docs/07-tpms-codegen.md`) alive on every
    /// layer instead of discarding it from the second one onward, which is what declaring the raw gap
    /// did: `layerHeight 0.4, beadHeight 0.5, adaptive` emitted declared heights `[0.5, 0.2]`.
    ///
    /// A nominal-height layer therefore declares exactly the configured `beadHeight` — including the
    /// first emitted layer, which is not a special case but the same rule applied to its nominal
    /// gap. It has no layer beneath it to measure against, `resolve` has no first-layer convention of
    /// its own to match, and `z0` is the nozzle Z rather than a guaranteed plate gap, so deriving a
    /// gap from `z0` would be inventing a convention.
    fn declared_bead_height(&self, gap: f64) -> f64 {
        if gap == self.layer_height {
            self.bead_height
        } else {
            round(gap * self.bead_height / self.layer_height)
        }
    }

    /// Refuse an option set that traces no contour on any layer (ADR 0002 §4: refuse, never silently
    /// emit nothing). Such a program resolves, verifies with zero findings and simulates to zero
    /// volume — a file that heats the nozzle and prints nothing. The `resolve` postcondition cannot
    /// see it: an empty toolpath is perfectly finite.
    ///
    /// A perimeter deposits material on its own, so it exempts the program from the check.
    fn reject_vacuous(&self, slices: &[LayerSlice]) -> Result<(), TpmsError> {
        // Scope, stated so it is not over-credited (#189): this is a *structural* test — does any
        // contour survive — not a magnitude one. It does not promise a useful amount of material:
        // `split-p` at `isoLevel 1.75` with `minPathLength 0` passes here and deposits 0.021 mm over
        // the whole part. Requiring a minimum extruded length would make acceptance depend on a
        // magnitude threshold, which ADR 0002 §2 declines to adopt. Documented in `docs/07`.
        //
        // The perimeter exemption is conditional on the rectangle actually having length. It used to
        // short-circuit on `self.perimeter` alone, so a perimeter of literally zero length exempted
        // the whole program from the vacuity check (#189). The span gate in `resolve` now refuses
        // that case up front; this keeps the exemption honest if the two ever drift apart.
        let perimeter_has_length = self.perimeter
            && (self.width - 2.0 * self.perimeter_inset) >= 1.0 / QUANTUM
            && (self.depth - 2.0 * self.perimeter_inset) >= 1.0 / QUANTUM;
        if perimeter_has_length || slices.iter().any(|s| s.path_count > 0) {
            return Ok(());
        }
        // Which option emptied the program is decided by the pre-filter contour count, not guessed:
        // contours that existed and were dropped implicate `minPathLength`; no contours at all means
        // `isoLevel` sits outside this surface's field range (which is surface-dependent, so a
        // computed check beats a per-surface table).
        let raw: usize = slices.iter().map(|s| s.raw_path_count).sum();
        if raw > 0 {
            let longest = slices
                .iter()
                .map(|s| s.longest_raw_length)
                .fold(0.0_f64, f64::max);
            if self.min_path_length_explicit {
                return Err(TpmsError::new(format!(
                    "minPathLength {} dropped all {raw} contours (longest was {longest}), so the \
                     program would deposit no material. Lower minPathLength or raise \
                     samplesPerCell.",
                    self.min_path_length
                )));
            }
            // The filter did the dropping, but the caller never chose its value — it is derived from
            // the sample spacing. Telling them to lower a number they did not set sent them to the
            // wrong knob on 7 of 10 surfaces, always in the narrow band at the edge of the field
            // range, which is exactly where someone tuning `isoLevel` lands (#189). Following that
            // advice did not help: the contours that survive are microscopic either way.
            return Err(TpmsError::new(format!(
                "isoLevel {} sits at the edge of the {} field's range: the {raw} contours it traces \
                 are shorter (longest {longest}) than the default minPathLength {} derived from the \
                 sample spacing, so the program would deposit no material. Move isoLevel toward the \
                 middle of the range, or raise samplesPerCell to resolve finer contours.",
                self.iso_level,
                self.surface.wire_name(),
                self.min_path_length
            )));
        }
        Err(TpmsError::new(format!(
            "isoLevel {} is outside the {} field's range: no layer traces a contour, so the program \
             would deposit no material.",
            self.iso_level,
            self.surface.wire_name()
        )))
    }

    /// Map a layer-local point (origin at the slice's lower-left) into world coordinates.
    #[inline]
    fn to_world(&self, p: Point) -> Point {
        Point {
            x: p.x - self.width / 2.0 + self.center_x,
            y: p.y - self.depth / 2.0 + self.center_y,
        }
    }

    fn build_layer_slices(&self) -> Vec<LayerSlice> {
        let base_z = self.base_layer_zs();
        let mut slices: Vec<LayerSlice> = Vec::with_capacity(base_z.len());
        slices.push(self.build_layer(base_z[0]));
        for &z in &base_z[1..] {
            let b = self.build_layer(z);
            if let Some(adapt) = &self.adaptive {
                let mut inserted = Vec::new();
                // `a` is the most recently committed slice.
                let a = slices.last().expect("at least one slice");
                self.refine_adaptive(a, &b, adapt, &mut inserted, 0);
                slices.extend(inserted);
            }
            slices.push(b);
        }
        slices
    }

    fn base_layer_zs(&self) -> Vec<f64> {
        let mut zs = vec![0.0];
        let mut z = self.layer_height;
        while z < self.height - EPS {
            zs.push(round(z));
            z += self.layer_height;
        }
        if self.height > EPS && (zs[zs.len() - 1] - self.height).abs() > EPS {
            let last = zs.len() - 1;
            // The clamped top layer gets whatever Z remains. When that remainder is a sliver —
            // `cellSize: 12, cellsZ: 1, layerHeight: 1.9999` leaves 0.6 µm — stacking it puts two
            // layers 0.0006 mm apart, each extruding a full 1.9999 mm bead. Merge it into the layer
            // below instead: the block still reaches its full height, without a second bead for it.
            if last > 0
                && self.height - zs[last] < DEGENERATE_TOP_LAYER_FRACTION * self.layer_height
            {
                zs[last] = round(self.height);
            } else {
                zs.push(round(self.height));
            }
        }
        zs
    }

    fn refine_adaptive(
        &self,
        a: &LayerSlice,
        b: &LayerSlice,
        opt: &AdaptiveOptions,
        out: &mut Vec<LayerSlice>,
        depth: u32,
    ) {
        if !needs_adaptive_layer(a, b, opt, depth) {
            return;
        }
        let mid_z = round((a.z_local + b.z_local) / 2.0);
        if mid_z - a.z_local < opt.min_layer_height - EPS
            || b.z_local - mid_z < opt.min_layer_height - EPS
        {
            return;
        }
        let mid = self.build_layer(mid_z);
        self.refine_adaptive(a, &mid, opt, out, depth + 1);
        out.push(mid.clone());
        self.refine_adaptive(&mid, b, opt, out, depth + 1);
    }

    fn build_layer(&self, z_local: f64) -> LayerSlice {
        let segments = self.marching_squares_layer(z_local);
        let stitched: Vec<Path> = stitch_segments(&segments)
            .into_iter()
            .filter(|p| p.points.len() >= 2)
            .collect();
        let raw_path_count = stitched.len();
        let longest_raw_length = stitched
            .iter()
            .map(|p| path_length(&p.points))
            .fold(0.0_f64, f64::max);
        let paths: Vec<Path> = stitched
            .into_iter()
            .filter(|p| path_length(&p.points) >= self.min_path_length)
            .collect();
        layer_slice(z_local, paths, raw_path_count, longest_raw_length)
    }

    fn marching_squares_layer(&self, z_local: f64) -> Vec<Seg2> {
        let nx = self.nx;
        let ny = self.ny;
        let stride = nx + 1;
        let zc = (z_local / self.cell_size + self.phase_z) * std::f64::consts::TAU;

        let mut values = vec![0.0_f64; stride * (ny + 1)];
        for j in 0..=ny {
            let y = (j as f64 * self.dy / self.cell_size + self.phase_y) * std::f64::consts::TAU;
            for i in 0..=nx {
                let x =
                    (i as f64 * self.dx / self.cell_size + self.phase_x) * std::f64::consts::TAU;
                values[j * stride + i] = self.field(x, y, zc) - self.iso_level;
            }
        }
        let value_at = |i: usize, j: usize| values[j * stride + i];

        let mut segments = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                let p00 = Point {
                    x: i as f64 * self.dx,
                    y: j as f64 * self.dy,
                };
                let p10 = Point {
                    x: (i + 1) as f64 * self.dx,
                    y: j as f64 * self.dy,
                };
                let p11 = Point {
                    x: (i + 1) as f64 * self.dx,
                    y: (j + 1) as f64 * self.dy,
                };
                let p01 = Point {
                    x: i as f64 * self.dx,
                    y: (j + 1) as f64 * self.dy,
                };
                let v00 = scrub_zero(value_at(i, j));
                let v10 = scrub_zero(value_at(i + 1, j));
                let v11 = scrub_zero(value_at(i + 1, j + 1));
                let v01 = scrub_zero(value_at(i, j + 1));

                let mut crossings: Vec<Point> = Vec::with_capacity(4);
                if crosses(v00, v10) {
                    crossings.push(interpolate(p00, p10, v00, v10));
                }
                if crosses(v10, v11) {
                    crossings.push(interpolate(p10, p11, v10, v11));
                }
                if crosses(v11, v01) {
                    crossings.push(interpolate(p11, p01, v11, v01));
                }
                if crosses(v01, v00) {
                    crossings.push(interpolate(p01, p00, v01, v00));
                }

                if crossings.len() == 2 {
                    segments.push(Seg2 {
                        a: crossings[0],
                        b: crossings[1],
                    });
                } else if crossings.len() == 4 {
                    let xc = ((i as f64 + 0.5) * self.dx / self.cell_size + self.phase_x)
                        * std::f64::consts::TAU;
                    let yc = ((j as f64 + 0.5) * self.dy / self.cell_size + self.phase_y)
                        * std::f64::consts::TAU;
                    if self.field(xc, yc, zc) - self.iso_level >= 0.0 {
                        segments.push(Seg2 {
                            a: crossings[0],
                            b: crossings[1],
                        });
                        segments.push(Seg2 {
                            a: crossings[2],
                            b: crossings[3],
                        });
                    } else {
                        segments.push(Seg2 {
                            a: crossings[0],
                            b: crossings[3],
                        });
                        segments.push(Seg2 {
                            a: crossings[1],
                            b: crossings[2],
                        });
                    }
                }
            }
        }
        segments
    }
}

/// An upper bound on the number of layers the slicer can produce.
///
/// Adaptive refinement **bisects** each base interval and stops on two conditions: `max_depth`
/// levels, and neighbours no closer than `min_layer_height`. So a base interval carries at most
/// `2^max_depth - 1` inserted layers, and at most `layer_height / min_layer_height - 1`.
///
/// The previous estimate charged the job as if every interval refined all the way down to
/// `min_layer_height` — `ceil(height / min_layer_height) + 1` — which ignores `max_depth`, the
/// binding limit at the defaults. Measured on the audit's case that was **2001 layers estimated
/// against 133 actual**, a 15x over-estimate, so switching `adaptive` on could make a job illegal
/// that is perfectly legal without it. Refusing work that would have succeeded is the mirror image
/// of the defects this hardening pass exists to remove.
fn estimate_layer_count(height: f64, layer_height: f64, adaptive: Option<&AdaptiveOptions>) -> f64 {
    let base = libm::ceil(height / layer_height) + 1.0;
    let Some(a) = adaptive else {
        return base;
    };
    let by_depth = libm::pow(2.0, a.max_depth as f64) - 1.0;
    let by_min_height = (layer_height / a.min_layer_height - 1.0).max(0.0);
    let per_interval = by_depth.min(by_min_height);
    base + (base - 1.0).max(0.0) * per_interval
}

/// Estimate the field-sample count and reject a request that blows the guardrail.
///
/// `0` means "no limit" — the documented sentinel `sdk/ts` encodes `Infinity` as — and a non-finite
/// limit is treated the same. NaN and negative values never reach here; they are refused during
/// validation, because accepting them switched the guardrail off silently.
fn assert_budget(
    nx: usize,
    ny: usize,
    estimated_layers: f64,
    max_field_samples: Option<f64>,
) -> Result<(), TpmsError> {
    let max = max_field_samples.unwrap_or(DEFAULT_MAX_FIELD_SAMPLES);
    if !max.is_finite() || max <= 0.0 {
        return Ok(());
    }
    let estimated = (nx + 1) as f64 * (ny + 1) as f64 * estimated_layers;
    if estimated <= max {
        return Ok(());
    }
    Err(TpmsError::new(format!(
        "TPMS resolution budget exceeded ({} field samples > {}). \
         Reduce samples/cells/cell height or raise the layer height.",
        libm::ceil(estimated),
        libm::ceil(max)
    )))
}

fn layer_slice(
    z_local: f64,
    paths: Vec<Path>,
    raw_path_count: usize,
    longest_raw_length: f64,
) -> LayerSlice {
    let point_count = paths.iter().map(|p| p.points.len()).sum();
    let length = paths.iter().map(|p| path_length(&p.points)).sum();
    LayerSlice {
        z_local: round(z_local),
        path_count: paths.len(),
        point_count,
        length,
        raw_path_count,
        longest_raw_length,
        paths,
    }
}

fn needs_adaptive_layer(a: &LayerSlice, b: &LayerSlice, opt: &AdaptiveOptions, depth: u32) -> bool {
    let dz = b.z_local - a.z_local;
    if dz <= opt.min_layer_height + EPS {
        return false;
    }
    if dz > opt.max_layer_height + EPS {
        return depth < opt.max_depth;
    }
    if depth >= opt.max_depth {
        return false;
    }
    if a.path_count.abs_diff(b.path_count) >= 2 {
        return true;
    }
    let length_scale = a.length.max(b.length).max(1.0);
    let point_scale = (a.point_count.max(b.point_count) as f64).max(1.0);
    (a.length - b.length).abs() / length_scale > opt.max_length_delta
        || a.point_count.abs_diff(b.point_count) as f64 / point_scale > opt.max_point_delta
}

fn rectangle_path(width: f64, depth: f64, inset: f64) -> Vec<Point> {
    vec![
        Point { x: inset, y: inset },
        Point {
            x: width - inset,
            y: inset,
        },
        Point {
            x: width - inset,
            y: depth - inset,
        },
        Point {
            x: inset,
            y: depth - inset,
        },
        Point { x: inset, y: inset },
    ]
}

/// Quantised endpoint key (`1e-6` grid), used to match contour endpoints when stitching.
fn point_key(p: Point) -> (i64, i64) {
    (
        (p.x * QUANTUM).round() as i64,
        (p.y * QUANTUM).round() as i64,
    )
}

/// Stitch raw marching-squares segments into continuous polylines by walking a graph over quantised
/// endpoints. The traversal is insertion-ordered (lowest unused segment index first) so the output is
/// deterministic across platforms (no hash-iteration-order dependence).
fn stitch_segments(segments: &[Seg2]) -> Vec<Path> {
    let mut endpoints: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, seg) in segments.iter().enumerate() {
        endpoints.entry(point_key(seg.a)).or_default().push(i);
        endpoints.entry(point_key(seg.b)).or_default().push(i);
    }

    let mut used = vec![false; segments.len()];
    let mut paths = Vec::new();
    let mut cursor = 0usize;
    loop {
        // next unused seed segment (lowest index), mirroring the insertion-ordered JS Set.
        while cursor < used.len() && used[cursor] {
            cursor += 1;
        }
        if cursor >= used.len() {
            break;
        }
        let first = cursor;
        used[first] = true;
        let mut path = vec![segments[first].a, segments[first].b];
        extend_path(&mut path, segments, &endpoints, &mut used, false);
        extend_path(&mut path, segments, &endpoints, &mut used, true);
        paths.push(Path {
            points: dedupe_consecutive(&path),
        });
    }
    paths
}

fn extend_path(
    path: &mut Vec<Point>,
    segments: &[Seg2],
    endpoints: &HashMap<(i64, i64), Vec<usize>>,
    used: &mut [bool],
    at_start: bool,
) {
    loop {
        let endpoint = if at_start {
            path[0]
        } else {
            path[path.len() - 1]
        };
        let key = point_key(endpoint);
        let next = endpoints
            .get(&key)
            .and_then(|list| list.iter().copied().find(|&idx| !used[idx]));
        let Some(next) = next else { return };
        used[next] = true;
        let seg = &segments[next];
        let next_point = if point_key(seg.a) == key {
            seg.b
        } else {
            seg.a
        };
        if at_start {
            path.insert(0, next_point);
        } else {
            path.push(next_point);
        }
    }
}

/// Order paths by a greedy nearest-neighbour walk from `cursor`, reversing each so its nearer end is
/// visited first. All comparisons are in layer-local coordinates.
fn order_paths(paths: Vec<Path>, cursor: Option<Point>) -> Vec<Path> {
    let mut remaining = paths;
    let mut ordered = Vec::with_capacity(remaining.len());
    let mut current = cursor;
    while !remaining.is_empty() {
        let mut best_index = 0;
        let mut best = orient_path(&remaining[0], current);
        let mut best_distance = current.map_or(0.0, |c| distance(c, best.points[0]));
        for (i, candidate_path) in remaining.iter().enumerate().skip(1) {
            let candidate = orient_path(candidate_path, current);
            let d = current.map_or(0.0, |c| distance(c, candidate.points[0]));
            if d < best_distance {
                best_index = i;
                best = candidate;
                best_distance = d;
            }
        }
        current = best.points.last().copied();
        ordered.push(best);
        remaining.remove(best_index);
    }
    ordered
}

fn orient_path(path: &Path, cursor: Option<Point>) -> Path {
    let Some(cursor) = cursor else {
        return path.clone();
    };
    if path.points.len() < 2 {
        return path.clone();
    }
    let first = path.points[0];
    let last = path.points[path.points.len() - 1];
    if distance(cursor, last) < distance(cursor, first) {
        let mut points = path.points.clone();
        points.reverse();
        Path { points }
    } else {
        path.clone()
    }
}

fn append_path(ops: &mut Vec<Op>, points: &[Point], z: f64) {
    let Some((start, rest)) = points.split_first() else {
        return;
    };
    ops.push(Op::Extruder { on: false });
    ops.push(move_op(*start, z));
    ops.push(Op::Extruder { on: true });
    for p in rest {
        ops.push(move_op(*p, z));
    }
}

fn move_op(p: Point, z: f64) -> Op {
    Op::Move {
        x: Some(round(p.x)),
        y: Some(round(p.y)),
        z: Some(round(z)),
    }
}

fn interpolate(a: Point, b: Point, va: f64, vb: f64) -> Point {
    let t = va / (va - vb);
    Point {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
    }
}

fn crosses(a: f64, b: f64) -> bool {
    (a < 0.0 && b > 0.0) || (a > 0.0 && b < 0.0)
}

fn scrub_zero(value: f64) -> f64 {
    if value.abs() > EPS {
        value
    } else if value < 0.0 {
        -EPS
    } else {
        EPS
    }
}

fn dedupe_consecutive(points: &[Point]) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for &p in points {
        match out.last() {
            Some(prev) if distance(*prev, p) <= 1e-7 => {}
            _ => out.push(p),
        }
    }
    out
}

fn path_length(points: &[Point]) -> f64 {
    points.windows(2).map(|w| distance(w[0], w[1])).sum()
}

fn distance(a: Point, b: Point) -> f64 {
    libm::hypot(a.x - b.x, a.y - b.y)
}

/// Build the selected TPMS infill as an L1 op list, returning a structured error for invalid options.
pub fn try_tpms_ops(options: &TpmsOptions) -> Result<Vec<Op>, TpmsError> {
    Tpms::resolve(options)?.emit()
}

/// Build the selected TPMS infill as an L1 op list.
///
/// This compatibility wrapper panics on invalid options; bindings and other user-facing boundaries
/// should call [`try_tpms_ops`] so they can return a structured error.
pub fn tpms_ops(options: &TpmsOptions) -> Vec<Op> {
    try_tpms_ops(options).expect("valid Dry TPMS options")
}

/// Build the selected TPMS infill as an L1 [`Design`], returning a structured error for invalid options.
pub fn try_tpms_design(options: &TpmsOptions) -> Result<Design, TpmsError> {
    Ok(Design {
        ops: try_tpms_ops(options)?,
    })
}

/// Build the selected TPMS infill as an L1 [`Design`] (panics on invalid options; see [`try_tpms_design`]).
pub fn tpms_design(options: &TpmsOptions) -> Design {
    Design {
        ops: tpms_ops(options),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::simulate;
    use crate::resolve::{resolve_checked, ResolveParams};
    use crate::verify::{verify, Contracts};

    /// A small single-cell gyroid at default resolution — fast, but with real contours per layer.
    fn small_cell() -> TpmsOptions {
        TpmsOptions {
            cells_x: Some(1),
            cells_y: Some(1),
            cells_z: Some(1),
            ..Default::default()
        }
    }

    /// Pair each emitted layer's Z with the bead height declared for it, walking the op stream the
    /// way `resolve` does: `Op::Geometry` is a sticky channel, so every move carries the last one
    /// seen. Only the first move of each layer is recorded.
    fn declared_bead_heights_by_layer(ops: &[Op]) -> Vec<(f64, f64)> {
        let mut height: Option<f64> = None;
        let mut out: Vec<(f64, f64)> = Vec::new();
        for op in ops {
            match op {
                Op::Geometry {
                    height: Some(h), ..
                } => height = Some(*h),
                Op::Move { z: Some(z), .. } => {
                    let h = height.expect("a geometry op must precede the first move");
                    if out.last().is_none_or(|&(prev_z, _)| prev_z != *z) {
                        out.push((*z, h));
                    }
                }
                _ => {}
            }
        }
        out
    }

    #[test]
    fn adaptive_layers_declare_the_bead_height_they_occupy() {
        // Measured before the fix: one `Op::Geometry { height: 0.4 }` against actual layer gaps of
        // [0.05, 0.1, 0.2, 0.4] — an 8x over-extrusion on the thinnest layer. `resolve.rs` reads the
        // declared height as deposited volume (`length × width × height × flow`), so the IR records
        // a self-consistent lie that no verifier can catch.
        let options = TpmsOptions {
            cells_x: Some(1),
            cells_y: Some(1),
            cells_z: Some(1),
            samples_per_cell: Some(8),
            layer_height: Some(0.4),
            adaptive: Some(true),
            adaptive_min_layer_height: Some(0.05),
            adaptive_max_length_delta: Some(0.01),
            adaptive_max_point_delta: Some(0.01),
            ..Default::default()
        };
        let ops = try_tpms_ops(&options).expect("valid adaptive options");

        let declared = declared_bead_heights_by_layer(&ops);
        for w in declared.windows(2) {
            let (z_prev, _) = w[0];
            let (z, height) = w[1];
            let gap = z - z_prev;
            assert!(
                (height - gap).abs() <= 1e-9,
                "layer at z={z} spans {gap} but declares bead height {height}"
            );
        }
        assert!(
            declared.len() > 2,
            "the fixture must actually refine: {declared:?}"
        );
        // The refinement must really be non-uniform, or the assertion above is vacuous.
        let gaps: Vec<f64> = declared.windows(2).map(|w| w[1].0 - w[0].0).collect();
        let (lo, hi) = gaps
            .iter()
            .fold((f64::MAX, 0.0_f64), |(lo, hi), &g| (lo.min(g), hi.max(g)));
        assert!(
            hi - lo > 1e-6,
            "the fixture must produce varying layer gaps, got {lo}..{hi}"
        );
    }

    #[test]
    fn a_uniform_non_adaptive_job_declares_one_bead_height() {
        // The common path must not churn: when the block height is an exact multiple of the layer
        // height there is no clamped remainder, so a uniform-gap job keeps exactly one
        // `Op::Geometry`. (`cellSize 12 / layerHeight 0.25` divides exactly; the 0.28 default does
        // not — that case is `a_clamped_top_layer_declares_the_gap_it_occupies`.)
        let layer_height = 0.25;
        let options = TpmsOptions {
            layer_height: Some(layer_height),
            ..small_cell()
        };
        let ops = try_tpms_ops(&options).expect("valid options");
        let geometry: Vec<&Op> = ops
            .iter()
            .filter(|o| matches!(o, Op::Geometry { .. }))
            .collect();
        assert_eq!(
            geometry.len(),
            1,
            "a uniform non-adaptive job must declare its bead once, got {geometry:?}"
        );
        assert!(
            matches!(geometry[0], Op::Geometry { height: Some(h), .. }
                if (*h - layer_height).abs() < 1e-12),
            "the single declaration must be the layer height, got {:?}",
            geometry[0]
        );
    }

    #[test]
    fn a_clamped_top_layer_declares_the_gap_it_occupies() {
        // The top base layer is clamped to the block height, so it gets whatever Z remains — and the
        // per-layer declaration used to be gated on `adaptive`, so a non-adaptive job declared the
        // full `layerHeight` for it. Measured at the generator defaults (`cellSize 12`,
        // `layerHeight 0.28`): the top layer spans 0.2 mm and declared 0.28 (1.40x). The residual was
        // not bounded at 40% — `DEGENERATE_TOP_LAYER_FRACTION` merges only sub-1% remainders, so the
        // worst surviving case is ~100x, measured at `layerHeight 0.5997` (gap 0.006, declared
        // 0.5997 — 99.95x).
        for (layer_height, expected_top_gap) in [(DEFAULT_LAYER_HEIGHT, 0.24), (0.5997, 0.006)] {
            let options = TpmsOptions {
                layer_height: Some(layer_height),
                ..small_cell()
            };
            let ops = try_tpms_ops(&options).expect("valid options");
            let declared = declared_bead_heights_by_layer(&ops);
            for w in declared.windows(2) {
                let (z_prev, _) = w[0];
                let (z, height) = w[1];
                let gap = round(z - z_prev);
                assert!(
                    (height - gap).abs() <= 1e-9,
                    "layerHeight {layer_height}: layer at z={z} spans {gap} but declares bead \
                     height {height}"
                );
            }
            // The fixture must really clamp, or the loop above is satisfied by uniformity alone.
            let (top_z, top_height) = *declared.last().expect("layers were emitted");
            let (below_z, _) = declared[declared.len() - 2];
            assert!(
                (round(top_z - below_z) - expected_top_gap).abs() <= 1e-9,
                "layerHeight {layer_height}: expected a clamped top gap of {expected_top_gap}, got \
                 {} (z {below_z} -> {top_z})",
                top_z - below_z
            );
            assert!(
                (top_height - expected_top_gap).abs() <= 1e-9,
                "layerHeight {layer_height}: the clamped top layer must declare {expected_top_gap}, \
                 got {top_height}"
            );
            // Exactly one extra declaration: the initial one plus the clamped layer's.
            let geometry = ops
                .iter()
                .filter(|o| matches!(o, Op::Geometry { .. }))
                .count();
            assert_eq!(
                geometry, 2,
                "layerHeight {layer_height}: a clamped job declares twice, got {geometry}"
            );
        }
    }

    #[test]
    fn bead_height_is_preserved_as_a_ratio_on_every_layer() {
        // `beadHeight` is a documented public option and deliberate squish is a legitimate use, but
        // declaring the raw gap discarded it from the second layer onward: measured
        // `layerHeight 0.4, beadHeight 0.5, adaptive, adaptiveMinLayerHeight 0.2` emitted declared
        // heights [0.5, 0.2] — the 1.25x squish silently became 0.5x on every layer after the first.
        let layer_height = 0.4;
        let bead_height = 0.5;
        let squish = bead_height / layer_height;
        let options = TpmsOptions {
            samples_per_cell: Some(8),
            layer_height: Some(layer_height),
            bead_height: Some(bead_height),
            adaptive: Some(true),
            adaptive_min_layer_height: Some(0.2),
            ..small_cell()
        };
        let ops = try_tpms_ops(&options).expect("valid options");
        let declared = declared_bead_heights_by_layer(&ops);
        for w in declared.windows(2) {
            let (z_prev, _) = w[0];
            let (z, height) = w[1];
            let gap = round(z - z_prev);
            assert!(
                (height - gap * squish).abs() <= 1e-6,
                "layer at z={z} spans {gap} and must declare {} at a {squish}x squish, got {height}",
                gap * squish
            );
        }
        // The first layer has no gap beneath it, so it declares the configured bead as-is.
        assert!(
            (declared[0].1 - bead_height).abs() <= 1e-12,
            "the first layer must declare the configured beadHeight, got {}",
            declared[0].1
        );
        // The fixture must actually refine below the nominal layer height, or every declaration is
        // just `beadHeight` and the ratio is untested.
        let gaps: Vec<f64> = declared
            .windows(2)
            .map(|w| round(w[1].0 - w[0].0))
            .collect();
        assert!(
            gaps.iter().any(|g| *g < layer_height - 1e-9),
            "the fixture must produce sub-nominal gaps, got {gaps:?}"
        );

        // The adaptive fixture above is not what a user with a squish setting actually hits: the
        // common case is a plain job whose top layer is clamped. `layerHeight 0.28,
        // beadHeight 0.35` (a 1.25x squish) leaves a 0.24 mm top gap, which must declare 0.30 —
        // the same ratio — rather than 0.24 (the raw gap) or 0.35 (the nominal bead).
        let options = TpmsOptions {
            layer_height: Some(0.28),
            bead_height: Some(0.35),
            ..small_cell()
        };
        let ops = try_tpms_ops(&options).expect("valid options");
        let clamped: Vec<f64> = ops
            .iter()
            .filter_map(|o| match o {
                Op::Geometry {
                    height: Some(h), ..
                } => Some(*h),
                _ => None,
            })
            .collect();
        assert_eq!(
            clamped.len(),
            2,
            "a clamped squished job declares the nominal bead and the clamped one: {clamped:?}"
        );
        assert!(
            (clamped[0] - 0.35).abs() <= 1e-12 && (clamped[1] - 0.30).abs() <= 1e-12,
            "layerHeight 0.28 / beadHeight 0.35 must declare [0.35, 0.30], got {clamped:?}"
        );
    }

    #[test]
    fn a_nominal_layer_declares_the_configured_bead_bit_for_bit() {
        // `declared_bead_height`'s `gap == layer_height` arm is an exactness guard, not a policy
        // case: without it a nominal layer's declaration goes through `round`, which can land a
        // hair off the configured `beadHeight` and churn a spurious second `Op::Geometry` on a job
        // that has only nominal gaps. `layerHeight 0.25` divides `cellSize 12` exactly, so every
        // gap is nominal; `beadHeight 1/3` is not representable at the 1e-6 quantum, so `round`
        // yields 0.333333 against a configured 0.3333333333333333. With the guard the job declares
        // once (3231 ops); replacing it with `false` gives two declarations (3232).
        let bead_height = 0.3333333333333333_f64;
        let options = TpmsOptions {
            layer_height: Some(0.25),
            bead_height: Some(bead_height),
            ..small_cell()
        };
        let ops = try_tpms_ops(&options).expect("valid options");
        let geometry: Vec<f64> = ops
            .iter()
            .filter_map(|o| match o {
                Op::Geometry {
                    height: Some(h), ..
                } => Some(*h),
                _ => None,
            })
            .collect();
        assert_eq!(
            geometry.len(),
            1,
            "every gap is nominal, so an unrepresentable beadHeight must still declare once, got \
             {geometry:?}"
        );
        assert_eq!(
            geometry[0], bead_height,
            "the declaration must be the configured beadHeight bit for bit, not a rounded copy"
        );
    }

    #[test]
    fn a_layer_after_skipped_layers_declares_the_nominal_bead_not_the_span() {
        // A layer that traces no contour emits nothing and does not advance `previous_z`, so the
        // next depositing layer's gap spans every skipped layer. Ungating the declaration made that
        // the default-path behaviour: measured at `neovius, isoLevel 1.0, layerHeight 0.2`, the
        // layer at z=7.8 follows fifteen empty ones and spans 3.2 mm, so it declared 3.2 — a 16x
        // bead, worth +20% of filament through `resolve_checked` (47.2735 -> 56.7330 mm).
        //
        // The gap is only a bead height when it is material actually stacked beneath. Nothing was
        // deposited across the skipped layers, so the nozzle lays one ordinary bead onto whatever
        // is below rather than a 3.2 mm column into air: such a layer declares the nominal bead.
        let layer_height = 0.2;
        let options = TpmsOptions {
            surface: Some(Surface::Neovius),
            iso_level: Some(1.0),
            layer_height: Some(layer_height),
            ..small_cell()
        };
        let ops = try_tpms_ops(&options).expect("valid options");
        let declared = declared_bead_heights_by_layer(&ops);

        // The fixture must really skip layers, or every assertion below is satisfied by uniformity.
        let spanning: Vec<(f64, f64)> = declared
            .windows(2)
            .filter(|w| round(w[1].0 - w[0].0) > layer_height + 1e-9)
            .map(|w| (w[1].0, round(w[1].0 - w[0].0)))
            .collect();
        assert_eq!(
            spanning,
            vec![(7.8, 3.2)],
            "the fixture must leave exactly one 3.2 mm gap of skipped layers, got {spanning:?} \
             from {:?}",
            declared.iter().map(|(z, _)| *z).collect::<Vec<_>>()
        );

        // No layer may declare more than the nominal bead: the spanning one declares nominal, and
        // every other gap is nominal or the clamped remainder.
        for &(z, height) in &declared {
            assert!(
                height <= layer_height + 1e-9,
                "layer at z={z} declares {height}, above the nominal {layer_height}"
            );
        }
        let (span_z, span_height) = *declared
            .iter()
            .find(|(z, _)| *z == 7.8)
            .expect("the spanning layer is emitted");
        assert!(
            (span_height - layer_height).abs() <= 1e-12,
            "the layer at z={span_z} follows skipped layers and must declare the nominal \
             {layer_height}, got {span_height}"
        );
        // With every gap either nominal or spanned-but-nominal, the job declares exactly once.
        let geometry: Vec<&Op> = ops
            .iter()
            .filter(|o| matches!(o, Op::Geometry { .. }))
            .collect();
        assert_eq!(
            geometry.len(),
            1,
            "skipped layers must not add declarations, got {geometry:?}"
        );
    }

    #[test]
    fn the_degenerate_top_layer_threshold_is_one_percent() {
        // `DEGENERATE_TOP_LAYER_FRACTION` survives H1.6 as a slicing rule (an honest declaration
        // does not make a degenerate layer emittable), so its boundary is load-bearing and pinned
        // here from both sides. `layerHeight 2.0` against a block of 12.018 leaves 0.9% — merged;
        // 12.022 leaves 1.1% — stacked. Anything that moves the constant fails one of the two.
        let base_zs = |cell_size: f64| {
            let options = TpmsOptions {
                cell_size: Some(cell_size),
                layer_height: Some(2.0),
                ..small_cell()
            };
            Tpms::resolve(&options)
                .expect("valid options")
                .base_layer_zs()
        };

        let merged = base_zs(12.018);
        assert_eq!(
            merged.len(),
            7,
            "a 0.9% remainder must merge into the layer below, got {merged:?}"
        );
        assert!(
            (merged[6] - 12.018).abs() <= 1e-9 && (merged[5] - 10.0).abs() <= 1e-9,
            "the merged block must still reach its full height in one 2.018 mm step: {merged:?}"
        );

        let stacked = base_zs(12.022);
        assert_eq!(
            stacked.len(),
            8,
            "a 1.1% remainder is a layer, not an artifact, and must be stacked: {stacked:?}"
        );
        assert!(
            (stacked[7] - 12.022).abs() <= 1e-9 && (stacked[6] - 12.0).abs() <= 1e-9,
            "the stacked top layer must sit 0.022 mm above the one below: {stacked:?}"
        );
    }

    #[test]
    fn a_sliver_top_layer_is_merged_not_stacked() {
        // `base_layer_zs` unconditionally appended a final layer at exactly `height`. With
        // cellSize 12, cellsZ 1, layerHeight 1.9999 that put two layers 0.6 µm apart, both
        // extruding a full 1.9999 mm bead.
        let layer_height = 1.9999;
        let options = TpmsOptions {
            layer_height: Some(layer_height),
            ..small_cell()
        };
        let ops = try_tpms_ops(&options).expect("valid options");
        let declared = declared_bead_heights_by_layer(&ops);
        let zs: Vec<f64> = declared.iter().map(|(z, _)| *z).collect();
        for w in zs.windows(2) {
            assert!(
                w[1] - w[0] > 0.01 * layer_height,
                "layers at {} and {} are a sliver apart: {zs:?}",
                w[0],
                w[1]
            );
        }
        // Merging must not shorten the block: the top layer still caps it at z0 + height.
        assert!(
            zs.last().is_some_and(|z| (z - (0.2 + 12.0)).abs() < 1e-6),
            "the block must still reach its full height: {zs:?}"
        );
        // Merging makes the absorbing layer taller than nominal, so it must declare the enlarged
        // gap: 12.0 - 9.9995 = 2.0005, not the 1.9999 it was configured with.
        let (top_z, top_height) = *declared.last().expect("layers were emitted");
        let (below_z, _) = declared[declared.len() - 2];
        assert!(
            (top_height - round(top_z - below_z)).abs() <= 1e-9
                && (top_height - 2.0005).abs() <= 1e-9,
            "the merged top layer spans {} and must declare it, got {top_height}",
            top_z - below_z
        );
    }

    #[test]
    fn iso_level_outside_the_field_range_is_rejected() {
        // The gyroid saturates at ±1.5, so nothing at or beyond that traces a contour. Every one of
        // these used to return `Ok` with 4 ops and no moves: a program that heats the nozzle,
        // verifies clean, and prints nothing.
        for iso in [1.5_f64, 5.0, 1e6, -1e6] {
            let options = TpmsOptions {
                iso_level: Some(iso),
                ..small_cell()
            };
            let err = try_tpms_ops(&options).expect_err(&format!(
                "isoLevel {iso} must be refused, not emitted empty"
            ));
            assert!(
                err.to_string().contains("isoLevel"),
                "message must name the option: {err}"
            );
            // A message that blames the wrong option is worse than none: nothing was filtered here,
            // there were no contours to filter.
            assert!(
                !err.to_string().contains("minPathLength"),
                "message must not blame the filter when no contour existed: {err}"
            );
        }
    }

    #[test]
    fn min_path_length_that_filters_every_path_is_rejected() {
        let options = TpmsOptions {
            min_path_length: Some(1e9),
            ..small_cell()
        };
        let err = try_tpms_ops(&options)
            .expect_err("a minPathLength above the contour scale must be refused");
        assert!(
            err.to_string().contains("minPathLength"),
            "message must name the option that actually dropped the contours: {err}"
        );
        // The iso level is fine here — contours were traced and then filtered away.
        assert!(
            !err.to_string().contains("isoLevel"),
            "message must not blame the iso level when contours were traced: {err}"
        );
    }

    #[test]
    fn a_derived_min_path_length_blames_iso_level_not_the_filter() {
        // #189: `minPathLength` defaults to `dx.min(dy)` — derived from the sample spacing, never
        // typed by the caller. When contours exist but are shorter than that derived value, the old
        // message told the user to "lower minPathLength", naming a number they had not set, on 7 of
        // 10 surfaces. It was always in the narrow band at the edge of the field range, which is
        // exactly where someone tuning `isoLevel` lands — and lowering the filter did not help,
        // because the surviving contours are microscopic either way.
        for (surface, iso) in [
            ("schwarz-d", 1.4),
            ("fischer-koch-s", 1.4),
            ("split-p", 1.7),
        ] {
            let json = format!(
                r#"{{"surface":"{surface}","isoLevel":{iso},"cellSize":12,"cellsX":1,"cellsY":1,
                    "cellsZ":1,"samplesPerCell":8}}"#
            );
            let options: TpmsOptions = serde_json::from_str(&json).unwrap();
            let err = try_tpms_ops(&options)
                .expect_err(&format!("{surface} at isoLevel {iso} must be refused"));
            let msg = err.to_string();

            assert!(
                msg.contains("isoLevel"),
                "[{surface}] must name the option the caller actually set: {msg}"
            );
            assert!(
                !msg.contains("Lower minPathLength"),
                "[{surface}] must not send the user to a knob they never touched: {msg}"
            );
        }

        // The control: when the caller *does* set `minPathLength`, it keeps the blame.
        let options = TpmsOptions {
            min_path_length: Some(1e9),
            ..small_cell()
        };
        let msg = try_tpms_ops(&options).unwrap_err().to_string();
        assert!(msg.contains("Lower minPathLength"), "{msg}");
    }

    #[test]
    fn perimeter_inset_collapsing_the_rectangle_is_rejected() {
        let options = TpmsOptions {
            perimeter: Some(true),
            perimeter_inset: Some(1e9),
            ..small_cell()
        };
        let err = try_tpms_ops(&options)
            .expect_err("an inset >= width/2 must be refused, not clamped to a 2e-9 rectangle");
        assert!(
            err.to_string().contains("perimeterInset"),
            "message must name the option: {err}"
        );
    }

    #[test]
    fn a_valid_job_still_deposits_material() {
        let ops = try_tpms_ops(&small_cell()).expect("default options are valid");
        assert!(
            ops.iter().any(|o| matches!(o, Op::Move { .. })),
            "control: valid options must still emit moves"
        );
    }

    #[test]
    fn budget_guardrail_triggers_at_threshold() {
        // nx = ny = 10 → (10+1)·(10+1) = 121 samples per layer.
        //
        // The layer count uses a NON-integral height/layerHeight ratio on purpose: height = 10 with
        // layerHeight = 3 gives ceil(10/3) + 1 = 5 layers, where floor would give 4. The old fixture
        // used layerHeight = 1, so ceil(10) and floor(10) were both 10 and a `ceil`→`floor` mutation
        // survived the boundary test — the same defect fixed in `generate/pocket.rs`.
        //
        // 121 · 5 = 605 samples; the floor mutant would estimate 121 · 4 = 484.
        let base = TpmsOptions {
            cell_size: Some(10.0),
            cells_x: Some(1),
            cells_y: Some(1),
            cells_z: Some(1),
            samples_per_cell: Some(10),
            layer_height: Some(3.0),
            ..Default::default()
        };

        let mut too_tight = base.clone();
        too_tight.max_field_samples = Some(604.0);
        let err = try_tpms_ops(&too_tight).expect_err("604 < 605 should trip the guardrail");
        assert!(
            err.to_string().contains("budget"),
            "expected a budget error, got: {err}"
        );

        let mut just_enough = base;
        just_enough.max_field_samples = Some(605.0);
        assert!(
            try_tpms_ops(&just_enough).is_ok(),
            "605 == estimate should pass"
        );
    }

    #[test]
    fn max_field_samples_rejects_nan_and_negative_but_keeps_zero_unlimited() {
        let huge = || TpmsOptions {
            cell_size: Some(10.0),
            cells_x: Some(1),
            cells_y: Some(1),
            cells_z: Some(1),
            samples_per_cell: Some(10),
            layer_height: Some(3.0),
            ..Default::default()
        };

        for bad in [f64::NAN, -1.0, -f64::INFINITY] {
            let mut o = huge();
            o.max_field_samples = Some(bad);
            let err = try_tpms_ops(&o).expect_err(&format!(
                "maxFieldSamples {bad} must be refused, not ignored"
            ));
            assert!(
                err.to_string().contains("maxFieldSamples"),
                "message must name the option: {err}"
            );
        }

        // `0` stays the documented "unlimited" sentinel — it is the value `sdk/ts` encodes
        // `Infinity` as, so changing it would break a released TS API.
        let mut unlimited = huge();
        unlimited.max_field_samples = Some(0.0);
        assert!(
            try_tpms_ops(&unlimited).is_ok(),
            "0 must keep meaning unlimited"
        );
    }

    #[test]
    fn adaptive_budget_charges_bisection_depth_not_the_minimum_layer_height() {
        // Adaptive bisects, bounded by `adaptiveMaxDepth`. Charging the job as if every interval
        // refined to `adaptiveMinLayerHeight` over-counted 15x at the defaults, so switching
        // `adaptive` on could refuse a job that is legal without it.
        let base_layers = libm::ceil(10.0 / 3.0) + 1.0;
        let adaptive = AdaptiveOptions {
            min_layer_height: 0.01,
            max_layer_height: 3.0,
            max_length_delta: 0.35,
            max_point_delta: 0.45,
            max_depth: 2,
        };

        let estimate = estimate_layer_count(10.0, 3.0, Some(&adaptive));
        // depth 2 → at most 3 inserted layers per interval, not 3/0.01 = 299.
        assert_eq!(estimate, base_layers + (base_layers - 1.0) * 3.0);

        let naive = libm::ceil(10.0 / adaptive.min_layer_height) + 1.0;
        assert!(
            estimate < naive / 10.0,
            "estimate {estimate} should be far below the old {naive}"
        );
        assert_eq!(estimate_layer_count(10.0, 3.0, None), base_layers);
    }

    #[test]
    fn adaptive_max_depth_is_validated() {
        let mut o = small_cell();
        o.adaptive = Some(true);
        o.adaptive_max_depth = Some(MAX_ADAPTIVE_MAX_DEPTH + 1);
        let err = try_tpms_ops(&o).expect_err("an absurd refinement depth must be refused");
        assert!(
            err.to_string().contains("adaptiveMaxDepth"),
            "message must name the option: {err}"
        );
    }

    #[test]
    fn a_perimeter_rectangle_below_the_emission_grid_is_rejected() {
        // #189: the old gate was an exact `2·inset >= width` boundary, so an inset one epsilon below
        // it left a rectangle far under the 1e-6 emission grid — 176 coincident extruding moves with
        // zero extruded length, and `verify` clean.
        let mut o = small_cell();
        o.cell_size = Some(12.0);
        o.cells_x = Some(1);
        o.cells_y = Some(1);
        o.cells_z = Some(1);
        o.perimeter = Some(true);
        o.perimeter_inset = Some(5.9999999);

        let err = try_tpms_ops(&o).expect_err("a sub-grid perimeter rectangle must be refused");
        let msg = err.to_string();
        assert!(msg.contains("perimeterInset"), "names the option: {msg}");
        assert!(msg.contains("emission grid"), "explains the floor: {msg}");
    }

    #[test]
    fn a_defaulted_perimeter_inset_says_where_the_value_came_from() {
        // `perimeterInset` defaults to `beadWidth`, so any `perimeter: true` job on a block narrower
        // than two beads is refused naming a knob the caller never set (#189).
        let mut o = small_cell();
        o.cell_size = Some(0.9);
        o.cells_x = Some(1);
        o.cells_y = Some(1);
        o.cells_z = Some(1);
        o.perimeter = Some(true);
        o.perimeter_inset = None;

        let err = try_tpms_ops(&o).expect_err("a block narrower than two beads must be refused");
        let msg = err.to_string();
        assert!(
            msg.contains("defaulted from beadWidth"),
            "must say the value was derived, not chosen: {msg}"
        );
    }

    #[test]
    fn small_gyroid_cell_emits_balanced_extruder_ops() {
        let ops = tpms_ops(&small_cell());
        assert!(!ops.is_empty(), "expected a non-empty op list");

        let on = ops
            .iter()
            .filter(|o| matches!(o, Op::Extruder { on: true }))
            .count();
        let off = ops
            .iter()
            .filter(|o| matches!(o, Op::Extruder { on: false }))
            .count();
        assert!(on > 0, "expected at least one extruding path");
        // each path emits exactly one `off` then one `on`; plus a single trailing `off`.
        assert_eq!(
            off,
            on + 1,
            "extruder on/off must be balanced (+1 trailing off)"
        );
        assert!(
            ops.iter().any(|o| matches!(o, Op::Move { .. })),
            "expected at least one move"
        );
    }

    #[test]
    fn moves_are_finite_and_within_bounds() {
        let options = small_cell();
        let ops = tpms_ops(&options);

        // default single cell: cellSize 12 → width = depth = height = 12, centred at (50, 50), z0 = 0.2.
        let (cell, cx, cy, z0) = (12.0, 50.0, 50.0, 0.2);
        let (x_lo, x_hi) = (cx - cell / 2.0, cx + cell / 2.0);
        let (y_lo, y_hi) = (cy - cell / 2.0, cy + cell / 2.0);
        let (z_lo, z_hi) = (z0, z0 + cell);
        let tol = 1e-6;

        let mut moves = 0;
        for op in &ops {
            if let Op::Move { x, y, z } = op {
                if let Some(x) = x {
                    assert!(
                        x.is_finite() && *x >= x_lo - tol && *x <= x_hi + tol,
                        "x = {x}"
                    );
                }
                if let Some(y) = y {
                    assert!(
                        y.is_finite() && *y >= y_lo - tol && *y <= y_hi + tol,
                        "y = {y}"
                    );
                }
                if let Some(z) = z {
                    assert!(
                        z.is_finite() && *z >= z_lo - tol && *z <= z_hi + tol,
                        "z = {z}"
                    );
                }
                moves += 1;
            }
        }
        assert!(moves > 0, "expected at least one move to check");
    }

    #[test]
    fn resolved_toolpath_verifies_and_extrudes() {
        let design = tpms_design(&small_cell());
        let tp = resolve_checked(&design, &ResolveParams::default()).expect("resolve");

        let report = verify(&tp, &Contracts::default());
        assert!(
            report.findings.is_empty(),
            "permissive verify should be clean, got: {:?}",
            report.findings
        );

        let metrics = simulate(&tp);
        assert!(
            metrics.extruded_volume.value() > 0.0,
            "expected non-zero extruded volume, got {}",
            metrics.extruded_volume.value()
        );
    }

    /// Every surface, sliced over a small multi-cell block, must produce real geometry that survives
    /// the whole pipeline: balanced extruder ops, finite in-bounds moves, a clean permissive verify,
    /// and non-zero deposited volume.
    #[test]
    fn every_surface_emits_valid_in_bounds_geometry() {
        let surfaces = [
            Surface::Gyroid,
            Surface::SchwarzP,
            Surface::SchwarzD,
            Surface::Iwp,
            Surface::Neovius,
            Surface::FischerKochS,
            Surface::FischerKochY,
            Surface::Frd,
            Surface::Lidinoid,
            Surface::SplitP,
        ];
        // A 2x2x2 block guarantees several iso-crossings for every surface at isoLevel 0.
        let (cells, cell, spc, lh, cx, cy, z0) = (2u32, 8.0, 12u32, 0.4, 50.0, 50.0, 0.2);
        let span = cells as f64 * cell;
        let (x_lo, x_hi) = (cx - span / 2.0, cx + span / 2.0);
        let (y_lo, y_hi) = (cy - span / 2.0, cy + span / 2.0);
        let (z_lo, z_hi) = (z0, z0 + span);
        let tol = 1e-6;

        for surface in surfaces {
            let options = TpmsOptions {
                surface: Some(surface),
                cells_x: Some(cells),
                cells_y: Some(cells),
                cells_z: Some(cells),
                cell_size: Some(cell),
                samples_per_cell: Some(spc),
                layer_height: Some(lh),
                ..Default::default()
            };
            let ops = tpms_ops(&options);

            let on = ops
                .iter()
                .filter(|o| matches!(o, Op::Extruder { on: true }))
                .count();
            let off = ops
                .iter()
                .filter(|o| matches!(o, Op::Extruder { on: false }))
                .count();
            assert!(on > 0, "{surface:?}: expected at least one extruding path");
            assert_eq!(
                off,
                on + 1,
                "{surface:?}: extruder on/off must balance (+1)"
            );

            let mut moves = 0;
            for op in &ops {
                if let Op::Move { x, y, z } = op {
                    if let Some(x) = x {
                        assert!(
                            x.is_finite() && *x >= x_lo - tol && *x <= x_hi + tol,
                            "{surface:?}: x = {x}"
                        );
                    }
                    if let Some(y) = y {
                        assert!(
                            y.is_finite() && *y >= y_lo - tol && *y <= y_hi + tol,
                            "{surface:?}: y = {y}"
                        );
                    }
                    if let Some(z) = z {
                        assert!(
                            z.is_finite() && *z >= z_lo - tol && *z <= z_hi + tol,
                            "{surface:?}: z = {z}"
                        );
                    }
                    moves += 1;
                }
            }
            assert!(moves > 0, "{surface:?}: expected moves");

            let tp = resolve_checked(&tpms_design(&options), &ResolveParams::default())
                .expect("resolve");
            let report = verify(&tp, &Contracts::default());
            assert!(
                report.findings.is_empty(),
                "{surface:?}: permissive verify should be clean, got {:?}",
                report.findings
            );
            assert!(
                simulate(&tp).extruded_volume.value() > 0.0,
                "{surface:?}: expected non-zero extruded volume"
            );
        }
    }

    #[test]
    fn unknown_surface_string_is_a_clean_deserialize_error() {
        let err = serde_json::from_str::<TpmsOptions>(r#"{"surface":"bogus"}"#)
            .expect_err("an unknown surface name must be a deserialize error, not a panic");
        assert!(
            err.to_string().contains("bogus") || err.to_string().contains("variant"),
            "got: {err}"
        );
    }

    #[test]
    fn omitted_surface_defaults_to_gyroid() {
        let default_ops = tpms_ops(&small_cell());
        let explicit = TpmsOptions {
            surface: Some(Surface::Gyroid),
            ..small_cell()
        };
        let explicit_ops = tpms_ops(&explicit);
        assert_eq!(
            default_ops.len(),
            explicit_ops.len(),
            "omitting `surface` must match explicit gyroid"
        );
    }

    #[test]
    fn options_deserialize_from_camel_case_json() {
        let options: TpmsOptions =
            serde_json::from_str(r#"{"cellSize":10,"cellsX":1,"cellsY":1,"cellsZ":1}"#).unwrap();
        assert_eq!(options.cell_size, Some(10.0));
        assert_eq!(options.cells_x, Some(1));
        assert!(try_tpms_ops(&options).is_ok());
    }
}

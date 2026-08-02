//! Clothoid (Euler-spiral) corner blending — the geometry behind the L1 [`crate::resolve::Op::Clothoid`]
//! node (`docs/04-tasks.md` P5.5).
//!
//! # Why a corner blend, and why a standalone op
//!
//! The issue left one design choice open: a standalone `Op::Clothoid` or a corner-blend parameter on
//! the existing path nodes. This is **both**: a standalone op whose *geometry* is a corner blend.
//!
//! A blend parameter on `Op::Move` was rejected for two reasons. It needs lookahead — the blend at a
//! corner is a function of the move *after* it — and `resolve` is a strictly forward single pass over
//! the op list, with `validate_design_geometry` running the same forward walk to gate it; a node that
//! reads ahead would have to be threaded through both, and through `expand_feature_ops`, which
//! rewrites ops one at a time. And it would change the meaning of an existing op: every `Move` would
//! grow a field whose default has to be "no blend", which is a wire-format change to the op every
//! design already uses, for a feature almost none of them want.
//!
//! A standalone node that draws a spiral for its own sake was rejected in the other direction: an
//! Euler spiral exists to make curvature continuous *through a corner*. Drawing one in isolation
//! needs a start heading, and `resolve` tracks position only — it would have to infer the heading
//! from whatever segment happened to precede it, which is silent, order-dependent, and wrong the
//! moment a dwell or a retract sits in between.
//!
//! So the op carries its own corner: a construction vertex plus the point beyond it. That makes it
//! self-contained (no heading state, no lookahead) while still being a corner blend. It is the same
//! shape as [`crate::resolve::Op::Arc`], deliberately: `Arc` carries an XY construction point
//! (`cx`, `cy`) that the path never visits, an end point that inherits unset axes from the running
//! position, and a Z that rises linearly along the planar curve. `Clothoid` carries `corner_x`,
//! `corner_y`, the same inheriting end point, and the same Z convention.
//!
//! # What is evaluated, and how accurately
//!
//! An Euler spiral has curvature linear in arc length, `κ(s) = s / A²`. Integrating the tangent angle
//! `θ(s) = s²/(2A²)` gives a position with no closed form:
//!
//! ```text
//! x(s) = ∫₀ˢ cos(t²/(2A²)) dt,   y(s) = ∫₀ˢ sin(t²/(2A²)) dt
//! ```
//!
//! Substituting `t = A·τ` makes both integrals scale-free: `x = A·Cf(s/A)`, `y = A·Sf(s/A)`, where
//! `Cf`/`Sf` are the Fresnel integrals in the `t²/2` convention. [`fresnel`] evaluates them by their
//! Maclaurin series, summed until a term falls to [`FRESNEL_SERIES_EPSILON`] *relative to the partial
//! sum* — see that constant for why the threshold is relative and what error it buys. Every
//! transcendental call is `libm`, so the sampled points are bit-identical on native and wasm.
//!
//! The blend is then sampled to `2·SAMPLES` line segments and lowered in `resolve` (`resolve.rs`),
//! so no downstream pass — codec, verify, optimize, emit — learns a new segment kind.

use crate::resolve::SAMPLES;

/// Relative truncation threshold for the Fresnel series in [`fresnel`].
///
/// The series is summed until `|term| <= FRESNEL_SERIES_EPSILON * |partial sum|`. The threshold is
/// **relative**, not absolute, and that is load-bearing: a sampled point is `A·Cf(τ)`, and `A` is
/// unbounded above (it grows without limit as the corner's deflection shrinks toward a straight
/// line). An absolute series bound `ε` would therefore give an unbounded position error `A·ε`. A
/// relative bound gives `A·ε·Cf(τ) ≤ ε · blend`, since `A·Cf(τ)` never exceeds the tangent length
/// the caller asked for — the truncation error is a fixed fraction of the blend, at any scale.
///
/// `1e-17` sits just below the binary64 relative resolution (`2⁻⁵³ ≈ 1.11e-16`), so the truncation is
/// dominated by rounding rather than the other way round. It is a policy threshold on the *terms*,
/// not a measured error; the measured accuracy of the truncated series is published as a separate
/// budget (`proofs/resolve-clothoid-numeric-profile-v0.toml`) and measured by
/// `crates/core/tests/clothoid.rs` against an independent quadrature.
pub const FRESNEL_SERIES_EPSILON: f64 = 1e-17;

/// Hard cap on Fresnel series terms, so the loop terminates on any input including a NaN argument.
///
/// Never reached in the op's domain: the argument is `τ ≤ √π ≈ 1.7725` (a corner cannot deflect by
/// more than 180°), where the alternating terms fall about two decades apart and the relative
/// threshold above is met at the 13th term. Measured, not argued —
/// `fresnel_series_terminates_well_inside_its_cap` walks the domain and reports the worst case.
const FRESNEL_SERIES_MAX_TERMS: usize = 32;

/// Why a corner blend could not be constructed. Each variant is an *exact* rejection — a comparison
/// against zero or against a supplied length — not a tolerance test, so none of these carries an
/// epsilon.
#[derive(Debug, Clone, PartialEq)]
pub enum ClothoidError {
    /// The incoming leg (running position → corner) has zero XY length, so there is no direction to
    /// enter the corner on.
    DegenerateIncomingLeg,
    /// The outgoing leg (corner → end point) has zero XY length.
    DegenerateOutgoingLeg,
    /// The legs are collinear and codirectional: there is no corner to blend.
    NoDeflection,
    /// The legs are exactly antiparallel. A symmetric clothoid pair through a 180° deflection needs
    /// an infinite tangent length, so no finite blend describes it.
    Reversal,
    /// The requested tangent length exceeds a leg, so the blend would start before the running
    /// position or end past the supplied end point. Refused rather than clamped (ADR 0002 §4).
    BlendExceedsLeg { blend: f64, leg: f64 },
    /// The corner has a solution over the reals but not in binary64: some produced coordinate
    /// overflowed. Refused here rather than left to `require_finite_toolpath`, so this path never
    /// hands a non-finite value to `Length::mm` — whose debug assertion ADR 0002 records as a known
    /// limit of the spline lowering. This is the postcondition of ADR 0002 §2, applied locally: an
    /// `is_finite` test on what was *computed*, not a magnitude policy on what was supplied.
    NotRepresentable,
}

impl std::fmt::Display for ClothoidError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClothoidError::DegenerateIncomingLeg => {
                write!(f, "clothoid corner needs a non-zero incoming leg in xy")
            }
            ClothoidError::DegenerateOutgoingLeg => {
                write!(f, "clothoid corner needs a non-zero outgoing leg in xy")
            }
            ClothoidError::NoDeflection => {
                write!(f, "clothoid corner needs a non-zero deflection")
            }
            ClothoidError::Reversal => write!(
                f,
                "clothoid corner cannot blend a 180 degree reversal with a finite tangent length"
            ),
            ClothoidError::BlendExceedsLeg { blend, leg } => write!(
                f,
                "clothoid blend {blend} mm exceeds the {leg} mm leg it is consumed from"
            ),
            ClothoidError::NotRepresentable => write!(
                f,
                "clothoid corner has no finite binary64 solution at this scale"
            ),
        }
    }
}

impl std::error::Error for ClothoidError {}

/// A solved symmetric clothoid corner, in absolute XY.
#[derive(Debug, Clone, PartialEq)]
pub struct CornerBlend {
    /// Where the blend leaves the incoming leg. Curvature is zero here, so the join with the straight
    /// leg is curvature-continuous — that is the whole point of the node.
    pub enter: [f64; 2],
    /// Where the blend rejoins the outgoing leg. Curvature is zero here too.
    pub exit: [f64; 2],
    /// The sampled blend: `2·SAMPLES` points, the first one *after* `enter` and the last one exactly
    /// `exit`. Uniform in arc length, because the Fresnel parameterisation is unit-speed.
    pub points: Vec<[f64; 2]>,
    /// Exact arc length of the blend (`2·A·σ`), as opposed to the shorter sampled polyline.
    pub length: f64,
    /// The clothoid parameter `A` of each half (`κ = s/A²`).
    pub a: f64,
    /// Signed deflection of the corner, radians in `(-π, π)`; positive turns left.
    pub deflection: f64,
}

/// The Fresnel integrals `Cf(τ) = ∫₀^τ cos(t²/2) dt` and `Sf(τ) = ∫₀^τ sin(t²/2) dt`.
///
/// Summed from the Maclaurin series of the integrand, integrated term by term:
///
/// ```text
/// Cf(τ) = Σₙ (-1)ⁿ τ^(4n+1) / (4ⁿ (2n)! (4n+1))
/// Sf(τ) = Σₙ (-1)ⁿ τ^(4n+3) / (2^(2n+1) (2n+1)! (4n+3))
/// ```
///
/// Each series is advanced by its term ratio rather than by recomputing factorials and powers, so a
/// term costs two multiplies and a divide and nothing overflows on the way to a small result.
pub fn fresnel(tau: f64) -> (f64, f64) {
    let (cf, sf, _, _) = fresnel_with_terms(tau);
    (cf, sf)
}

/// [`fresnel`], plus how many terms each series consumed.
///
/// One implementation, two entry points: the term counts exist so `crates/core/tests/clothoid.rs`
/// can *measure* the worst-case distance to [`FRESNEL_SERIES_MAX_TERMS`] over the op's domain rather
/// than assert a headroom nobody checked.
pub fn fresnel_with_terms(tau: f64) -> (f64, f64, usize, usize) {
    let t4 = tau * tau * tau * tau;

    // Cf: term₀ = τ; termₙ₊₁/termₙ = -τ⁴ (4n+1) / (4 (2n+1)(2n+2)(4n+5)).
    let mut c_term = tau;
    let mut cf = c_term;
    let mut c_used = 1;
    for n in 0..FRESNEL_SERIES_MAX_TERMS {
        let n = n as f64;
        c_term *=
            -t4 * (4.0 * n + 1.0) / (4.0 * (2.0 * n + 1.0) * (2.0 * n + 2.0) * (4.0 * n + 5.0));
        cf += c_term;
        c_used += 1;
        if c_term.abs() <= FRESNEL_SERIES_EPSILON * cf.abs() {
            break;
        }
    }

    // Sf: term₀ = τ³/6; termₙ₊₁/termₙ = -τ⁴ (4n+3) / (4 (2n+2)(2n+3)(4n+7)).
    let mut s_term = tau * tau * tau / 6.0;
    let mut sf = s_term;
    let mut s_used = 1;
    for n in 0..FRESNEL_SERIES_MAX_TERMS {
        let n = n as f64;
        s_term *=
            -t4 * (4.0 * n + 3.0) / (4.0 * (2.0 * n + 2.0) * (2.0 * n + 3.0) * (4.0 * n + 7.0));
        sf += s_term;
        s_used += 1;
        if s_term.abs() <= FRESNEL_SERIES_EPSILON * sf.abs() {
            break;
        }
    }

    (cf, sf, c_used, s_used)
}

/// Solve the symmetric clothoid corner at `corner` between the legs `start → corner → end`, with
/// `blend` mm of tangent length consumed from each leg.
///
/// The construction is the standard symmetric pair: two Euler spirals of equal parameter, each
/// turning half the deflection, meeting where curvature peaks, mirrored about the corner's bisector.
/// With `θ = |deflection|/2` and `σ = √(2θ)` (the normalised arc length at which a spiral has turned
/// `θ`), the endpoint of one half sits at `A·(Cf(σ), Sf(σ))` in its own start frame, and the tangent
/// length back to the corner is `T = A·(Cf(σ) + Sf(σ)·tan θ)`. That is *linear* in `A`, so
/// parameterising the node by `T` — the setback a machinist actually specifies, and the quantity that
/// has to fit inside the legs — inverts in closed form to `A = T / (Cf(σ) + Sf(σ)·tan θ)`, with no
/// iteration and therefore no solver tolerance. Parameterising by `A` instead would have made the
/// leg-fit check the iterative one, which is the wrong way round: the leg fit is what can fail.
///
/// All arithmetic is planar; `resolve` adds Z on top, linear in XY arc length, exactly as it does for
/// the helical rise of an [`crate::resolve::Op::Arc`].
pub fn corner_blend(
    start: [f64; 2],
    corner: [f64; 2],
    end: [f64; 2],
    blend: f64,
) -> Result<CornerBlend, ClothoidError> {
    let incoming = [corner[0] - start[0], corner[1] - start[1]];
    let outgoing = [end[0] - corner[0], end[1] - corner[1]];
    let incoming_len = libm::hypot(incoming[0], incoming[1]);
    let outgoing_len = libm::hypot(outgoing[0], outgoing[1]);
    if incoming_len <= 0.0 {
        return Err(ClothoidError::DegenerateIncomingLeg);
    }
    if outgoing_len <= 0.0 {
        return Err(ClothoidError::DegenerateOutgoingLeg);
    }
    // Refused before the geometry, not after: a blend longer than a leg has no symmetric solution
    // inside the corner the caller described, and clamping it to the leg would silently deliver a
    // different corner than the one asked for (ADR 0002 section 4).
    if blend > incoming_len {
        return Err(ClothoidError::BlendExceedsLeg {
            blend,
            leg: incoming_len,
        });
    }
    if blend > outgoing_len {
        return Err(ClothoidError::BlendExceedsLeg {
            blend,
            leg: outgoing_len,
        });
    }

    let u = [incoming[0] / incoming_len, incoming[1] / incoming_len];
    let w = [outgoing[0] / outgoing_len, outgoing[1] / outgoing_len];
    // Signed deflection in (-pi, pi]: atan2 of the cross and dot products of the two unit legs.
    let deflection = libm::atan2(u[0] * w[1] - u[1] * w[0], u[0] * w[0] + u[1] * w[1]);
    if deflection == 0.0 {
        return Err(ClothoidError::NoDeflection);
    }
    // Exactly antiparallel legs give atan2(0, -1) == PI. Near-reversals are *not* refused here: they
    // have a finite solution with a small A, and the leg check above is what bounds them.
    if deflection == std::f64::consts::PI {
        return Err(ClothoidError::Reversal);
    }

    let turn = if deflection < 0.0 { -1.0 } else { 1.0 };
    let theta = deflection.abs() / 2.0;
    let sigma = libm::sqrt(2.0 * theta);
    let (cf, sf) = fresnel(sigma);
    let a = blend / (cf + sf * libm::tan(theta));

    let enter = [corner[0] - u[0] * blend, corner[1] - u[1] * blend];
    let exit = [corner[0] + w[0] * blend, corner[1] + w[1] * blend];

    // The two halves are reflections of each other about the corner's bisector. That reflection maps
    // the entry frame `(enter; u, turn·u⊥)` onto `(exit; -w, turn·w⊥)`, so the second half is the
    // same spiral read backwards from `exit` — no separate mirroring arithmetic, and the two halves
    // meet at the joint to within the closure budget measured in `crates/core/tests/clothoid.rs`.
    let u_normal = [-u[1] * turn, u[0] * turn];
    let w_normal = [-w[1] * turn, w[0] * turn];
    let mut points = Vec::with_capacity(2 * SAMPLES);
    for step in 1..=SAMPLES {
        let tau = sigma * step as f64 / SAMPLES as f64;
        let (c, s) = fresnel(tau);
        points.push([
            enter[0] + a * c * u[0] + a * s * u_normal[0],
            enter[1] + a * c * u[1] + a * s * u_normal[1],
        ]);
    }
    for step in (0..SAMPLES).rev() {
        if step == 0 {
            // The spiral's own tau = 0 point *is* `exit`; use it verbatim so the blend ends exactly
            // on the outgoing leg, the same way a spline span ends exactly on its through-point.
            points.push(exit);
            continue;
        }
        let tau = sigma * step as f64 / SAMPLES as f64;
        let (c, s) = fresnel(tau);
        points.push([
            exit[0] - a * c * w[0] + a * s * w_normal[0],
            exit[1] - a * c * w[1] + a * s * w_normal[1],
        ]);
    }

    let solved = CornerBlend {
        enter,
        exit,
        points,
        length: 2.0 * a * sigma,
        a,
        deflection,
    };
    if !solved.is_finite() {
        return Err(ClothoidError::NotRepresentable);
    }
    Ok(solved)
}

impl CornerBlend {
    /// Every produced quantity is finite. Checked before the solve is handed back, so `resolve`'s
    /// lowering cannot build a `Length` from an overflowed coordinate.
    fn is_finite(&self) -> bool {
        self.a.is_finite()
            && self.length.is_finite()
            && self.enter.iter().all(|v| v.is_finite())
            && self.exit.iter().all(|v| v.is_finite())
            && self
                .points
                .iter()
                .all(|point| point.iter().all(|v| v.is_finite()))
    }
}

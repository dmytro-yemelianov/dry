//! P5.5 — the clothoid (Euler-spiral) corner-blend L1 node.
//!
//! Three kinds of test live here, kept apart on purpose:
//!
//! 1. **Against an oracle outside Dry.** The Fresnel series is checked against published values of
//!    the standard Fresnel integrals and, independently, against composite Simpson quadrature of the
//!    same integrand — a different algorithm, not a different spelling of this one. Dry's parser
//!    accepting Dry's output would establish nothing (ADR 0002 §5), so the accuracy claims rest on
//!    these two and nothing else.
//! 2. **Against the definition.** An Euler spiral *is* the curve whose curvature is linear in arc
//!    length. That is measured from the emitted points, not assumed from the formula.
//! 3. **Against the refusal contract.** Every degenerate corner is a named error, never a clamp and
//!    never a silently different corner (ADR 0002 §4).

use kmet_kernel::clothoid::{
    corner_blend, fresnel, fresnel_with_terms, ClothoidError, CornerBlend,
};
use kmet_kernel::{
    emit_stream, resolve_checked, Design, EmitParams, Op, ResolveParams, Segment, SegmentKind,
};
use std::f64::consts::PI;

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

fn resolved(ops: &str) -> Vec<Segment> {
    resolve_checked(&design(ops), &ResolveParams::default())
        .expect("design resolves")
        .segments
}

/// Composite Simpson on `f` over `[0, tau]`. An independent algorithm for the same integral: no
/// series, no shared code with `clothoid.rs`.
fn simpson(f: impl Fn(f64) -> f64, tau: f64, intervals: usize) -> f64 {
    let h = tau / intervals as f64;
    let mut total = f(0.0) + f(tau);
    for i in 1..intervals {
        let weight = if i % 2 == 1 { 4.0 } else { 2.0 };
        total += weight * f(h * i as f64);
    }
    total * h / 3.0
}

/// The largest argument the node can produce, and the value published as
/// `LIMIT.FRESNEL_ARGUMENT.upper`. A corner deflects by strictly less than 180 degrees, so
/// `sigma = sqrt(|deflection|)` is at most `sqrt` of the largest double below pi — which rounds to
/// the same double as `sqrt(f64::consts::PI)`. Pinned by
/// `the_swept_domain_is_the_published_domain`.
const SIGMA_MAX: f64 = 1.772_453_850_905_515_9;

/// The correctly rounded binary64 √π — one ulp **above** [`SIGMA_MAX`], and therefore one ulp
/// outside the node's own domain. Used at exactly one place: the Abramowitz & Stegun reference
/// values are tabulated at `z = 1` under the substitution `t = u·√π`, so reproducing them needs the
/// best √π available, not the largest argument the node happens to reach. Evaluating the series
/// there is a deliberate step outside the published domain by one ulp, and the FRESNEL_SERIES
/// boundary's exclusion list says so.
const ROOT_PI: f64 = 1.772_453_850_905_516;

/// The deflection/blend sweep every measured budget in this file is taken over, and therefore the
/// domain each published ceiling actually covers — which is *narrower* than the interval the node
/// admits, deliberately and with the gap published as `LIMIT.BUDGETED_DEFLECTION_RAD`; see
/// `the_shape_budgets_do_not_reach_the_ends_of_the_admitted_interval`. Both ends of the sweep are
/// near-degenerate on purpose: 179 degrees is one step from the refused reversal, 0.01 degrees is
/// one step from the refused straight line, and a 9.999 mm blend is one step from consuming the
/// whole 10 mm leg.
const DEGREES: [f64; 20] = [
    -179.0, -175.0, -170.0, -120.0, -90.0, -60.0, -30.0, -5.0, -1.0, -0.01, 0.01, 1.0, 5.0, 30.0,
    60.0, 90.0, 120.0, 170.0, 175.0, 179.0,
];
const BLENDS: [f64; 7] = [0.01, 0.1, 0.5, 1.0, 2.0, 5.0, 9.999];

/// The corner every sweep uses: 10 mm legs meeting at (10, 0), the second one rotated by `degrees`.
fn swept_corner(degrees: f64) -> ([f64; 2], [f64; 2], [f64; 2]) {
    deflected_corner(degrees.to_radians())
}

/// The same corner, taken in radians, so the tests that work at the ends of the admitted interval
/// can name a deflection an ulp from pi without going through a degree conversion that would round
/// it away.
fn deflected_corner(radians: f64) -> ([f64; 2], [f64; 2], [f64; 2]) {
    let corner = [10.0, 0.0];
    (
        [0.0, 0.0],
        corner,
        [
            corner[0] + 10.0 * libm::cos(radians),
            corner[1] + 10.0 * libm::sin(radians),
        ],
    )
}

#[test]
fn the_swept_domain_is_the_published_domain() {
    // SIGMA_MAX is a literal, and a literal that drifts from the thing it claims to be turns every
    // sweep below into a measurement of the wrong interval. Pin it against the largest argument the
    // node can actually construct: `sigma = sqrt(|deflection|)` with `|deflection|` at most the
    // largest double strictly below pi, since exactly pi is refused.
    let below_pi = f64::from_bits(PI.to_bits() - 1);
    assert_eq!(
        SIGMA_MAX,
        libm::sqrt(below_pi),
        "SIGMA_MAX is not sqrt of the largest admitted deflection"
    );
    assert_eq!(
        SIGMA_MAX,
        libm::sqrt(PI),
        "SIGMA_MAX is not the published LIMIT.FRESNEL_ARGUMENT.upper"
    );
    // ROOT_PI is deliberately the *next* double up: the A&S substitution needs the best sqrt(pi),
    // not the node's largest argument, and the difference is exactly one ulp.
    assert_eq!(
        ROOT_PI.to_bits(),
        SIGMA_MAX.to_bits() + 1,
        "ROOT_PI must be exactly one ulp above SIGMA_MAX"
    );
}

#[test]
fn fresnel_matches_published_fresnel_integral_values() {
    // Abramowitz & Stegun 7.3.1 defines C(z) = int_0^z cos(pi u^2 / 2) du. Substituting t = u*sqrt(pi)
    // gives Cf(z*sqrt(pi)) = sqrt(pi) * C(z), and likewise for S. The tabulated values at z = 1 are
    // C(1) = 0.7798934003768228 and S(1) = 0.4382591473903548; both were independently reproduced to
    // the last digit shown by a 60-digit Decimal evaluation while writing this test.
    //
    // Evaluated at ROOT_PI, the correctly rounded sqrt(pi), because that is what the substitution
    // names. One ulp lower — at SIGMA_MAX, the node's own domain top — the same comparison measures
    // 2.22e-16 and 1.67e-16 instead of 1.11e-16 and 0: the reference values are sharp enough that a
    // single ulp in the argument is visible in them, which is worth knowing and is why the two
    // constants are kept apart.
    let root_pi = ROOT_PI;
    let (cf, sf) = fresnel(root_pi);
    let published_c = 0.779_893_400_376_822_8_f64;
    let published_s = 0.438_259_147_390_354_8_f64;
    let c_error = (cf / root_pi - published_c).abs();
    let s_error = (sf / root_pi - published_s).abs();
    assert!(
        c_error <= 4e-16 && s_error <= 4e-16,
        "Fresnel C(1) error {c_error:e}, S(1) error {s_error:e}"
    );
}

#[test]
fn fresnel_matches_independent_quadrature_across_the_nodes_whole_domain() {
    // 2^16 Simpson intervals put the quadrature's own truncation error far below its rounding error
    // for this smooth integrand, so what this bounds is essentially the series against `libm::cos` /
    // `libm::sin` summed a completely different way.
    let mut worst_c: f64 = 0.0;
    let mut worst_s: f64 = 0.0;
    for step in 1..=200 {
        let tau = SIGMA_MAX * step as f64 / 200.0;
        let (cf, sf) = fresnel(tau);
        worst_c = worst_c.max((cf - simpson(|t| libm::cos(t * t / 2.0), tau, 1 << 16)).abs());
        worst_s = worst_s.max((sf - simpson(|t| libm::sin(t * t / 2.0), tau, 1 << 16)).abs());
    }
    // Measured on this build: see the ceiling published as
    // FM1.NUMERIC.PROFILE.RESOLVE.CLOTHOID.V0.BUDGET.FRESNEL_ABS_ERROR.
    assert!(
        worst_c <= 1e-13 && worst_s <= 1e-13,
        "Fresnel vs quadrature: worst Cf {worst_c:e}, worst Sf {worst_s:e}"
    );
}

#[test]
fn fresnel_series_terminates_well_inside_its_cap() {
    // The cap exists so the loop terminates on any input at all; the relative threshold is what
    // actually stops it in the node's domain. Measure the distance between the two, at both ends:
    // the published rationale quotes a range, and a range needs a floor as well as a ceiling.
    let mut worst = 0usize;
    let mut least = usize::MAX;
    for step in 0..=2000 {
        let tau = SIGMA_MAX * step as f64 / 2000.0;
        let (_, _, c_terms, s_terms) = fresnel_with_terms(tau);
        worst = worst.max(c_terms.max(s_terms));
        least = least.min(c_terms.min(s_terms));
    }
    assert!(
        worst <= 16,
        "worst-case Fresnel term count over the domain was {worst}"
    );
    // The loop body always runs at least once, so the floor is two terms and not one. Pinned
    // because the profile used to say one, which is unreachable.
    assert_eq!(least, 2, "fewest Fresnel terms over the domain");
    assert_eq!(
        fresnel_with_terms(0.0),
        (0.0, 0.0, 2, 2),
        "the series still costs two terms at the bottom of the domain"
    );
    // The cap counts iterations *past* the leading term, so a non-terminating argument costs 33
    // terms and not 32. Pinned so the constant's name and the number it produces stay in step.
    let (_, _, c_capped, s_capped) = fresnel_with_terms(f64::NAN);
    assert_eq!(
        (c_capped, s_capped),
        (33, 33),
        "the hard cap admits term_0 plus 32 iterations"
    );
}

/// Sample the blend and report the maximum deviation of the chord-turn second difference from the
/// constant an Euler spiral predicts, relative to that constant.
fn curvature_linearity_error(deflection_deg: f64, blend: f64) -> f64 {
    let (start, corner, end) = swept_corner(deflection_deg);
    let solved = corner_blend(start, corner, end, blend).expect("corner solves");
    curvature_linearity_of(&solved)
}

fn curvature_linearity_of(solved: &CornerBlend) -> f64 {
    // Samples are uniform in arc length because the Fresnel parameterisation is unit-speed, so the
    // chord between consecutive samples subtends a constant arc length h = length / (2*SAMPLES).
    let mut path = vec![solved.enter];
    path.extend_from_slice(&solved.points);
    let angles: Vec<f64> = path
        .windows(2)
        .map(|w| libm::atan2(w[1][1] - w[0][1], w[1][0] - w[0][0]))
        .collect();
    let turns: Vec<f64> = angles.windows(2).map(|w| w[1] - w[0]).collect();

    // Curvature linear in arc length => the tangent turns by a quantity that grows linearly, so the
    // second difference of chord angles is the constant h^2 / A^2. Curvature rises through the first
    // half and falls through the second, so the constant flips sign at the joint.
    //
    // `turns[15]` is the turn at the joint vertex itself, where curvature peaks rather than
    // continuing linearly; the two windows that contain it are excluded and checked separately by
    // `the_curvature_peak_sits_exactly_at_the_joint`.
    let joint = 15;
    let h = solved.length / (2.0 * 16.0);
    let predicted = h * h / (solved.a * solved.a);
    let mut worst: f64 = 0.0;
    for (index, pair) in turns.windows(2).enumerate() {
        if index == joint - 1 || index == joint {
            continue;
        }
        let sign = if index < joint { 1.0 } else { -1.0 };
        let observed = (pair[1] - pair[0]) * solved.deflection.signum() * sign;
        worst = worst.max((observed - predicted).abs() / predicted);
    }
    worst
}

#[test]
fn blend_curvature_is_linear_in_arc_length() {
    // This is the property that makes the node a clothoid rather than a fillet or a spline: the
    // tangent turns by an arithmetic progression, i.e. curvature grows linearly with arc length.
    let mut worst: f64 = 0.0;
    for degrees in DEGREES {
        for blend in BLENDS {
            worst = worst.max(curvature_linearity_error(degrees, blend));
        }
    }
    // Measured on this build; published as
    // FM1.NUMERIC.PROFILE.RESOLVE.CLOTHOID.V0.BUDGET.CURVATURE_LINEARITY_RELATIVE_ERROR.
    assert!(
        worst <= 1e-6,
        "worst relative curvature-linearity error {worst:e}"
    );
}

#[test]
fn the_sampled_polyline_stays_within_its_chord_budget_of_the_true_spiral() {
    // What the lowering hands downstream is a polyline, not a spiral. This measures the only error
    // that costs the user anything: how far the chords depart from the curve they sample, as a
    // fraction of the blend, which is the length scale the whole corner is built from.
    //
    // All 32 chords, not the first 16. The second half is generated by a *different* code path —
    // the exit frame, walking tau back down to zero — so covering only the first half would have
    // left half of the published budget's stated range unmeasured. The two halves are reconstructed
    // from their own frames here for the same reason.
    let mut worst_first: f64 = 0.0;
    let mut worst_second: f64 = 0.0;
    for degrees in DEGREES {
        for blend in BLENDS {
            let (start, corner, end) = swept_corner(degrees);
            let solved = corner_blend(start, corner, end, blend).expect("corner solves");
            let sigma = libm::sqrt(solved.deflection.abs());
            let turn = solved.deflection.signum();
            let leg = libm::hypot(corner[0] - start[0], corner[1] - start[1]);
            let u = [(corner[0] - start[0]) / leg, (corner[1] - start[1]) / leg];
            let u_normal = [-u[1] * turn, u[0] * turn];
            let outgoing = libm::hypot(end[0] - corner[0], end[1] - corner[1]);
            let w = [
                (end[0] - corner[0]) / outgoing,
                (end[1] - corner[1]) / outgoing,
            ];
            let w_normal = [-w[1] * turn, w[0] * turn];

            let mut path = vec![solved.enter];
            path.extend_from_slice(&solved.points);
            for step in 0..32 {
                let (p0, p1) = (path[step], path[step + 1]);
                let chord = [p1[0] - p0[0], p1[1] - p0[1]];
                let chord_len = libm::hypot(chord[0], chord[1]);
                if chord_len == 0.0 {
                    continue;
                }
                for k in 1..32 {
                    let along = step as f64 + k as f64 / 32.0;
                    let exact = if step < 16 {
                        let (c, s) = fresnel(sigma * along / 16.0);
                        [
                            solved.enter[0] + solved.a * c * u[0] + solved.a * s * u_normal[0],
                            solved.enter[1] + solved.a * c * u[1] + solved.a * s * u_normal[1],
                        ]
                    } else {
                        let (c, s) = fresnel(sigma * (32.0 - along) / 16.0);
                        [
                            solved.exit[0] - solved.a * c * w[0] + solved.a * s * w_normal[0],
                            solved.exit[1] - solved.a * c * w[1] + solved.a * s * w_normal[1],
                        ]
                    };
                    let offset = ((exact[0] - p0[0]) * chord[1] - (exact[1] - p0[1]) * chord[0])
                        .abs()
                        / chord_len;
                    if step < 16 {
                        worst_first = worst_first.max(offset / blend);
                    } else {
                        worst_second = worst_second.max(offset / blend);
                    }
                }
            }
        }
    }
    // Measured; published as
    // FM1.NUMERIC.PROFILE.RESOLVE.CLOTHOID.V0.BUDGET.CHORD_DEVIATION_PER_BLEND_MM. The two halves
    // are reported separately so a regression in one cannot hide behind the other.
    assert!(
        worst_first <= 1e-3 && worst_second <= 1e-3,
        "worst chord deviation / blend: first half {worst_first:e}, second half {worst_second:e}"
    );
}

#[test]
fn the_two_spiral_halves_meet_at_the_joint() {
    // The second half is generated as the same spiral read backwards from the exit point, never by
    // reflecting the first, so the two halves arriving at the same joint is a real closure condition
    // and not an identity. Rebuild the second half's joint point here, from the published fields and
    // the legs this test supplied, and measure how far apart the two constructions land.
    let mut worst: f64 = 0.0;
    for degrees in DEGREES {
        for blend in BLENDS {
            let (start, corner, end) = swept_corner(degrees);
            let solved = corner_blend(start, corner, end, blend).expect("corner solves");

            let outgoing = [end[0] - corner[0], end[1] - corner[1]];
            let len = libm::hypot(outgoing[0], outgoing[1]);
            let w = [outgoing[0] / len, outgoing[1] / len];
            let turn = solved.deflection.signum();
            let sigma = libm::sqrt(solved.deflection.abs());
            let (cf, sf) = fresnel(sigma);
            let from_exit = [
                solved.exit[0] - solved.a * cf * w[0] + solved.a * sf * (-w[1] * turn),
                solved.exit[1] - solved.a * cf * w[1] + solved.a * sf * (w[0] * turn),
            ];
            let from_enter = solved.points[15];
            worst = worst.max(libm::hypot(
                from_exit[0] - from_enter[0],
                from_exit[1] - from_enter[1],
            ));
        }
    }
    // Measured; published as FM1.NUMERIC.PROFILE.RESOLVE.CLOTHOID.V0.BUDGET.JOINT_CLOSURE_MM.
    assert!(worst <= 1e-14, "worst joint closure gap {worst:e} mm");
}

#[test]
fn the_curvature_peak_sits_exactly_at_the_joint() {
    // The complement of the linearity test: at the joint the tangent stops turning faster and starts
    // turning slower, so the chord-angle second difference there is 2/3 of the constant rather than
    // +/- it. That ratio is a property of a symmetric peak sampled uniformly; seeing it is how we
    // know the peak is at the joint and not one sample off.
    let solved = corner_blend([0.0, 0.0], [10.0, 0.0], [10.0, 10.0], 3.0).expect("corner solves");
    let mut path = vec![solved.enter];
    path.extend_from_slice(&solved.points);
    let angles: Vec<f64> = path
        .windows(2)
        .map(|w| libm::atan2(w[1][1] - w[0][1], w[1][0] - w[0][0]))
        .collect();
    let turns: Vec<f64> = angles.windows(2).map(|w| w[1] - w[0]).collect();
    let h = solved.length / 32.0;
    let predicted = h * h / (solved.a * solved.a);
    for (index, expected) in [(14usize, 2.0 / 3.0), (15, -2.0 / 3.0)] {
        let ratio = (turns[index + 1] - turns[index]) / predicted;
        assert!(
            (ratio - expected).abs() < 1e-3,
            "second difference at the joint window {index} was {ratio}, expected {expected}"
        );
    }
    // The peak turn is the joint's own, and it is the largest.
    let peak = turns
        .iter()
        .cloned()
        .fold(f64::MIN, |best, turn| best.max(turn));
    assert_eq!(peak, turns[15], "the largest turn is the one at the joint");
}

#[test]
fn blend_joins_its_legs_tangentially_and_symmetrically() {
    let solved = corner_blend([0.0, 0.0], [10.0, 0.0], [10.0, 10.0], 3.0).expect("corner solves");
    assert_eq!(
        solved.enter,
        [7.0, 0.0],
        "enter sits `blend` back along the leg"
    );
    assert_eq!(
        solved.exit,
        [10.0, 3.0],
        "exit sits `blend` along the outgoing leg"
    );
    assert!((solved.deflection - PI / 2.0).abs() < 1e-15);

    // Tangency: the first chord leaves along the incoming leg and the last arrives along the
    // outgoing one, to within the chord-vs-tangent error of a 16-sample half.
    let first = solved.points[0];
    let entry_angle = libm::atan2(first[1] - solved.enter[1], first[0] - solved.enter[0]);
    let last = solved.points[solved.points.len() - 1];
    let previous = solved.points[solved.points.len() - 2];
    let exit_angle = libm::atan2(last[1] - previous[1], last[0] - previous[0]);
    assert!(entry_angle.abs() < 2e-3, "entry chord angle {entry_angle}");
    assert!(
        (exit_angle - PI / 2.0).abs() < 2e-3,
        "exit chord angle {exit_angle}"
    );

    // Symmetry about the corner's bisector. For this right-angle corner at (10, 0) with legs +x and
    // +y, reflecting about the bisector maps (10 + dx, dy) to (10 - dy, -dx) — it carries `enter` to
    // `exit`, so on the full path (enter followed by the samples) it must carry point i to point
    // n-1-i. This is the strongest statement available about the two halves: not that they meet, but
    // that one is the other.
    let mut path = vec![solved.enter];
    path.extend_from_slice(&solved.points);
    let n = path.len();
    for i in 0..n {
        let a = path[i];
        let b = path[n - 1 - i];
        let mirrored = [10.0 - b[1], 10.0 - b[0]];
        assert!(
            libm::hypot(a[0] - mirrored[0], a[1] - mirrored[1]) < 1e-14,
            "point {i} is not the mirror of point {}",
            n - 1 - i
        );
    }
}

#[test]
fn a_clothoid_cornered_design_emits() {
    // The P5.5 acceptance sentence, checked end to end: L1 -> L2 -> g-code.
    let segments = resolved(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"clothoid","corner_x":20,"corner_y":0,"x":20,"y":20,"z":0.2,"blend":4}]"#,
    );
    // 1 positioning move + (1 incoming leg + 32 blend samples + 1 outgoing leg).
    assert_eq!(segments.len(), 35);
    assert!(segments[1..].iter().all(|s| s.kind == SegmentKind::Line));
    assert!(segments[1..].iter().all(|s| !s.travel));

    let end = segments.last().unwrap().end;
    assert_eq!(end[0].unwrap().value(), 20.0);
    assert_eq!(end[1].unwrap().value(), 20.0);

    let lines = emit_stream(segments.iter().cloned().map(Ok), &EmitParams::default())
        .expect("clothoid-cornered program emits");
    assert_eq!(lines.len(), 35);
    assert!(lines.iter().all(|line| line.starts_with('G')));

    // The corner is *cut*, not visited. Measured on the geometry, not on the text: the emitter
    // trims trailing zeros, so the string this used to look for ("X20.000 Y0.000") could not appear
    // in any program at all — a design that drives *straight through* the vertex emits `G1 X20`,
    // and the assertion passed on that too. The closest the blended path comes to (20, 0) is the
    // measured setback below; the unblended square corner reaches it exactly.
    let closest = segments
        .iter()
        .map(|s| libm::hypot(s.end[0].unwrap().value() - 20.0, s.end[1].unwrap().value()))
        .fold(f64::INFINITY, f64::min);
    assert!(
        (closest - 1.190_195_851_562_828_8).abs() < 1e-12,
        "closest approach to the vertex was {closest}, not the expected setback"
    );

    // The control: the same corner without the blend does visit the vertex, so the assertion above
    // is capable of failing. Without this, "the path avoids the vertex" is a claim about one path
    // with nothing to contrast it against.
    let square = resolved(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"move","x":20,"y":0},{"op":"move","x":20,"y":20}]"#,
    );
    assert!(
        square
            .iter()
            .any(|s| { s.end[0].unwrap().value() == 20.0 && s.end[1].unwrap().value() == 0.0 }),
        "the unblended control must pass through the vertex"
    );
}

#[test]
fn a_clothoid_carries_extrusion_and_process_state_like_any_other_move() {
    let segments = resolved(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"temperature","nozzle":215},{"op":"fan","speed":0.5},{"op":"tool","index":1},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"clothoid","corner_x":20,"corner_y":0,"x":20,"y":20,"z":0.2,"blend":4}]"#,
    );
    let blend = &segments[1..];
    assert!(blend.iter().all(|s| s.temperature == Some(215.0)));
    assert!(blend.iter().all(|s| s.fan == Some(0.5)));
    assert!(blend.iter().all(|s| s.tool == Some(1)));
    // volume = length * width * height for every sub-move, and filament follows the bead area.
    for s in blend {
        let expected = s.length.value() * 0.6 * 0.2;
        assert!((s.volume.value() - expected).abs() < 1e-12);
    }
    // The blend is shorter than going around the corner: that is what a corner blend is for. The
    // lower bound is the straight-line distance between the blend's own endpoints.
    let total: f64 = blend.iter().map(|s| s.length.value()).sum();
    assert!(
        total < 40.0 && total > 38.0,
        "blended corner length {total} should undercut the 40 mm square corner"
    );
}

#[test]
fn a_clothoid_rises_in_z_linearly_and_lands_on_the_commanded_point() {
    let segments = resolved(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"clothoid","corner_x":20,"corner_y":0,"x":20,"y":20,"z":1.2,"blend":4}]"#,
    );
    let last = segments.last().unwrap();
    // Exactly, not nearly: the final sample is the commanded point verbatim.
    assert_eq!(last.end[2].unwrap().value(), 1.2);

    // Monotone rise, and dz proportional to the xy length of each sub-move.
    let blend = &segments[1..];
    let total_xy: f64 = blend
        .iter()
        .map(|s| {
            libm::hypot(
                s.end[0].unwrap().value() - s.start[0].unwrap().value(),
                s.end[1].unwrap().value() - s.start[1].unwrap().value(),
            )
        })
        .sum();
    for s in blend {
        let xy = libm::hypot(
            s.end[0].unwrap().value() - s.start[0].unwrap().value(),
            s.end[1].unwrap().value() - s.start[1].unwrap().value(),
        );
        let dz = s.end[2].unwrap().value() - s.start[2].unwrap().value();
        assert!(dz > 0.0, "z must rise monotonically");
        assert!(
            (dz - 1.0 * xy / total_xy).abs() < 1e-12,
            "z rise {dz} is not proportional to the xy step {xy}"
        );
    }
}

#[test]
fn a_travel_clothoid_deposits_nothing() {
    let segments = resolved(
        r#"[{"op":"geometry","width":0.6,"height":0.2},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"clothoid","corner_x":20,"corner_y":0,"x":20,"y":20,"z":0.2,"blend":4}]"#,
    );
    assert!(segments[1..].iter().all(|s| s.travel));
    assert!(segments[1..].iter().all(|s| s.volume.value() == 0.0));
    assert!(segments[1..].iter().all(|s| s.filament.value() == 0.0));
}

#[test]
fn a_blend_that_consumes_a_whole_leg_emits_no_empty_leg() {
    let segments = resolved(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"clothoid","corner_x":10,"corner_y":0,"x":10,"y":20,"z":0.2,"blend":10}]"#,
    );
    // The incoming leg is entirely consumed, so only the blend and the outgoing leg remain.
    assert_eq!(segments.len(), 1 + 32 + 1);
}

// ---------------------------------------------------------------------------------------------
// Refusals. Each is a named error at ingress; none clamps, and none emits a different corner.
// ---------------------------------------------------------------------------------------------

fn refusal(ops: &str) -> String {
    resolve_checked(&design(ops), &ResolveParams::default())
        .expect_err("design must be refused")
        .to_string()
}

#[test]
fn a_blend_longer_than_its_leg_is_refused_not_clamped() {
    let message = refusal(
        r#"[{"op":"move","x":0,"y":0,"z":0.2},
            {"op":"clothoid","corner_x":10,"corner_y":0,"x":10,"y":20,"z":0.2,"blend":10.5}]"#,
    );
    assert!(
        message.contains("exceeds the 10 mm leg"),
        "unexpected message: {message}"
    );
}

#[test]
fn a_collinear_corner_is_refused() {
    let message = refusal(
        r#"[{"op":"move","x":0,"y":0,"z":0.2},
            {"op":"clothoid","corner_x":10,"corner_y":0,"x":20,"y":0,"z":0.2,"blend":1}]"#,
    );
    assert!(
        message.contains("non-zero deflection"),
        "unexpected message: {message}"
    );
}

#[test]
fn an_exact_reversal_is_refused_whichever_way_the_legs_point() {
    // All four axis-aligned orientations, not just `+x -> -x`.
    //
    // This test used to check that one alone, and it was the one that worked. For an exactly
    // antiparallel pair the cross product is a *signed* zero whose sign follows the legs'
    // orientation: `+x -> -x` and `-y -> +y` produce `+0.0`, `-x -> +x` and `+y -> -y` produce
    // `-0.0`. The guard was `deflection == PI`, and `atan2(-0.0, -1.0)` is `-PI`, so half of all
    // exact reversals walked straight through it and lowered to a 2.8e-16 mm blend with
    // `enter == exit` — a 9 mm extrude and a 9 mm retrace over the same line, with `verify`
    // reporting nothing.
    for (corner_x, corner_y, label) in [
        (10.0, 0.0, "+x -> -x"),
        (-10.0, 0.0, "-x -> +x"),
        (0.0, 10.0, "+y -> -y"),
        (0.0, -10.0, "-y -> +y"),
    ] {
        let message = refusal(&format!(
            r#"[{{"op":"move","x":0,"y":0,"z":0.2}},
                {{"op":"clothoid","corner_x":{corner_x},"corner_y":{corner_y},"x":0,"y":0,"z":0.2,"blend":1}}]"#
        ));
        assert!(
            message.contains("180 degree reversal"),
            "{label}: unexpected message: {message}"
        );
    }

    // And at every orientation in between, not only the axis-aligned ones — a rotated reversal must
    // not find a rounding crack the axis-aligned four happen to miss.
    for step in 0..720 {
        let angle = step as f64 * PI / 360.0;
        let corner = [10.0 * libm::cos(angle), 10.0 * libm::sin(angle)];
        let error = corner_blend([0.0, 0.0], corner, [0.0, 0.0], 1.0)
            .expect_err("a reversal must be refused at every orientation");
        assert_eq!(error, ClothoidError::Reversal, "at {step} half-degrees");
    }
}

#[test]
fn a_reversal_is_refused_before_it_can_lower_to_a_retrace() {
    // The refusal restated as the thing that goes wrong without it, from the published surface:
    // the corner that used to be admitted (`+y -> -y`, blend 1 mm) resolved to three segments —
    // position, 9 mm out, 9 mm back over the same line — and `verify` had nothing to say about it.
    let message = refusal(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":5,"y":5,"z":0.2},
            {"op":"clothoid","corner_x":5,"corner_y":15,"x":5,"y":5,"z":0.2,"blend":1}]"#,
    );
    assert!(
        message.contains("180 degree reversal"),
        "unexpected message: {message}"
    );
}

#[test]
fn a_near_reversal_that_rounds_onto_pi_is_refused_too() {
    // `atan2` maps the last half-ulp of its branch cut onto +-PI itself, so legs that are not
    // exactly antiparallel can still report a deflection of exactly PI. Those are refused as well,
    // which is what keeps the reported deflection inside the open interval `LIMIT.DEFLECTION_RAD`
    // publishes. One ulp further from the cut, the corner is admitted.
    let tiny = 1e-300;
    let refused = corner_blend([0.0, 0.0], [10.0, 0.0], [-1.0, tiny], 1.0)
        .expect_err("a deflection that rounds onto PI must be refused");
    assert_eq!(refused, ClothoidError::Reversal);

    let admitted = corner_blend([0.0, 0.0], [10.0, 0.0], [-1.0, 1e-8], 1.0)
        .expect("a deflection an ulp inside the cut still solves");
    assert!(
        admitted.deflection.abs() < PI,
        "reported deflection {} is not inside the open interval",
        admitted.deflection
    );
}

#[test]
fn the_shape_budgets_do_not_reach_the_ends_of_the_admitted_interval() {
    // A disclosure, pinned so it cannot rot: the deflection interval the node *admits* is wider
    // than the band its shape budgets were *measured* over, and the profile publishes both
    // (`LIMIT.DEFLECTION_RAD` and `LIMIT.BUDGETED_DEFLECTION_RAD`).
    //
    // Why they differ. `A = blend / (Cf + Sf*tan(theta))`, and as the deflection approaches 180
    // degrees `theta` approaches pi/2, where `tan` is ill-conditioned; the whole blend then shrinks
    // to a length the absolute coordinates cannot carry. One ulp below pi the sampled polyline is
    // no longer a clothoid to any published accuracy. That is a conditioning fact, not a policy
    // one, and it is *not* fixed by a threshold on the deflection: the same corner is inside the
    // budget at one blend and outside it at another (measured below), so no deflection band
    // separates the two.
    //
    // If this ever starts failing, the node got better and the published band should be widened —
    // do that rather than deleting the test.
    let below_pi = f64::from_bits(PI.to_bits() - 1);
    let (start, corner, end) = deflected_corner(below_pi);
    let solved = corner_blend(start, corner, end, 1.0).expect("one ulp inside the interval solves");
    assert!(
        curvature_linearity_of(&solved) > 1e-6,
        "one ulp below PI is now inside the published curvature ceiling"
    );

    // The same deflection, two blends, on opposite sides of the ceiling. This is the measurement
    // that rules out a deflection threshold as the remedy. Measured 4.2e-7 and 5.3e-4 against a
    // 1e-6 ceiling, so neither assertion sits on a knife edge.
    let (start, corner, end) = deflected_corner(PI - 1e-6);
    let wide = corner_blend(start, corner, end, 9.999).expect("corner solves");
    let narrow = corner_blend(start, corner, end, 0.01).expect("corner solves");
    assert!(
        curvature_linearity_of(&wide) <= 1e-6,
        "expected the 9.999 mm blend to stay inside the ceiling, got {:e}",
        curvature_linearity_of(&wide)
    );
    assert!(
        curvature_linearity_of(&narrow) > 1e-6,
        "expected the 0.01 mm blend at the same deflection to fall outside it, got {:e}",
        curvature_linearity_of(&narrow)
    );

    // What does hold everywhere the node admits: the corner still closes. The two halves are built
    // from independent frames, and the closure identity is self-consistent in `A`, so the joint gap
    // stays at the published JOINT_CLOSURE_MM ceiling right up to the refusal.
    let outgoing = libm::hypot(end[0] - corner[0], end[1] - corner[1]);
    let w = [
        (end[0] - corner[0]) / outgoing,
        (end[1] - corner[1]) / outgoing,
    ];
    let turn = narrow.deflection.signum();
    let (cf, sf) = fresnel(libm::sqrt(narrow.deflection.abs()));
    let from_exit = [
        narrow.exit[0] - narrow.a * cf * w[0] + narrow.a * sf * (-w[1] * turn),
        narrow.exit[1] - narrow.a * cf * w[1] + narrow.a * sf * (w[0] * turn),
    ];
    let gap = libm::hypot(
        from_exit[0] - narrow.points[15][0],
        from_exit[1] - narrow.points[15][1],
    );
    assert!(gap <= 1e-14, "joint gap near the reversal was {gap:e} mm");
}

#[test]
fn a_zero_length_leg_is_refused() {
    let incoming = refusal(
        r#"[{"op":"move","x":10,"y":0,"z":0.2},
            {"op":"clothoid","corner_x":10,"corner_y":0,"x":20,"y":5,"z":0.2,"blend":1}]"#,
    );
    assert!(
        incoming.contains("non-zero incoming leg"),
        "unexpected message: {incoming}"
    );
    let outgoing = refusal(
        r#"[{"op":"move","x":0,"y":0,"z":0.2},
            {"op":"clothoid","corner_x":10,"corner_y":0,"x":10,"y":0,"z":0.2,"blend":1}]"#,
    );
    assert!(
        outgoing.contains("non-zero outgoing leg"),
        "unexpected message: {outgoing}"
    );
}

#[test]
fn a_non_positive_or_non_finite_blend_is_refused() {
    for bad in ["0", "-1"] {
        let message = refusal(&format!(
            r#"[{{"op":"move","x":0,"y":0,"z":0.2}},
                {{"op":"clothoid","corner_x":10,"corner_y":0,"x":10,"y":10,"z":0.2,"blend":{bad}}}]"#
        ));
        assert!(
            message.contains("blend must be > 0"),
            "unexpected message for blend {bad}: {message}"
        );
    }
    // serde_json refuses the NaN/Infinity literals outright, so a non-finite blend can only arrive
    // through the Rust API; check the validator there rather than pretending JSON can carry it.
    let program = Design {
        ops: vec![
            Op::Move {
                x: Some(0.0),
                y: Some(0.0),
                z: Some(0.2),
            },
            Op::Clothoid {
                corner_x: 10.0,
                corner_y: 0.0,
                x: Some(10.0),
                y: Some(10.0),
                z: Some(0.2),
                blend: f64::INFINITY,
            },
        ],
    };
    let message = resolve_checked(&program, &ResolveParams::default())
        .expect_err("a non-finite blend must be refused")
        .to_string();
    assert!(
        message.contains("blend must be finite"),
        "unexpected message: {message}"
    );
}

#[test]
fn the_public_solver_refuses_a_non_positive_blend_on_its_own() {
    // `corner_blend` is re-exported from the crate root, so `validate_design`'s `require_positive`
    // is not the only way in. Without a rejection of its own it returned `Ok` for `blend <= 0`,
    // with a negative `a` and a negative `length` — a field documented as an arc length.
    for blend in [0.0, -0.0, -1.0, f64::NAN] {
        let error = corner_blend([0.0, 0.0], [10.0, 0.0], [10.0, 10.0], blend)
            .expect_err("a non-positive blend must be refused");
        assert!(
            matches!(error, ClothoidError::BlendNotPositive { .. }),
            "blend {blend} gave {error:?}"
        );
    }
    // A positive but infinite blend is a different refusal, and the accurate one: it does not fit
    // the leg it is consumed from.
    assert!(matches!(
        corner_blend([0.0, 0.0], [10.0, 0.0], [10.0, 10.0], f64::INFINITY),
        Err(ClothoidError::BlendExceedsLeg { .. })
    ));
    // The op path is unaffected: `validate_design` still refuses these first, with its own message.
    let message = refusal(
        r#"[{"op":"move","x":0,"y":0,"z":0.2},
            {"op":"clothoid","corner_x":10,"corner_y":0,"x":10,"y":10,"z":0.2,"blend":0}]"#,
    );
    assert!(
        message.contains("blend must be > 0"),
        "unexpected message: {message}"
    );
}

#[test]
fn an_unrepresentable_corner_is_refused_rather_than_panicking() {
    // A very long, very shallow corner: A = blend / (Cf + Sf*tan(theta)) overflows because the
    // denominator collapses with the deflection while the numerator does not. The node refuses
    // instead of handing an infinite coordinate to `Length::mm`, whose debug assertion would abort.
    let epsilon = 1e-9_f64;
    let corner = [1e307, 0.0];
    let end = [
        corner[0] + 1e307 * epsilon.cos(),
        corner[1] + 1e307 * epsilon.sin(),
    ];
    let error = corner_blend([0.0, 0.0], corner, end, 1e307).unwrap_err();
    assert_eq!(error, ClothoidError::NotRepresentable);
}

#[test]
fn a_clothoid_inside_a_placed_feature_is_transformed() {
    // Without an arm in `expand_feature_ops` the op would fall through the catch-all clone and a
    // placed corner would silently land in the wrong place, with the wrong handedness.
    let program: kmet_kernel::FeatureProgram = serde_json::from_str(
        r#"{"features":[{"kind":"feature","name":"corner",
             "pose":{"x":100,"y":0,"z":0,"rotate_z_deg":90},
             "ops":[{"op":"move","x":0,"y":0,"z":0.2},
                    {"op":"clothoid","corner_x":10,"corner_y":0,"x":10,"y":10,"z":0.2,"blend":2}]}]}"#,
    )
    .unwrap();
    let expanded = kmet_kernel::expand_features(&program).expect("feature expands");
    match expanded.ops[1] {
        Op::Clothoid {
            corner_x,
            corner_y,
            x,
            y,
            blend,
            ..
        } => {
            // A 90 degree rotation about Z then a +100 mm translation in x.
            assert!((corner_x - 100.0).abs() < 1e-12, "corner_x {corner_x}");
            assert!((corner_y - 10.0).abs() < 1e-12, "corner_y {corner_y}");
            assert!((x.unwrap() - 90.0).abs() < 1e-12, "x {x:?}");
            assert!((y.unwrap() - 10.0).abs() < 1e-12, "y {y:?}");
            // The *field* is copied, bit for bit — not that the corner is invariant, which is a
            // different and weaker statement; see
            // `a_blend_that_exactly_fills_its_leg_is_not_pose_stable`.
            assert_eq!(blend, 2.0, "the tangent length is copied, not recomputed");
        }
        ref other => panic!("expected a transformed clothoid, got {other:?}"),
    }
}

#[test]
fn a_blend_that_exactly_fills_its_leg_is_not_pose_stable() {
    // A pose is rigid over the reals; it is not rigid in binary64. `apply_xy` and `apply_point`
    // rotate with rounded cos/sin, so the leg lengths the placed corner is measured against move by
    // ulps while `blend` is copied through unchanged. A corner whose blend exactly fills its leg —
    // the inclusive top of LIMIT.BLEND_TO_LEG_RATIO — therefore validates unplaced and is refused
    // at some rotations.
    //
    // Refusing is correct: the placed corner really does have a leg shorter than the blend, and
    // admitting it would mean clamping to a corner nobody asked for. What was wrong was the claim
    // that this could not happen, which both the code comment and the APPLY.POINT boundary made.
    let placed = |degrees: f64| {
        let text = format!(
            r#"{{"features":[{{"kind":"feature","name":"corner",
                 "pose":{{"x":0,"y":0,"z":0,"rotate_z_deg":{degrees}}},
                 "ops":[{{"op":"move","x":0,"y":0,"z":0.2}},
                        {{"op":"clothoid","corner_x":10,"corner_y":0,"x":10,"y":10,"z":0.2,"blend":10}}]}}]}}"#
        );
        let program: kmet_kernel::FeatureProgram = serde_json::from_str(&text).unwrap();
        let expanded = kmet_kernel::expand_features(&program).expect("feature expands");
        resolve_checked(&Design { ops: expanded.ops }, &ResolveParams::default())
            .map(|toolpath| toolpath.segments.len())
            .map_err(|error| error.to_string())
    };

    // Unplaced, the blend exactly fills the leg and the corner is admitted.
    assert_eq!(placed(0.0), Ok(33));
    // Rotated by 30 degrees, the same corner's leg comes out an ulp short and it is refused.
    let refused = placed(30.0).expect_err("a 30 degree pose must refuse this corner");
    assert!(
        refused.contains("exceeds the 9.999999999999998 mm leg"),
        "unexpected message: {refused}"
    );
    // The refusals that *are* pose-stable, because they compare against zero rather than a length.
    for degrees in [0.0, 30.0, 45.0, 210.0] {
        let text = format!(
            r#"{{"features":[{{"kind":"feature","name":"corner",
                 "pose":{{"x":0,"y":0,"z":0,"rotate_z_deg":{degrees}}},
                 "ops":[{{"op":"move","x":0,"y":0,"z":0.2}},
                        {{"op":"clothoid","corner_x":10,"corner_y":0,"x":20,"y":0,"z":0.2,"blend":1}}]}}]}}"#
        );
        let program: kmet_kernel::FeatureProgram = serde_json::from_str(&text).unwrap();
        let expanded = kmet_kernel::expand_features(&program).expect("feature expands");
        let message = resolve_checked(&Design { ops: expanded.ops }, &ResolveParams::default())
            .expect_err("a collinear corner is refused at every pose")
            .to_string();
        assert!(
            message.contains("non-zero deflection"),
            "at {degrees} degrees: {message}"
        );
    }
}

#[test]
fn a_clothoid_in_a_feature_needs_a_defined_local_start() {
    // The arc and spline arms have this same first-error ordering, and both are mutation-covered by
    // `proofs/feature-refinement-mutations-v0.toml` against a generated Lean witness corpus. The
    // clothoid arm is not: that corpus has no clothoid op, so there is no fixture that could kill a
    // reordering mutation here. This test is the substitute, and it is weaker — it pins the refusal,
    // not the order in which the two checks fire.
    let program: kmet_kernel::FeatureProgram = serde_json::from_str(
        r#"{"features":[{"kind":"feature","name":"corner",
             "ops":[{"op":"clothoid","corner_x":10,"corner_y":0,"x":10,"y":10,"z":0.2,"blend":2}]}]}"#,
    )
    .unwrap();
    let message = kmet_kernel::expand_features(&program)
        .expect_err("a clothoid with no local start must be refused")
        .to_string();
    assert!(
        message.contains("requires a fully defined local start point"),
        "unexpected message: {message}"
    );
}

#[test]
fn the_op_round_trips_through_its_json_wire_form() {
    let text =
        r#"{"op":"clothoid","corner_x":10.0,"corner_y":0.0,"x":10.0,"y":10.0,"z":0.2,"blend":2.0}"#;
    let op: Op = serde_json::from_str(text).unwrap();
    assert_eq!(serde_json::to_string(&op).unwrap(), text);
}

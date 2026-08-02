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

use dry_core::clothoid::{corner_blend, fresnel, fresnel_with_terms, ClothoidError};
use dry_core::{
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

// The largest argument the node can produce: a corner deflects by less than 180 degrees, so
// theta < pi/2 and sigma = sqrt(2*theta) < sqrt(pi).
const SIGMA_MAX: f64 = 1.772_453_850_905_516;

/// The deflection/blend sweep every measured budget in this file is taken over, and therefore the
/// domain each published ceiling actually covers. Both ends are near-degenerate on purpose: 179
/// degrees is one step from the refused reversal, 0.01 degrees is one step from the refused straight
/// line, and a 9.999 mm blend is one step from consuming the whole 10 mm leg.
const DEGREES: [f64; 20] = [
    -179.0, -175.0, -170.0, -120.0, -90.0, -60.0, -30.0, -5.0, -1.0, -0.01, 0.01, 1.0, 5.0, 30.0,
    60.0, 90.0, 120.0, 170.0, 175.0, 179.0,
];
const BLENDS: [f64; 7] = [0.01, 0.1, 0.5, 1.0, 2.0, 5.0, 9.999];

/// The corner every sweep uses: 10 mm legs meeting at (10, 0), the second one rotated by `degrees`.
fn swept_corner(degrees: f64) -> ([f64; 2], [f64; 2], [f64; 2]) {
    let angle = degrees.to_radians();
    let corner = [10.0, 0.0];
    (
        [0.0, 0.0],
        corner,
        [
            corner[0] + 10.0 * angle.cos(),
            corner[1] + 10.0 * angle.sin(),
        ],
    )
}

#[test]
fn fresnel_matches_published_fresnel_integral_values() {
    // Abramowitz & Stegun 7.3.1 defines C(z) = int_0^z cos(pi u^2 / 2) du. Substituting t = u*sqrt(pi)
    // gives Cf(z*sqrt(pi)) = sqrt(pi) * C(z), and likewise for S. The tabulated values at z = 1 are
    // C(1) = 0.7798934003768228 and S(1) = 0.4382591473903548; both were independently reproduced to
    // the last digit shown by a 60-digit Decimal evaluation while writing this test.
    let root_pi = SIGMA_MAX;
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
    // actually stops it in the node's domain. Measure the distance between the two.
    let mut worst = 0usize;
    for step in 0..=2000 {
        let tau = SIGMA_MAX * step as f64 / 2000.0;
        let (_, _, c_terms, s_terms) = fresnel_with_terms(tau);
        worst = worst.max(c_terms.max(s_terms));
    }
    assert!(
        worst <= 16,
        "worst-case Fresnel term count over the domain was {worst}"
    );
}

/// Sample the blend and report the maximum deviation of the chord-turn second difference from the
/// constant an Euler spiral predicts, relative to that constant.
fn curvature_linearity_error(deflection_deg: f64, blend: f64) -> f64 {
    let (start, corner, end) = swept_corner(deflection_deg);
    let solved = corner_blend(start, corner, end, blend).expect("corner solves");

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
    let mut worst: f64 = 0.0;
    for degrees in DEGREES {
        for blend in BLENDS {
            let (start, corner, end) = swept_corner(degrees);
            let solved = corner_blend(start, corner, end, blend).expect("corner solves");
            let sigma = libm::sqrt(solved.deflection.abs());
            let turn = solved.deflection.signum();
            let leg = libm::hypot(corner[0] - start[0], corner[1] - start[1]);
            let u = [(corner[0] - start[0]) / leg, (corner[1] - start[1]) / leg];
            let normal = [-u[1] * turn, u[0] * turn];

            let mut path = vec![solved.enter];
            path.extend_from_slice(&solved.points);
            for step in 0..16 {
                let (p0, p1) = (path[step], path[step + 1]);
                let chord = [p1[0] - p0[0], p1[1] - p0[1]];
                let chord_len = libm::hypot(chord[0], chord[1]);
                if chord_len == 0.0 {
                    continue;
                }
                for k in 1..32 {
                    let tau = sigma * (step as f64 + k as f64 / 32.0) / 16.0;
                    let (c, s) = fresnel(tau);
                    let exact = [
                        solved.enter[0] + solved.a * c * u[0] + solved.a * s * normal[0],
                        solved.enter[1] + solved.a * c * u[1] + solved.a * s * normal[1],
                    ];
                    let offset = ((exact[0] - p0[0]) * chord[1] - (exact[1] - p0[1]) * chord[0])
                        .abs()
                        / chord_len;
                    worst = worst.max(offset / blend);
                }
            }
        }
    }
    // Measured; published as
    // FM1.NUMERIC.PROFILE.RESOLVE.CLOTHOID.V0.BUDGET.CHORD_DEVIATION_PER_BLEND_MM.
    assert!(worst <= 1e-3, "worst chord deviation / blend {worst:e}");
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

    // The corner is *cut*, not visited: the blend keeps the path strictly inside the vertex.
    let hits_corner = lines.iter().any(|line| line.contains("X20.000 Y0.000"));
    assert!(!hits_corner, "the blend must not pass through the vertex");
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
fn an_exact_reversal_is_refused() {
    let message = refusal(
        r#"[{"op":"move","x":0,"y":0,"z":0.2},
            {"op":"clothoid","corner_x":10,"corner_y":0,"x":0,"y":0,"z":0.2,"blend":1}]"#,
    );
    assert!(
        message.contains("180 degree reversal"),
        "unexpected message: {message}"
    );
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
    let program: dry_core::FeatureProgram = serde_json::from_str(
        r#"{"features":[{"kind":"feature","name":"corner",
             "pose":{"x":100,"y":0,"z":0,"rotate_z_deg":90},
             "ops":[{"op":"move","x":0,"y":0,"z":0.2},
                    {"op":"clothoid","corner_x":10,"corner_y":0,"x":10,"y":10,"z":0.2,"blend":2}]}]}"#,
    )
    .unwrap();
    let expanded = dry_core::expand_features(&program).expect("feature expands");
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
            assert_eq!(blend, 2.0, "a rigid pose leaves a tangent length alone");
        }
        ref other => panic!("expected a transformed clothoid, got {other:?}"),
    }
}

#[test]
fn a_clothoid_in_a_feature_needs_a_defined_local_start() {
    // The arc and spline arms have this same first-error ordering, and both are mutation-covered by
    // `proofs/feature-refinement-mutations-v0.toml` against a generated Lean witness corpus. The
    // clothoid arm is not: that corpus has no clothoid op, so there is no fixture that could kill a
    // reordering mutation here. This test is the substitute, and it is weaker — it pins the refusal,
    // not the order in which the two checks fire.
    let program: dry_core::FeatureProgram = serde_json::from_str(
        r#"{"features":[{"kind":"feature","name":"corner",
             "ops":[{"op":"clothoid","corner_x":10,"corner_y":0,"x":10,"y":10,"z":0.2,"blend":2}]}]}"#,
    )
    .unwrap();
    let message = dry_core::expand_features(&program)
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

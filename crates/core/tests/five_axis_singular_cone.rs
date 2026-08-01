//! #180 gap 3 — the BC/AC singular cone must not swing the C axis.
//!
//! `Ac` and `Bc` recover `C = atan2(j, i)`. Where the tool points along ±Z the tilt `hypot(i, j)` is
//! zero and `C` is not determined by the direction at all; `atan2(0, 0)` returns `0`, which is a C
//! library return value rather than a choice. A path that tilts, passes through straight-up and
//! tilts back therefore used to emit `C90 → C0 → C90` — a 180° round trip mid-cut, at cutting feed,
//! with `B = 0` on the offending line so the rotation could not affect the tool direction at all.
//!
//! **Policy pinned here.** Inside the cone (`hypot(i, j) <= 1e-9`, `SINGULAR_CONE_SIN_TILT`) the
//! previously determined `C` is *held*, for the linear axes as well as the rotary word. The first
//! move seeds `C = 0`, the identity — there is no previous orientation and the program cannot know
//! where the operator left the axis. So a program that *starts* inside the cone is unchanged, and a
//! program that *enters* it holds. Travels are motion like any other and hold too.

// These exercise the deprecated infallible `emit()` on purpose: it is still the entry point the
// in-tree call sites use, and refusing the whole program is part of what is under test here.
#![allow(deprecated)]

use dry_core::{emit, resolve, Design, EmitParams, Kinematics, ResolveParams};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

fn model(name: &str) -> Kinematics {
    Kinematics::named(name).unwrap()
}

fn five_axis(kinematics: Kinematics) -> EmitParams {
    EmitParams {
        five_axis: true,
        kinematics,
        ..EmitParams::default()
    }
}

/// A path tilted ~80° toward +Y, through straight-up, then back to the same tilt.
fn through_the_cone(extruding_middle: bool) -> Design {
    let j = (80.0f64).to_radians().sin();
    let k = (80.0f64).to_radians().cos();
    let middle = if extruding_middle {
        ""
    } else {
        r#"{"op":"extruder","on":false},"#
    };
    let resume = if extruding_middle {
        ""
    } else {
        r#"{"op":"extruder","on":true},"#
    };
    design(&format!(
        r#"[
            {{"op":"geometry","width":0.6,"height":0.2}},
            {{"op":"extruder","on":true}},
            {{"op":"speed","print":1000}},
            {{"op":"move","x":0,"y":0,"z":0.2}},
            {{"op":"orient","i":0.0,"j":{j},"k":{k}}},
            {{"op":"move","x":0,"y":10,"z":0.2}},
            {middle}{{"op":"orient","i":0.0,"j":0.0,"k":1.0}},
            {{"op":"move","x":20,"y":0,"z":0.2}},
            {resume}{{"op":"orient","i":0.0,"j":{j},"k":{k}}},
            {{"op":"move","x":0,"y":30,"z":0.2}}
        ]"#
    ))
}

/// Three genuinely tilted orientations, none within a decade of the cone.
fn never_near_the_cone() -> Design {
    design(
        r#"[
            {"op":"geometry","width":0.6,"height":0.2},
            {"op":"extruder","on":true},
            {"op":"speed","print":1000},
            {"op":"orient","i":0.6,"j":0.0,"k":0.8},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"move","x":10,"y":0,"z":0.2},
            {"op":"orient","i":0.0,"j":0.6,"k":0.8},
            {"op":"move","x":10,"y":10,"z":0.2},
            {"op":"orient","i":0.2,"j":0.6,"k":0.76},
            {"op":"move","x":20,"y":10,"z":0.4}
        ]"#,
    )
}

/// Read the modal rotary/linear words off an emitted program: `X`/`Y`/`Z`/`B`/`C` are only written
/// when they change, so a line's machine state is the accumulated one.
fn modal_states(gcode: &[String]) -> Vec<[f64; 5]> {
    let mut state = [0.0f64; 5];
    let mut out = Vec::new();
    for line in gcode {
        for word in line.split(' ') {
            let (letter, rest) = word.split_at(1);
            let slot = match letter {
                "X" => 0,
                "Y" => 1,
                "Z" => 2,
                "B" => 3,
                "C" => 4,
                _ => continue,
            };
            if let Ok(value) = rest.parse::<f64>() {
                state[slot] = value;
            }
        }
        out.push(state);
    }
    out
}

#[test]
fn singular_cone_holds_the_previous_c_instead_of_collapsing_to_zero() {
    let tp = resolve(&through_the_cone(true), &ResolveParams::default());
    let gcode = emit(&tp, &five_axis(model("bc")));

    // Measured before the fix:
    //   G1 F1000 X0 Y0 Z0.2 C0 B0 E0
    //   G1 X-1.53952 Y0 Z9.882807 C90 B80 E0.498902
    //   G1 X20 Y0 Z0.2 C0 B0 E1.115579          <- the cone: C swings 90° out and 90° back
    //   G1 X-5.012484 Y0 Z29.578962 C90 B80 E1.798817
    assert_eq!(
        gcode,
        vec![
            "G1 F1000 X0 Y0 Z0.2 C0 B0 E0",
            "G1 X-1.53952 Y0 Z9.882807 C90 B80 E0.498902",
            "G1 X0 Y20 Z0.2 B0 E1.115579",
            "G1 X-5.012484 Y0 Z29.578962 B80 E1.798817",
        ]
    );

    // The C sequence after the identity-seeded first move is 90 → 90 → 90: the word is simply absent
    // once it stops changing, which is the whole point.
    let c: Vec<f64> = modal_states(&gcode).iter().map(|s| s[4]).collect();
    assert_eq!(c, vec![0.0, 90.0, 90.0, 90.0]);
}

/// Holding C is only correct if the *linear* axes are expressed in the frame C actually puts the
/// table in. They are computed from the same resolved angles, so inverting the BC forward transform
/// on the emitted words must recover the programmed WCS point.
#[test]
fn the_held_c_line_still_describes_the_programmed_wcs_point() {
    let tp = resolve(&through_the_cone(true), &ResolveParams::default());
    let gcode = emit(&tp, &five_axis(model("bc")));
    let cone_line = modal_states(&gcode)[2];
    let [x, y, z, b_deg, c_deg] = cone_line;

    // Zero pivot ⇒ MCS = R_y(b) · R_z(c) · p, so p = R_z(-c) · R_y(-b) · MCS.
    let (b, c) = (b_deg.to_radians(), c_deg.to_radians());
    let (sb, cb) = (b.sin(), b.cos());
    let unrolled = [cb * x - sb * z, y, sb * x + cb * z];
    let (sc, cc) = (c.sin(), c.cos());
    let wcs = [
        cc * unrolled[0] + sc * unrolled[1],
        -sc * unrolled[0] + cc * unrolled[1],
        unrolled[2],
    ];

    let programmed = [20.0, 0.0, 0.2];
    for axis in 0..3 {
        assert!(
            (wcs[axis] - programmed[axis]).abs() < 1e-9,
            "axis {axis}: emitted words map back to {wcs:?}, programmed {programmed:?}"
        );
    }
    assert_eq!(c_deg, 90.0, "the cone line must carry the held C");
}

/// A rounding-level component is not a direction. Before the fix `[-1e-17, 0, 1]` left the cone by
/// the strictest possible margin and emitted `C180` — a full half turn, with the linear axes
/// following it from `X20` to `X-20`.
#[test]
fn a_rounding_level_component_no_longer_flips_c_by_180_degrees() {
    let mut tp = resolve(&through_the_cone(true), &ResolveParams::default());
    for segment in &mut tp.segments {
        if segment.orientation == Some([0.0, 0.0, 1.0]) {
            segment.orientation = Some([-1e-17, 0.0, 1.0]);
        }
    }
    let perturbed = emit(&tp, &five_axis(model("bc")));

    let clean = emit(
        &resolve(&through_the_cone(true), &ResolveParams::default()),
        &five_axis(model("bc")),
    );
    assert_eq!(
        perturbed, clean,
        "a 1e-17 component must not change the program"
    );
}

/// The cone is entered by a rapid here, not a cutting move. A travel is motion like any other: it
/// holds, so the two identically tilted cuts on either side of it need no C move at all.
#[test]
fn the_cone_is_held_across_a_travel() {
    let tp = resolve(&through_the_cone(false), &ResolveParams::default());
    let gcode = emit(&tp, &five_axis(model("bc")));
    let travel = gcode
        .iter()
        .find(|line| line.starts_with("G0"))
        .expect("the middle move must be a rapid");
    assert!(
        !travel.split(' ').any(|word| word.starts_with('C')),
        "a travel through the cone must not carry a C word: {travel}"
    );
    let c: Vec<f64> = modal_states(&gcode).iter().map(|s| s[4]).collect();
    assert_eq!(c, vec![0.0, 90.0, 90.0, 90.0]);
}

/// A program that *starts* inside the cone has no previous C, so it seeds the identity — which is
/// exactly what it emitted before this policy existed.
#[test]
fn a_program_that_starts_inside_the_cone_seeds_c_at_the_identity() {
    for name in ["ac", "bc"] {
        let tp = resolve(&through_the_cone(true), &ResolveParams::default());
        let gcode = emit(&tp, &five_axis(model(name)));
        assert!(
            gcode[0].split(' ').any(|word| word == "C0"),
            "{name}: the first move must seed C0: {}",
            gcode[0]
        );
    }
}

/// The regression guard: ordinary 5-axis output must be untouched. These three programs were
/// captured from the build immediately before the singular-cone policy landed.
#[test]
fn a_path_that_never_enters_the_cone_is_byte_identical() {
    let tp = resolve(&never_near_the_cone(), &ResolveParams::default());

    assert_eq!(
        emit(&tp, &five_axis(model("bc"))),
        vec![
            "G1 F1000 X0.12 Y0 Z0.16 C0 B36.869898 E0",
            "G1 X8.12 Y0 Z-5.84 E0.498902",
            "G1 X-7.88 Y10 Z6.16 C90 E0.498902",
            "G1 X-2.174845 Y22.135944 Z2.330247 C71.565051 B39.766494 E0.499002",
        ]
    );
    assert_eq!(
        emit(&tp, &five_axis(model("ac"))),
        vec![
            "G1 F1000 X0 Y-0.12 Z0.16 C0 A36.869898 E0",
            "G1 X10 Y-0.12 Z0.16 E0.498902",
            "G1 X-10 Y7.88 Z6.16 C90 E0.498902",
            "G1 X-3.162278 Y16.7591 Z14.466947 C71.565051 A39.766494 E0.499002",
        ]
    );
    // The Ab model has no C axis and is untouched by the policy — including its own analogous
    // singularity at the tool along ±Y, which this slice deliberately does not address.
    assert_eq!(
        emit(&tp, &five_axis(model("ab"))),
        vec![
            "G1 F1000 X0 Y0 Z0.2 A0 B36.869898 E0",
            "G1 X10 Y0 Z0.2 E0.498902",
            "G1 X10 Y10 Z0.2 A36.869898 B0 E0.498902",
            "G1 X20 Y10 Z0.4 A37.361006 B14.743563 E0.499002",
        ]
    );
}

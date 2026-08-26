//! A contract that cannot decide anything must not be reported as a rule that ran.
//!
//! `Report::rules_evaluated` exists so a vacuous pass is not byte-identical to a real one (H1.3
//! design section 3.5). A NaN ceiling defeated that: every ordering comparison against NaN is false,
//! so the rule could never fire, yet it was still listed as evaluated — a clean report for a check
//! that never happened.

use dry_core::{resolve, verify, Contracts, Design, Op, ResolveParams};

fn extruding_toolpath() -> dry_core::Toolpath {
    let mut design = Design::default();
    design.ops.push(Op::Geometry {
        width: Some(0.6),
        height: Some(0.2),
    });
    design.ops.push(Op::Extruder { on: true });
    design.ops.push(Op::Move {
        x: Some(0.0),
        y: Some(0.0),
        z: Some(0.2),
    });
    design.ops.push(Op::Move {
        x: Some(50.0),
        y: Some(0.0),
        z: Some(0.2),
    });
    resolve(&design, &ResolveParams::default())
}

#[test]
fn a_real_ceiling_is_evaluated_and_can_fire() {
    let toolpath = extruding_toolpath();
    let contracts = Contracts {
        max_flow: Some(0.0001),
        ..Default::default()
    };
    let report = verify(&toolpath, &contracts);
    assert!(
        report.rules_evaluated.iter().any(|r| r.contains("flow")),
        "a finite ceiling must be in force"
    );
    assert!(
        report.findings.iter().any(|f| f.rule.contains("flow")),
        "a ceiling this low must produce findings"
    );
}

#[test]
fn a_non_finite_ceiling_is_not_reported_as_evaluated() {
    let toolpath = extruding_toolpath();
    for ceiling in [f64::NAN, f64::INFINITY] {
        let contracts = Contracts {
            max_flow: Some(ceiling),
            ..Default::default()
        };
        let report = verify(&toolpath, &contracts);
        assert!(
            !report.rules_evaluated.iter().any(|r| r.contains("flow")),
            "a {ceiling} ceiling cannot decide anything, so the rule must not be listed as evaluated"
        );
        assert!(
            !report.findings.iter().any(|f| f.rule.contains("flow")),
            "and it must not produce findings either"
        );
    }
}

/// The same for the range and volume contracts, which are three and six numbers respectively:
/// one unusable component makes the whole contract unable to decide.
#[test]
fn partially_non_finite_range_and_bounds_are_not_in_force() {
    let toolpath = extruding_toolpath();

    let speed = Contracts {
        speed_range: Some([60.0, f64::NAN]),
        ..Default::default()
    };
    assert!(
        !verify(&toolpath, &speed)
            .rules_evaluated
            .iter()
            .any(|r| r.contains("speed")),
        "a range with one unusable end is not a contract"
    );

    let bounds = Contracts {
        bounds: Some([[0.0, 100.0], [0.0, f64::INFINITY], [0.0, 100.0]]),
        ..Default::default()
    };
    assert!(
        !verify(&toolpath, &bounds)
            .rules_evaluated
            .iter()
            .any(|r| r.contains("bounds")),
        "a build volume with one unusable face is not a contract"
    );

    // The finite forms of both are still in force.
    let ok = Contracts {
        speed_range: Some([60.0, 9000.0]),
        bounds: Some([[0.0, 100.0], [0.0, 100.0], [0.0, 100.0]]),
        ..Default::default()
    };
    let report = verify(&toolpath, &ok);
    assert!(report.rules_evaluated.iter().any(|r| r.contains("speed")));
    assert!(report.rules_evaluated.iter().any(|r| r.contains("bounds")));
}

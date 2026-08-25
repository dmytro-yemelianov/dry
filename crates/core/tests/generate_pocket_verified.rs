//! The pocket generator's verify-dependent assertion.
//!
//! `generate::pocket`'s `slot_at_exact_tool_width_still_cuts` made two claims about the same
//! fixture: that a slot exactly one tool wide still cuts its whole floor, and that the result is
//! *verifier-clean* rather than merely resolvable — which is the claim that matters, because
//! Profile mode expresses such a slot as a zero-width ring. `verify` is layer 2 and is not
//! reachable from `kmet-kernel`, where the generator lives, so the second claim runs here, over the
//! same fixture and with the same assertions; the first stayed in the kernel (plan Task 4).
//!
//! The split is what avoids copying `interior_samples` and the three exact point-to-path distance
//! helpers — ~140 lines whose whole value is being the single trusted implementation — out of the
//! kernel's test module, where two other coverage tests still use them.

use dry_core::{
    resolve_checked, try_pocket_ops, verify, Contracts, CutMode, Design, PocketOptions,
    PocketShape, ResolveParams, RuleId,
};

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

#[test]
fn slot_at_exact_tool_width_verifies_clean() {
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
        let tp = resolve_checked(&d, &ResolveParams::default())
            .unwrap_or_else(|e| panic!("{mode:?}: slot must resolve cleanly: {e:?}"));
        let report = verify(&tp, &Contracts::default());
        assert!(report.ok(), "{mode:?}: slot must verify clean: {report:?}");
        // State what "clean" covers here: this claim is geometric, so name the rules that carry
        // it rather than leaning on `ok()`, which is also true of a pass that inspected nothing.
        assert!(report.segments_inspected > 0, "{mode:?}: nothing inspected");
        for rule in [
            RuleId::Continuity,
            RuleId::SegmentLength,
            RuleId::ArcLength,
            RuleId::NegativeQuantity,
            RuleId::FilamentConsistency,
        ] {
            assert!(
                report.evaluated(rule),
                "{mode:?}: {} was not in force",
                rule.as_str()
            );
        }
    }
}

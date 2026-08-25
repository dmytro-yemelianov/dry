//! The TPMS generator's verify-dependent tests.
//!
//! Both assertions below run the generated design through `verify`, which is layer 2 and therefore
//! not reachable from `kmet-kernel`, where the generator itself lives. They ran as
//! `generate::tpms`'s inline `#[cfg(test)]` tests until the kernel was split out; the facade is the
//! lowest crate that can still see both layers, so they run here now, unchanged (plan Task 4).

use dry_core::{
    resolve_checked, simulate, tpms_design, tpms_ops, verify, Contracts, Op, ResolveParams,
    Surface, TpmsOptions,
};

/// A small single-cell gyroid at default resolution — fast, but with real contours per layer.
fn small_cell() -> TpmsOptions {
    TpmsOptions {
        cells_x: Some(1),
        cells_y: Some(1),
        cells_z: Some(1),
        ..Default::default()
    }
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

        let tp =
            resolve_checked(&tpms_design(&options), &ResolveParams::default()).expect("resolve");
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

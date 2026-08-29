//! 3D Dexel Grid Stock Subtraction & Volumetric CAM Simulation Test Suite (Track E2).

use dry_core::dexel::DexelGrid;
use dry_core::resolve::{Design, Op};
use dry_core::{resolve, ResolveParams};

#[test]
fn test_dexel_grid_initialization_and_volume() {
    let stock = DexelGrid::new_stock(0.0, 0.0, 0.0, 100.0, 100.0, 50.0, 1.0)
        .expect("valid stock grid");

    assert_eq!(stock.nx, 100);
    assert_eq!(stock.ny, 100);
    assert_eq!(stock.heights.len(), 10000);

    let initial_vol = stock.initial_volume();
    assert!((initial_vol - 500_000.0).abs() < 1e-5);

    let remaining_vol = stock.remaining_volume();
    assert!((remaining_vol - 500_000.0).abs() < 1e-3);
}

#[test]
fn test_dexel_single_pass_slot_carving() {
    let mut stock = DexelGrid::new_stock(0.0, 0.0, 0.0, 100.0, 50.0, 20.0, 0.5)
        .expect("valid stock grid");

    // Flat endmill radius 5mm cutting along X=20 to X=80 at Y=25, Z=15 (depth 5mm)
    stock.carve_segment([20.0, 25.0, 15.0], [80.0, 25.0, 15.0], 5.0, false);

    let report = stock.generate_report();
    assert!(report.removed_volume_mm3 > 0.0);
    assert!(report.remaining_volume_mm3 < report.initial_volume_mm3);
    assert!((report.min_height_mm - 15.0).abs() < 1e-5);
    assert!((report.max_height_mm - 20.0).abs() < 1e-5);
}

#[test]
fn test_dexel_full_toolpath_simulation() {
    let mut stock = DexelGrid::new_stock(0.0, 0.0, 0.0, 60.0, 60.0, 20.0, 1.0)
        .expect("valid stock grid");

    let design = Design {
        ops: vec![
            Op::Speed { print: 1200.0 },
            Op::Extruder { on: true },
            Op::Move {
                x: Some(10.0),
                y: Some(10.0),
                z: Some(10.0),
            },
            Op::Move {
                x: Some(50.0),
                y: Some(10.0),
                z: Some(10.0),
            },
            Op::Move {
                x: Some(50.0),
                y: Some(50.0),
                z: Some(10.0),
            },
            Op::Move {
                x: Some(10.0),
                y: Some(50.0),
                z: Some(10.0),
            },
            Op::Move {
                x: Some(10.0),
                y: Some(10.0),
                z: Some(10.0),
            },
        ],
    };

    let tp = resolve(&design, &ResolveParams::default());
    stock.simulate_toolpath(&tp, 4.0, false);

    let report = stock.generate_report();
    assert!(report.removed_volume_mm3 > 1000.0);
    assert!(report.material_removal_ratio > 0.01);
}

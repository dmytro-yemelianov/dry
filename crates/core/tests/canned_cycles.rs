use dry_core::{emit_cycle_cancel, DrillCycle, PeckDrillCycle};

#[test]
fn test_drill_cycle_g81_emission() {
    let drill = DrillCycle {
        x: 25.0,
        y: 50.0,
        z_depth: -12.5,
        r_plane: 2.0,
        feedrate_mm_min: 300.0,
    };

    let block = drill.emit_rs274();
    assert_eq!(block, "G81 X25.000 Y50.000 Z-12.500 R2.000 F300.0");
}

#[test]
fn test_peck_drill_cycle_g83_emission() {
    let peck = PeckDrillCycle {
        x: 10.0,
        y: 15.0,
        z_depth: -25.0,
        r_plane: 3.0,
        peck_depth_q: 5.0,
        feedrate_mm_min: 250.0,
    };

    let block = peck.emit_rs274();
    assert_eq!(block, "G83 X10.000 Y15.000 Z-25.000 R3.000 Q5.000 F250.0");

    assert_eq!(emit_cycle_cancel(), "G80");
}

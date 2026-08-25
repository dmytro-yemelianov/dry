//! The G-code importer's verify-dependent assertions.
//!
//! `gcode::lift` reconstructs an L2 toolpath from a slicer's own program, and two of its inline
//! tests judged the result with `verify` — one that hot imported G-code is *not* flagged for cold
//! extrusion, one that an E-only prime move *is* flagged over a flow ceiling. `verify` is layer 2
//! and is not reachable from `kmet-kernel`, where the importer lives, so they run here, over the
//! same fixtures and with the same assertions (plan Task 4).

use dry_core::{import_gcode, verify, Contracts, GcodeImportParams};

#[test]
fn imported_nozzle_temperature_satisfies_cold_extrusion_guard() {
    let tp = import_gcode(
        "M104 S210\nM109 S210\nM83\nG1 X0 Y0 Z0.2 F9000\nG1 X10 E1 F1200\n",
        &GcodeImportParams {
            line_width: Some(0.45),
            layer_height: Some(0.2),
            ..GcodeImportParams::default()
        },
    )
    .unwrap();
    let report = verify(
        &tp,
        &Contracts {
            min_temp: Some(180.0),
            ..Contracts::default()
        },
    );
    assert!(
        !report.findings.iter().any(|f| f.rule == "cold-extrusion"),
        "hot imported G-code should not be flagged: {:?}",
        report.findings
    );
}

#[test]
fn e_only_prime_moves_are_flagged_over_the_flow_limit() {
    let tp = import_gcode("M83\nG1 E5 F300\n", &Default::default()).unwrap();
    let report = verify(
        &tp,
        &Contracts {
            max_flow: Some(1.0),
            ..Contracts::default()
        },
    );
    assert!(report.findings.iter().any(|f| f.rule == "max-flow"));
}

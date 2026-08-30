//! A thermal-runaway detector must not report a printer healthy when it cannot read its temperature.
//!
//! `(NaN - target).abs()` is `NaN`, and `NaN > 15.0` is false, so a printing machine whose
//! thermistor read `NaN` came back with an empty anomaly list. A disconnected or failed thermistor
//! is precisely the fault this detector exists to catch, which makes failing open there the worst
//! available behaviour. An *infinite* reading was already caught (`inf > 15.0` holds) — only `NaN`
//! slipped through, which is what let the gap survive the existing tests.

use dry_moonraker::{FleetManager, PrinterLiveStatus};

fn printing(nozzle: f64, bed: f64) -> PrinterLiveStatus {
    PrinterLiveStatus {
        state: "printing".into(),
        nozzle_temp_c: nozzle,
        bed_temp_c: bed,
        progress: 0.5,
    }
}

#[test]
fn an_unreadable_sensor_is_itself_a_critical_anomaly() {
    let fleet = FleetManager::new();

    for (label, nozzle, bed) in [
        ("nozzle NaN", f64::NAN, 60.0),
        ("nozzle inf", f64::INFINITY, 60.0),
        ("bed NaN", 210.0, f64::NAN),
        ("both NaN", f64::NAN, f64::NAN),
    ] {
        let found = fleet.detect_anomalies(&printing(nozzle, bed), 210.0, 60.0);
        assert!(
            found.iter().any(|a| a.code == "SENSOR_READING_INVALID"),
            "{label}: an unreadable sensor must be reported, got {:?}",
            found.iter().map(|a| a.code.clone()).collect::<Vec<_>>()
        );
        assert!(
            found
                .iter()
                .filter(|a| a.code == "SENSOR_READING_INVALID")
                .all(|a| a.severity == "critical"),
            "{label}: an unreadable sensor is critical, not advisory"
        );
    }
}

#[test]
fn a_real_runaway_is_still_detected_and_a_healthy_printer_is_still_quiet() {
    let fleet = FleetManager::new();

    // 250 C against a 210 C target is a 40 C deviation: still reported, by its own code.
    let hot = fleet.detect_anomalies(&printing(250.0, 60.0), 210.0, 60.0);
    assert!(hot.iter().any(|a| a.code == "NOZZLE_THERMAL_DEVIATION"));
    assert!(!hot.iter().any(|a| a.code == "SENSOR_READING_INVALID"));

    // A printer on target reports nothing, so the new check has not made every run noisy.
    assert!(fleet
        .detect_anomalies(&printing(210.0, 60.0), 210.0, 60.0)
        .is_empty());

    // An idle printer with an unreadable sensor is not printing, so it is not a thermal emergency.
    let idle = PrinterLiveStatus {
        state: "standby".into(),
        nozzle_temp_c: f64::NAN,
        bed_temp_c: f64::NAN,
        progress: 0.0,
    };
    assert!(fleet.detect_anomalies(&idle, 210.0, 60.0).is_empty());
}

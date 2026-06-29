//! Golden firmware/printer profile matrix (`docs/16-support-matrix.md`, 08·WS3).
//!
//! For each supported profile under `conformance/profile-matrix/<entry>/profile.json`, this imports the
//! representative `examples/part.gcode` *under that profile* (its import defaults + contracts), runs the
//! review pipeline, and drift-gates the resulting `review.json`. Loading each profile via
//! `Profile::from_json` also asserts it is schema-valid by construction. Run with
//! `UPDATE_PROFILE_MATRIX=1` to (re)write the goldens + `MANIFEST.json`.
//!
//! The independent Python validator (`tools/validate_reports.py`) re-checks every `profile.json` against
//! the profile schema and every `review.json` against `ReviewReport`.

use dry_core::{import_gcode_with_map, simulate, verify, Profile, ReviewReport};
use std::fs;
use std::path::PathBuf;

fn matrix_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/profile-matrix")
}

fn part_gcode() -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/part.gcode"))
        .expect("examples/part.gcode exists")
}

fn update_mode() -> bool {
    std::env::var_os("UPDATE_PROFILE_MATRIX").is_some()
}

fn write_or_check(path: PathBuf, bytes: &[u8], update: bool) {
    if update {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
    } else {
        let committed = fs::read(&path).unwrap_or_else(|_| {
            panic!("missing {path:?} — run `UPDATE_PROFILE_MATRIX=1 cargo test -p dry-core --test profile_matrix`")
        });
        assert_eq!(committed, bytes, "{path:?} drifted");
    }
}

/// The matrix entry names, sorted (each is a directory holding `profile.json`).
fn entries() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(matrix_dir())
        .expect("conformance/profile-matrix exists")
        .filter_map(|e| {
            let p = e.ok()?.path();
            if p.is_dir() {
                Some(p.file_name()?.to_str()?.to_string())
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names
}

#[test]
fn profile_matrix_reviews_match_or_update() {
    let update = update_mode();
    let dir = matrix_dir();
    let gcode = part_gcode();
    let mut manifest_entries = Vec::new();

    let names = entries();
    assert!(
        names.len() >= 6,
        "expected ≥ 6 matrix entries, found {}",
        names.len()
    );

    for name in &names {
        let profile_path = dir.join(name).join("profile.json");
        let profile = Profile::from_json(&fs::read_to_string(&profile_path).unwrap())
            .unwrap_or_else(|e| panic!("[{name}] invalid profile: {e}"));

        let imported = import_gcode_with_map(&gcode, &profile.gcode_import_params())
            .unwrap_or_else(|e| panic!("[{name}] import: {e}"));
        let metrics = simulate(&imported.toolpath);
        let report = verify(&imported.toolpath, &profile.contracts());
        let review = ReviewReport::build(
            Some("examples/part.gcode".to_string()),
            profile.name.clone(),
            imported.toolpath.segments.len(),
            metrics,
            &report,
            |segment| imported.source_line_for_segment(segment),
        );
        let review_json = serde_json::to_string_pretty(&review).unwrap() + "\n";
        write_or_check(
            dir.join(name).join("review.json"),
            review_json.as_bytes(),
            update,
        );

        manifest_entries.push(serde_json::json!({
            "entry": name,
            "name": profile.name,
            "firmware": profile.firmware.flavor,
            "build_volume": profile.machine.build_volume,
            "max_volumetric_flow_mm3_s": profile.material.max_volumetric_flow_mm3_s,
            "min_nozzle_temperature_c": profile.material.min_nozzle_temperature_c,
        }));
    }

    let manifest = serde_json::json!({
        "schema": "spec/dry-profile-v1.schema.json",
        "review_schema": "spec/dry-reports-v1.schema.json#/$defs/ReviewReport",
        "entries": manifest_entries,
    });
    let manifest_bytes = (serde_json::to_string_pretty(&manifest).unwrap() + "\n").into_bytes();
    write_or_check(dir.join("MANIFEST.json"), &manifest_bytes, update);

    eprintln!("profile matrix: {} entries checked", names.len());
}

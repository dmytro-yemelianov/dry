use dry_core::apt::lift::{import_apt, AptImportParams};
use dry_core::emit::{
    emit_apt_to_writer, emit_irbcam_to_writer, AptFrame, DwellPolicy, EmitParams, ExtrusionCarry,
    FirmwareFlavor, IrbcamFrame, IrbcamLayout, RapidEncoding,
};
use std::fs;
use std::path::{Path, PathBuf};

fn update_goldens() -> bool {
    std::env::var_os("UPDATE_GOLDEN").is_some()
}

fn check_or_update(path: &Path, content: &str) {
    if update_goldens() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    } else {
        let existing = fs::read_to_string(path).unwrap_or_else(|e| {
            panic!("Golden file {path:?} missing ({e}). Run with UPDATE_GOLDEN=1 to create/update.")
        });
        assert_eq!(
            content, existing,
            "Content diverged from golden at {path:?}. If this change is intentional and contract-verified, run with UPDATE_GOLDEN=1."
        );
    }
}

#[test]
fn test_fusion360_pocket_drill_arc_goldens() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let apt_path = manifest_dir
        .join("../../conformance/apt/fusion360-apt-cps/pocket_drill_arc.apt")
        .canonicalize()
        .expect("pocket_drill_arc.apt exists");

    let source = fs::read_to_string(&apt_path).expect("read apt");
    let toolpath = import_apt(&source, &AptImportParams::default()).expect("lift apt to IR");

    // 1. IR JSON golden
    let ir_json = toolpath.to_json() + "\n";
    let ir_json_path = apt_path.with_extension("ir.json");
    check_or_update(&ir_json_path, &ir_json);

    // 2. IRBCAM JSON golden
    let irbcam_json_params = EmitParams {
        flavor: FirmwareFlavor::Irbcam,
        irbcam_frame: IrbcamFrame {
            layout: IrbcamLayout::Json,
            rapid: RapidEncoding::MinusOne,
            dwell: DwellPolicy::Drop,
            extrusion: ExtrusionCarry::Refuse,
            decimals: 6,
            ..IrbcamFrame::default()
        },
        ..EmitParams::default()
    };
    let mut irbcam_json_buf = Vec::new();
    emit_irbcam_to_writer(
        toolpath.segments.iter().cloned().map(Ok),
        &irbcam_json_params,
        &mut irbcam_json_buf,
    )
    .expect("emit irbcam json");
    let irbcam_json = String::from_utf8(irbcam_json_buf).expect("utf-8 irbcam json");
    let irbcam_json_path = apt_path.with_extension("irbcam.json");
    check_or_update(&irbcam_json_path, &irbcam_json);

    // 3. IRBCAM CSV golden
    let irbcam_csv_params = EmitParams {
        flavor: FirmwareFlavor::IrbcamCsv,
        irbcam_frame: IrbcamFrame {
            layout: IrbcamLayout::Csv,
            rapid: RapidEncoding::MinusOne,
            dwell: DwellPolicy::Drop,
            extrusion: ExtrusionCarry::Refuse,
            decimals: 6,
            ..IrbcamFrame::default()
        },
        ..EmitParams::default()
    };
    let mut irbcam_csv_buf = Vec::new();
    emit_irbcam_to_writer(
        toolpath.segments.iter().cloned().map(Ok),
        &irbcam_csv_params,
        &mut irbcam_csv_buf,
    )
    .expect("emit irbcam csv");
    let irbcam_csv = String::from_utf8(irbcam_csv_buf).expect("utf-8 irbcam csv");
    let irbcam_csv_path = apt_path.with_extension("irbcam.csv");
    check_or_update(&irbcam_csv_path, &irbcam_csv);

    // 4. Roundtrip APT golden
    let apt_params = EmitParams {
        flavor: FirmwareFlavor::Apt,
        apt_frame: AptFrame {
            partno: Some("POCKET_DRILL_ARC".to_string()),
            machin: Some("DRY_5AX".to_string()),
            decimals: 6,
        },
        ..EmitParams::default()
    };
    let mut apt_buf = Vec::new();
    emit_apt_to_writer(
        toolpath.segments.iter().cloned().map(Ok),
        &apt_params,
        &mut apt_buf,
    )
    .expect("emit apt");
    let roundtrip_apt = String::from_utf8(apt_buf).expect("utf-8 apt");
    let roundtrip_apt_path = apt_path.with_extension("roundtrip.apt");
    check_or_update(&roundtrip_apt_path, &roundtrip_apt);
}

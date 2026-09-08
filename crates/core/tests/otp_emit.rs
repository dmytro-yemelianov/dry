use dry_core::emit::{
    emit_otp_to_writer, EmitParams, FirmwareFlavor, Kinematics, OtpFrame, OTP_MIMETYPE,
    OTP_SPEC_VERSION,
};
use dry_core::ir::{Segment, SegmentKind, Toolpath};
use dry_core::units::{Feedrate, Length, Volume};

fn seg_line(start: [f64; 3], end: [f64; 3], speed: f64, tool: u32) -> Segment {
    Segment {
        start: [
            Some(Length::mm(start[0])),
            Some(Length::mm(start[1])),
            Some(Length::mm(start[2])),
        ],
        end: [
            Some(Length::mm(end[0])),
            Some(Length::mm(end[1])),
            Some(Length::mm(end[2])),
        ],
        travel: false,
        speed: Feedrate(speed),
        length: Length::mm(20.0),
        volume: Volume(2.0),
        filament: Length::mm(1.0),
        width: Some(Length::mm(0.4)),
        height: Some(Length::mm(0.2)),
        kind: SegmentKind::Line,
        centre: None,
        clockwise: false,
        temperature: Some(215.0),
        fan: Some(0.8),
        flow: Some(1.0),
        tool: Some(tool),
        power: None,
        dwell_s: None,
        manual_gcode: None,
        orientation: Some([0.0, 0.0, 1.0]),
        control_points: None,
    }
}

fn make_test_segments() -> Vec<Segment> {
    vec![
        seg_line([0.0, 0.0, 0.0], [10.0, 20.0, 0.0], 1200.0, 1),
        seg_line([10.0, 20.0, 0.0], [30.0, 20.0, 5.0], 1500.0, 2),
    ]
}

#[test]
fn test_otp_basic_package_emission() {
    let segments = make_test_segments();
    let params = EmitParams {
        flavor: FirmwareFlavor::Otp,
        ..Default::default()
    };

    let mut zip_bytes = Vec::new();
    let stats = emit_otp_to_writer(
        segments.clone().into_iter().map(Ok),
        &params,
        &mut zip_bytes,
    )
    .expect("OTP emission succeeds");

    assert_eq!(stats.segments_count, 2);
    assert_eq!(stats.tools_count, 2);
    assert_eq!(stats.total_bytes_written, zip_bytes.len());

    // 1. Raw magic inspection
    assert!(zip_bytes.starts_with(b"PK\x03\x04"));
    // Mimetype at byte offset 30
    assert_eq!(&zip_bytes[30..38], b"mimetype");
    assert_eq!(
        &zip_bytes[38..38 + OTP_MIMETYPE.len()],
        OTP_MIMETYPE.as_bytes()
    );

    // 2. Read back entries
    let cursor = std::io::Cursor::new(&zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor).expect("valid zip");

    let entries: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();

    assert!(entries.contains(&"mimetype".to_string()));
    assert!(entries.contains(&"manifest.json".to_string()));
    assert!(entries.contains(&"payload/toolpath.json".to_string()));
    assert!(entries.contains(&"tools.json".to_string()));
    assert!(entries.contains(&"context.json".to_string()));

    // 3. Check manifest
    let manifest: serde_json::Value = {
        let mut manifest_file = archive.by_name("manifest.json").unwrap();
        let mut manifest_str = String::new();
        std::io::Read::read_to_string(&mut manifest_file, &mut manifest_str).unwrap();
        serde_json::from_str(&manifest_str).unwrap()
    };

    assert_eq!(manifest["otp_version"], OTP_SPEC_VERSION);
    assert_eq!(manifest["entrypoints"]["payload"], "payload/toolpath.json");
    assert_eq!(manifest["entrypoints"]["payload_format"], "json");

    // 4. Verify SHA-256 digests recorded in manifest
    use sha2::{Digest, Sha256};
    for file_name in ["payload/toolpath.json", "tools.json", "context.json"] {
        let mut f = archive.by_name(file_name).unwrap();
        let mut content = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut content).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(&content);
        let actual_digest = format!("sha256:{:x}", hasher.finalize());

        let expected_digest = manifest["digests"][file_name].as_str().unwrap();
        assert_eq!(
            actual_digest, expected_digest,
            "Digest mismatch for {file_name}"
        );
    }
}

#[test]
fn test_otp_five_axis_kinematic_context() {
    let segments = make_test_segments();
    let params = EmitParams {
        flavor: FirmwareFlavor::Otp,
        five_axis: true,
        kinematics: Kinematics::default(),
        otp_frame: OtpFrame {
            domain: Some("subtractive".to_string()),
            sub_type: Some("milling_5axis".to_string()),
            description: Some("Test blisk machining".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    let mut zip_bytes = Vec::new();
    let stats = emit_otp_to_writer(segments.into_iter().map(Ok), &params, &mut zip_bytes)
        .expect("OTP emission succeeds");

    assert_eq!(stats.segments_count, 2);

    let cursor = std::io::Cursor::new(&zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor).expect("valid zip");

    let mut ctx_file = archive.by_name("context.json").unwrap();
    let mut ctx_str = String::new();
    std::io::Read::read_to_string(&mut ctx_file, &mut ctx_str).unwrap();
    let ctx: serde_json::Value = serde_json::from_str(&ctx_str).unwrap();

    assert_eq!(ctx["machine"]["kinematics"]["topology"], "table_table_ac");
    let axes = ctx["machine"]["kinematics"]["axes"].as_array().unwrap();
    assert_eq!(axes.len(), 5);
}

#[test]
fn test_otp_dry0_columnar_binary_payload() {
    let segments = make_test_segments();
    let params = EmitParams {
        flavor: FirmwareFlavor::Otp,
        otp_frame: OtpFrame {
            payload_format: Some("dry0".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    let mut zip_bytes = Vec::new();
    emit_otp_to_writer(segments.into_iter().map(Ok), &params, &mut zip_bytes)
        .expect("OTP dry0 emission succeeds");

    let cursor = std::io::Cursor::new(&zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor).expect("valid zip");

    assert!(archive.by_name("payload/toolpath.dry0").is_ok());

    let mut payload_file = archive.by_name("payload/toolpath.dry0").unwrap();
    let mut binary = Vec::new();
    std::io::Read::read_to_end(&mut payload_file, &mut binary).unwrap();

    let decoded_tp = Toolpath::from_bytes(&binary).expect("decode DRY0 payload");
    assert_eq!(decoded_tp.segments.len(), 2);
}

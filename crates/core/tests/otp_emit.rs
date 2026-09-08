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

#[test]
fn test_otp_dry1_streaming_binary_payload() {
    let segments = make_test_segments();
    let params = EmitParams {
        flavor: FirmwareFlavor::Otp,
        otp_frame: OtpFrame {
            payload_format: Some("dry1".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    let mut zip_bytes = Vec::new();
    emit_otp_to_writer(segments.into_iter().map(Ok), &params, &mut zip_bytes)
        .expect("OTP dry1 emission succeeds");

    let cursor = std::io::Cursor::new(&zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor).expect("valid zip");

    assert!(archive.by_name("payload/toolpath.dry1").is_ok());

    let mut payload_file = archive.by_name("payload/toolpath.dry1").unwrap();
    let mut binary = Vec::new();
    std::io::Read::read_to_end(&mut payload_file, &mut binary).unwrap();

    let (_version, _meta, stream) =
        dry_core::codec::decode_any_streaming(std::io::Cursor::new(binary))
            .expect("decode DRY1 payload");
    let decoded_segments: Vec<_> = stream.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(decoded_segments.len(), 2);
}

#[test]
fn test_otp_emit_stream_to_writer_dispatch() {
    let segments = make_test_segments();
    let params = EmitParams {
        flavor: FirmwareFlavor::Otp,
        ..Default::default()
    };

    let mut zip_bytes = Vec::new();
    dry_core::emit_stream_to_writer(segments.into_iter().map(Ok), &params, &mut zip_bytes)
        .expect("emit_stream_to_writer with Otp flavor succeeds");

    assert!(zip_bytes.starts_with(b"PK\x03\x04"));
    let cursor = std::io::Cursor::new(&zip_bytes);
    let archive = zip::ZipArchive::new(cursor).expect("valid zip archive");
    assert!(archive.len() >= 4);
}

#[test]
fn test_otp_refuses_non_finite_coordinates_and_speed() {
    let bad_seg = Segment {
        start: [
            Some(Length::mm(0.0)),
            Some(Length::mm(0.0)),
            Some(Length::mm(0.0)),
        ],
        end: [
            Some(Length::mm(10.0)),
            Some(Length::mm(20.0)),
            Some(Length::mm(0.0)),
        ],
        travel: false,
        speed: Feedrate(f64::NAN),
        length: Length::mm(20.0),
        volume: Volume(2.0),
        filament: Length::mm(1.0),
        width: Some(Length::mm(0.4)),
        height: Some(Length::mm(0.2)),
        kind: SegmentKind::Line,
        centre: None,
        clockwise: false,
        temperature: None,
        fan: None,
        flow: None,
        tool: None,
        power: None,
        dwell_s: None,
        manual_gcode: None,
        orientation: None,
        control_points: None,
    };

    let params = EmitParams {
        flavor: FirmwareFlavor::Otp,
        ..Default::default()
    };
    let mut out = Vec::new();
    // 1. Non-finite feedrate
    let res = emit_otp_to_writer(vec![Ok(bad_seg.clone())], &params, &mut out);
    assert!(res.is_err(), "non-finite feedrate must be refused");

    // 2. Non-finite start coordinate
    let mut bad_start = bad_seg.clone();
    bad_start.speed = Feedrate(1000.0);
    bad_start.start[0] = Some(Length(f64::INFINITY));
    assert!(emit_otp_to_writer(vec![Ok(bad_start)], &params, &mut out).is_err());

    // 3. Non-finite end coordinate
    let mut bad_end = bad_seg.clone();
    bad_end.speed = Feedrate(1000.0);
    bad_end.end[1] = Some(Length(f64::NAN));
    assert!(emit_otp_to_writer(vec![Ok(bad_end)], &params, &mut out).is_err());

    // 4. Non-finite arc centre
    let mut bad_centre = bad_seg.clone();
    bad_centre.speed = Feedrate(1000.0);
    bad_centre.centre = Some([Length(f64::NAN), Length(0.0)]);
    assert!(emit_otp_to_writer(vec![Ok(bad_centre)], &params, &mut out).is_err());

    // 5. Invalid power (negative or non-finite)
    let mut bad_power = bad_seg.clone();
    bad_power.speed = Feedrate(1000.0);
    bad_power.power = Some(-10.0);
    assert!(emit_otp_to_writer(vec![Ok(bad_power)], &params, &mut out).is_err());

    let mut nan_power = bad_seg.clone();
    nan_power.speed = Feedrate(1000.0);
    nan_power.power = Some(f64::NAN);
    assert!(emit_otp_to_writer(vec![Ok(nan_power)], &params, &mut out).is_err());
}

#[test]
fn test_otp_arc_and_spline_and_dwell_segments() {
    let segs = vec![
        Segment {
            start: [
                Some(Length::mm(0.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.0)),
            ],
            end: [
                Some(Length::mm(10.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.0)),
            ],
            travel: false,
            speed: Feedrate(1000.0),
            length: Length::mm(15.7),
            volume: Volume(1.0),
            filament: Length::mm(0.5),
            width: Some(Length::mm(0.4)),
            height: Some(Length::mm(0.2)),
            kind: SegmentKind::Arc,
            centre: Some([Length::mm(5.0), Length::mm(0.0)]),
            clockwise: true,
            temperature: None,
            fan: None,
            flow: None,
            tool: Some(1),
            power: None,
            dwell_s: None,
            manual_gcode: None,
            orientation: None,
            control_points: None,
        },
        Segment {
            start: [
                Some(Length::mm(10.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.0)),
            ],
            end: [
                Some(Length::mm(20.0)),
                Some(Length::mm(10.0)),
                Some(Length::mm(0.0)),
            ],
            travel: false,
            speed: Feedrate(1000.0),
            length: Length::mm(14.1),
            volume: Volume(1.0),
            filament: Length::mm(0.5),
            width: Some(Length::mm(0.4)),
            height: Some(Length::mm(0.2)),
            kind: SegmentKind::Spline,
            centre: None,
            clockwise: false,
            temperature: None,
            fan: None,
            flow: None,
            tool: Some(1),
            power: None,
            dwell_s: None,
            manual_gcode: None,
            orientation: None,
            control_points: Some(vec![
                [Length::mm(10.0), Length::mm(0.0), Length::mm(0.0)],
                [Length::mm(15.0), Length::mm(5.0), Length::mm(0.0)],
                [Length::mm(20.0), Length::mm(10.0), Length::mm(0.0)],
            ]),
        },
        Segment {
            start: [
                Some(Length::mm(20.0)),
                Some(Length::mm(10.0)),
                Some(Length::mm(0.0)),
            ],
            end: [
                Some(Length::mm(20.0)),
                Some(Length::mm(10.0)),
                Some(Length::mm(0.0)),
            ],
            travel: false,
            speed: Feedrate(0.0),
            length: Length::ZERO,
            volume: Volume::ZERO,
            filament: Length::ZERO,
            width: None,
            height: None,
            kind: SegmentKind::Dwell,
            centre: None,
            clockwise: false,
            temperature: None,
            fan: None,
            flow: None,
            tool: None,
            power: None,
            dwell_s: Some(1.5),
            manual_gcode: None,
            orientation: None,
            control_points: None,
        },
    ];

    let params = EmitParams {
        flavor: FirmwareFlavor::Otp,
        ..Default::default()
    };
    let mut out = Vec::new();
    let stats = emit_otp_to_writer(segs.into_iter().map(Ok), &params, &mut out).unwrap();
    assert_eq!(stats.segments_count, 3);
}

#[test]
fn test_otp_custom_tools_and_explicit_context() {
    use dry_core::emit::{
        OtpAxis, OtpContextDescriptor, OtpKinematics, OtpMachine, OtpStock, OtpToolCapabilities,
        OtpToolDefinition, OtpToolGeometry, OtpToolOffsets, OtpToolThermal, OtpWorkCoordinates,
    };

    let segs = make_test_segments();
    let mut offsets_map = std::collections::BTreeMap::new();
    let mut g54_axes = std::collections::BTreeMap::new();
    g54_axes.insert("x".to_string(), 10.0);
    g54_axes.insert("y".to_string(), 20.0);
    offsets_map.insert("G54".to_string(), g54_axes);

    let params = EmitParams {
        flavor: FirmwareFlavor::Otp,
        otp_frame: OtpFrame {
            package_id: Some("urn:uuid:12345678-1234-4234-8234-123456789abc".to_string()),
            created_at: Some("2026-09-08T12:00:00Z".to_string()),
            domain: Some("subtractive".to_string()),
            sub_type: Some("milling_3axis".to_string()),
            description: Some("Custom tooling test".to_string()),
            conformance_level: Some("strict".to_string()),
            tools: Some(vec![OtpToolDefinition {
                id: 1,
                name: "Endmill 10mm".to_string(),
                kind: "endmill".to_string(),
                geometry: Some(OtpToolGeometry {
                    diameter: Some(10.0),
                    corner_radius: Some(1.0),
                    flute_length: Some(35.0),
                    overall_length: Some(80.0),
                    flute_count: Some(4),
                    orifice_diameter: None,
                    filament_diameter: None,
                }),
                offsets: Some(OtpToolOffsets {
                    length_offset: Some(50.0),
                    radius_offset: Some(5.0),
                }),
                capabilities: Some(OtpToolCapabilities {
                    max_rpm: Some(18000.0),
                    max_feedrate: Some(5000.0),
                }),
                thermal: Some(OtpToolThermal {
                    max_temperature: Some(80.0),
                }),
            }]),
            context: Some(OtpContextDescriptor {
                schema: None,
                schema_version: "1.0".to_string(),
                machine: OtpMachine {
                    vendor: Some("Hermle".to_string()),
                    model: Some("C42".to_string()),
                    kinematics: OtpKinematics {
                        topology: "table_table_ac".to_string(),
                        axes: vec![OtpAxis {
                            name: "X".to_string(),
                            axis_type: "linear".to_string(),
                            min: Some(-400.0),
                            max: Some(400.0),
                            max_velocity: Some(30000.0),
                            axis_vector: Some([1.0, 0.0, 0.0]),
                        }],
                    },
                },
                work_coordinates: Some(OtpWorkCoordinates {
                    active_system: "G54".to_string(),
                    offsets: offsets_map,
                }),
                stock: Some(OtpStock {
                    stock_type: "box".to_string(),
                    dimensions: vec![100.0, 100.0, 50.0],
                    origin: Some([0.0, 0.0, 0.0]),
                }),
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let mut out = Vec::new();
    let stats = emit_otp_to_writer(segs.into_iter().map(Ok), &params, &mut out).unwrap();
    assert_eq!(stats.tools_count, 1);
}

#[test]
fn test_otp_robot_and_cnc_flavors_inferred_tools() {
    // Robot flavor
    let segs = make_test_segments();
    let params_robot = EmitParams {
        flavor: FirmwareFlavor::RobotKrl,
        ..Default::default()
    };
    let mut out_robot = Vec::new();
    let stats_robot = emit_otp_to_writer(
        segs.clone().into_iter().map(Ok),
        &params_robot,
        &mut out_robot,
    )
    .unwrap();
    assert_eq!(stats_robot.tools_count, 2);

    // CNC flavor
    let params_cnc = EmitParams {
        flavor: FirmwareFlavor::Rs274,
        ..Default::default()
    };
    let mut out_cnc = Vec::new();
    let stats_cnc =
        emit_otp_to_writer(segs.into_iter().map(Ok), &params_cnc, &mut out_cnc).unwrap();
    assert_eq!(stats_cnc.tools_count, 2);
}

#[test]
fn test_otp_unsupported_payload_format() {
    let segs = make_test_segments();
    let params = EmitParams {
        flavor: FirmwareFlavor::Otp,
        otp_frame: OtpFrame {
            payload_format: Some("unsupported_xyz".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut out = Vec::new();
    let err = emit_otp_to_writer(segs.into_iter().map(Ok), &params, &mut out).unwrap_err();
    assert!(err
        .to_string()
        .contains("unsupported OpenToolpath payload format"));
}

#[test]
fn test_otp_zip_archive_writer_and_crc() {
    use dry_core::emit::{crc32, ZipArchiveWriter};
    let data = b"Hello, OpenToolpath CRC and Zip Writer test!";
    let calculated_crc = crc32(data);
    assert_ne!(calculated_crc, 0);

    let mut buf = Vec::new();
    let mut zip = ZipArchiveWriter::new(&mut buf);
    // Uncompressed entry
    zip.add_file("stored.txt", data, false).unwrap();
    // Deflated entry
    zip.add_file("deflated.txt", data, true).unwrap();
    let written = zip.finish().unwrap();
    assert!(written > 0);
}

#[test]
fn test_otp_bc_kinematics_and_robot_context() {
    let segs = make_test_segments();
    // 1. Bc kinematics (table_table_bc)
    let params_bc = EmitParams {
        flavor: FirmwareFlavor::Otp,
        five_axis: true,
        kinematics: dry_core::emit::REFERENCE_FIVE_AXIS_MACHINE,
        ..Default::default()
    };
    let mut zip_bytes_bc = Vec::new();
    emit_otp_to_writer(
        segs.clone().into_iter().map(Ok),
        &params_bc,
        &mut zip_bytes_bc,
    )
    .unwrap();

    let cursor = std::io::Cursor::new(&zip_bytes_bc);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut ctx_file = archive.by_name("context.json").unwrap();
    let mut ctx_str = String::new();
    std::io::Read::read_to_string(&mut ctx_file, &mut ctx_str).unwrap();
    let ctx: serde_json::Value = serde_json::from_str(&ctx_str).unwrap();
    assert_eq!(ctx["machine"]["kinematics"]["topology"], "table_table_bc");

    // 2. Robot kinematics context (robot_6dof)
    let params_robot = EmitParams {
        flavor: FirmwareFlavor::RobotKrl,
        ..Default::default()
    };
    let mut zip_bytes_robot = Vec::new();
    emit_otp_to_writer(
        segs.into_iter().map(Ok),
        &params_robot,
        &mut zip_bytes_robot,
    )
    .unwrap();

    let cursor = std::io::Cursor::new(&zip_bytes_robot);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut ctx_file = archive.by_name("context.json").unwrap();
    let mut ctx_str = String::new();
    std::io::Read::read_to_string(&mut ctx_file, &mut ctx_str).unwrap();
    let ctx: serde_json::Value = serde_json::from_str(&ctx_str).unwrap();
    assert_eq!(ctx["machine"]["kinematics"]["topology"], "robot_6dof");
    let axes = ctx["machine"]["kinematics"]["axes"].as_array().unwrap();
    assert_eq!(axes.len(), 6);
}

#[test]
fn test_otp_default_tools_for_cnc_and_additive() {
    let mut seg = seg_line([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], 1000.0, 0);
    seg.tool = None;

    // 1. CNC default tool (endmill with flute/overall length)
    let params_cnc = EmitParams {
        flavor: FirmwareFlavor::Rs274,
        ..Default::default()
    };
    let mut out_cnc = Vec::new();
    let stats_cnc =
        emit_otp_to_writer(vec![Ok(seg.clone())], &params_cnc, &mut out_cnc).unwrap();
    assert_eq!(stats_cnc.tools_count, 1);

    // 2. Additive default tool (fff_nozzle with thermal/filament)
    let params_add = EmitParams {
        flavor: FirmwareFlavor::Otp,
        ..Default::default()
    };
    let mut out_add = Vec::new();
    let stats_add =
        emit_otp_to_writer(vec![Ok(seg.clone())], &params_add, &mut out_add).unwrap();
    assert_eq!(stats_add.tools_count, 1);

    // 3. Robot default tool (spindle)
    let params_rob = EmitParams {
        flavor: FirmwareFlavor::RobotKrl,
        ..Default::default()
    };
    let mut out_rob = Vec::new();
    let stats_rob = emit_otp_to_writer(vec![Ok(seg)], &params_rob, &mut out_rob).unwrap();
    assert_eq!(stats_rob.tools_count, 1);
}

#[test]
fn test_otp_five_axis_ab_kinematics() {
    let seg = seg_line([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], 1000.0, 1);
    let params = EmitParams {
        flavor: FirmwareFlavor::Otp,
        five_axis: true,
        kinematics: Kinematics::Ab {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [0.0, 0.0],
        },
        ..Default::default()
    };
    let mut out = Vec::new();
    emit_otp_to_writer(vec![Ok(seg)], &params, &mut out).unwrap();
    let cursor = std::io::Cursor::new(out);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut ctx_file = archive.by_name("context.json").unwrap();
    let mut ctx_str = String::new();
    std::io::Read::read_to_string(&mut ctx_file, &mut ctx_str).unwrap();
    assert!(ctx_str.contains("table_table_ac"));
}

#[test]
fn test_zip_archive_writer_incompressible_fallback() {
    use dry_core::emit::ZipArchiveWriter;
    let mut buf = Vec::new();
    let mut zip = ZipArchiveWriter::new(&mut buf);
    // tiny slice where compressed size >= uncompressed size, triggering uncompressed fallback
    let tiny = b"abc";
    zip.add_file("tiny.txt", tiny, true).unwrap();
    let total = zip.finish().unwrap();
    assert!(total > 0);
}

#[test]
fn test_otp_stock_and_offsets_model() {
    use dry_core::emit::OtpStock;
    let stock = OtpStock {
        stock_type: "box".to_string(),
        dimensions: vec![100.0, 100.0, 50.0],
        origin: Some([0.0, 0.0, -50.0]),
    };
    let s = serde_json::to_string(&stock).unwrap();
    let stock2: OtpStock = serde_json::from_str(&s).unwrap();
    assert_eq!(stock, stock2);
}

#[test]
fn test_otp_refuses_additional_non_finite_branches() {
    let seg = seg_line([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], 1000.0, 1);
    let params = EmitParams {
        flavor: FirmwareFlavor::Otp,
        ..Default::default()
    };
    let mut out = Vec::new();

    // 1. Non-finite start[1]
    let mut s1 = seg.clone();
    s1.start[1] = Some(Length(f64::NAN));
    assert!(emit_otp_to_writer(vec![Ok(s1)], &params, &mut out).is_err());

    // 2. Non-finite start[2]
    let mut s2 = seg.clone();
    s2.start[2] = Some(Length(f64::INFINITY));
    assert!(emit_otp_to_writer(vec![Ok(s2)], &params, &mut out).is_err());

    // 3. Non-finite end[0]
    let mut s3 = seg.clone();
    s3.end[0] = Some(Length(f64::NAN));
    assert!(emit_otp_to_writer(vec![Ok(s3)], &params, &mut out).is_err());

    // 4. Non-finite end[2]
    let mut s4 = seg.clone();
    s4.end[2] = Some(Length(f64::INFINITY));
    assert!(emit_otp_to_writer(vec![Ok(s4)], &params, &mut out).is_err());

    // 5. Non-finite arc centre[1]
    let mut s5 = seg;
    s5.centre = Some([Length::mm(5.0), Length(f64::NAN)]);
    assert!(emit_otp_to_writer(vec![Ok(s5)], &params, &mut out).is_err());
}


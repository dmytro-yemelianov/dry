//! Generator + drift gate for the public Dry IR v0 conformance vectors (`docs/10-dry-ir-v0-spec.md`).
//!
//! The authoritative seeds are defined here in typed Rust (clean-room — authored for the spec, not
//! derived from the FullControl oracle). Run with `UPDATE_VECTORS=1` to (re)write the committed
//! artifacts under `conformance/vectors/`; the normal `cargo test` run regenerates them from the seeds
//! and asserts they are byte-identical to the committed files (the drift gate), that every committed
//! `input.json` decodes back to its seed, and that every `frozen` vector still decodes.
//!
//! The independent Python validator (`tools/validate_vectors.py`) re-checks the same committed bytes
//! without `dry-core`.

// These exercise the deprecated infallible `emit()` on purpose: it is still the entry point the
// in-tree call sites use, and refusing the whole program is part of what is under test here.
#![allow(deprecated)]

use dry_core::{
    emit, simulate, EmitParams, Feedrate, FirmwareFlavor, Length, Meta, Segment, SegmentKind,
    Toolpath, Volume,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

fn vectors_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/vectors")
}

fn update_mode() -> bool {
    std::env::var_os("UPDATE_VECTORS").is_some()
}

struct Spec {
    name: &'static str,
    description: &'static str,
    feature_tags: &'static [&'static str],
    frozen: bool,
    emit: Option<EmitParams>,
    ir: Toolpath,
}

/// A baseline extruding line segment; override fields per vector.
fn base() -> Segment {
    Segment {
        start: [
            Some(Length::mm(0.0)),
            Some(Length::mm(0.0)),
            Some(Length::mm(0.2)),
        ],
        end: [
            Some(Length::mm(10.0)),
            Some(Length::mm(0.0)),
            Some(Length::mm(0.2)),
        ],
        travel: false,
        speed: Feedrate(1500.0),
        length: Length::mm(10.0),
        volume: Volume(0.8),
        filament: Length::mm(0.3326),
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
    }
}

fn tp(segments: Vec<Segment>) -> Toolpath {
    Toolpath {
        version: 0,
        meta: None,
        segments,
    }
}

fn specs() -> Vec<Spec> {
    let mut out = vec![Spec {
        name: "minimal_line",
        description: "One extruding line, all channels unset — the byte-identity baseline.",
        feature_tags: &["line", "no-channels"],
        frozen: true,
        emit: Some(EmitParams::default()),
        ir: tp(vec![base()]),
    }];

    out.push(Spec {
        name: "travel_and_line",
        description: "A travel move with undefined (null) start axes, then an extruding line.",
        feature_tags: &["travel", "null-axes"],
        frozen: true,
        emit: Some(EmitParams::default()),
        ir: tp(vec![
            Segment {
                start: [None, None, None],
                end: [
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                travel: true,
                speed: Feedrate(3000.0),
                length: Length::mm(0.0),
                volume: Volume(0.0),
                filament: Length::mm(0.0),
                ..base()
            },
            base(),
        ]),
    });

    out.push(Spec {
        name: "arc_g2_g3",
        description: "Two arcs sharing a centre: one clockwise (G2), one counter-clockwise (G3).",
        feature_tags: &["arc", "centre", "clockwise"],
        frozen: false,
        emit: Some(EmitParams::default()),
        ir: tp(vec![
            // A clockwise quarter turn ending where the G3 begins. Starting this arc at (10,0) —
            // directly below the centre — meant a *clockwise* traverse to (20,10) swept 270 degrees,
            // 47.12 mm, while the segment declared the 15.71 mm of the quarter arc. The emitted
            // `G2 I0 J10` cut the long way round and every metric billed the short one.
            Segment {
                start: [
                    Some(Length::mm(10.0)),
                    Some(Length::mm(20.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(20.0)),
                    Some(Length::mm(10.0)),
                    Some(Length::mm(0.2)),
                ],
                kind: SegmentKind::Arc,
                centre: Some([Length::mm(10.0), Length::mm(10.0)]),
                clockwise: true,
                length: Length::mm(15.70796),
                volume: Volume(1.256),
                ..base()
            },
            Segment {
                start: [
                    Some(Length::mm(20.0)),
                    Some(Length::mm(10.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(10.0)),
                    Some(Length::mm(20.0)),
                    Some(Length::mm(0.2)),
                ],
                kind: SegmentKind::Arc,
                centre: Some([Length::mm(10.0), Length::mm(10.0)]),
                clockwise: false,
                length: Length::mm(15.70796),
                volume: Volume(1.256),
                ..base()
            },
        ]),
    });

    out.push(Spec {
        name: "spline",
        description: "A spline segment carrying control points.",
        feature_tags: &["spline", "control-points"],
        frozen: false,
        emit: Some(EmitParams::default()),
        ir: tp(vec![Segment {
            kind: SegmentKind::Spline,
            control_points: Some(vec![
                [Length::mm(0.0), Length::mm(0.0), Length::mm(0.2)],
                [Length::mm(3.0), Length::mm(4.0), Length::mm(0.2)],
                [Length::mm(7.0), Length::mm(4.0), Length::mm(0.2)],
                [Length::mm(10.0), Length::mm(0.0), Length::mm(0.2)],
            ]),
            length: Length::mm(12.5),
            volume: Volume(1.0),
            ..base()
        }]),
    });

    out.push(Spec {
        name: "dwell",
        description: "A dwell (pause-in-place) segment with a duration.",
        feature_tags: &["dwell"],
        frozen: false,
        emit: Some(EmitParams::default()),
        ir: tp(vec![Segment {
            start: [
                Some(Length::mm(10.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            end: [
                Some(Length::mm(10.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            travel: false,
            speed: Feedrate(0.0),
            length: Length::mm(0.0),
            volume: Volume(0.0),
            filament: Length::mm(0.0),
            kind: SegmentKind::Dwell,
            dwell_s: Some(0.5),
            ..base()
        }]),
    });

    out.push(Spec {
        name: "retract_unretract",
        description: "An extruder-only retract followed by an unretract.",
        feature_tags: &["retract", "unretract"],
        frozen: false,
        emit: Some(EmitParams::default()),
        ir: tp(vec![
            // Extruder-only: the machine does not move, so start == end. Inheriting base()'s
            // 0 -> 10 endpoints while declaring length 0 made the emitter write
            // `G1 F1800 X10 Y0 Z0.2 E-2` — a 10 mm move at retraction feedrate — while metrics.json
            // recorded zero distance travelled.
            Segment {
                travel: false,
                speed: Feedrate(1800.0),
                length: Length::mm(0.0),
                volume: Volume(0.0),
                filament: Length::mm(-2.0),
                kind: SegmentKind::Retract,
                end: [
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                ..base()
            },
            Segment {
                travel: false,
                speed: Feedrate(900.0),
                length: Length::mm(0.0),
                volume: Volume(0.0),
                filament: Length::mm(2.0),
                kind: SegmentKind::Unretract,
                end: [
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                ..base()
            },
        ]),
    });

    out.push(Spec {
        name: "deposit",
        description: "A stationary deposit segment.",
        feature_tags: &["deposit"],
        frozen: false,
        emit: Some(EmitParams::default()),
        // "Stationary" has to mean it: inheriting base()'s 0 -> 10 endpoints made the emitter write
        // `G1 F600 X10 Y0 Z0.2 E0.02`, a 10 mm move, while metrics.json claimed zero distance and a
        // total time of 0.002 s for a move that takes a full second at F600.
        ir: tp(vec![Segment {
            travel: false,
            speed: Feedrate(600.0),
            length: Length::mm(0.0),
            volume: Volume(0.05),
            filament: Length::mm(0.02),
            kind: SegmentKind::Deposit,
            end: [
                Some(Length::mm(0.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            ..base()
        }]),
    });

    out.push(Spec {
        name: "manual_gcode",
        description: "A verbatim manual-gcode segment — exercises the three-way kind asymmetry \
                      (JSON 'manualgcode', DRY0 'manual_gcode', DRY1 tag 7; spec section 10).",
        feature_tags: &["manual-gcode", "kind-asymmetry"],
        frozen: true,
        emit: Some(EmitParams::default()),
        ir: tp(vec![Segment {
            start: [None, None, None],
            end: [None, None, None],
            travel: false,
            speed: Feedrate(0.0),
            length: Length::mm(0.0),
            volume: Volume(0.0),
            filament: Length::mm(0.0),
            width: None,
            height: None,
            kind: SegmentKind::ManualGcode,
            manual_gcode: Some("M117 hello".to_string()),
            ..base()
        }]),
    });

    out.push(Spec {
        name: "channels_full",
        description: "An extruding line with the four older process channels set: temperature, fan, \
                      flow, tool. It deliberately leaves `power` unset — this is the witness that a \
                      power-free toolpath still writes at DRY0 `enc_ver 1` (spec §5.3); the `power` \
                      channel has its own vector.",
        feature_tags: &["temperature", "fan", "flow", "tool", "enc-ver-1"],
        frozen: false,
        emit: Some(EmitParams::default()),
        ir: tp(vec![Segment {
            temperature: Some(215.0),
            fan: Some(0.6),
            flow: Some(1.05),
            tool: Some(1),
            ..base()
        }]),
    });

    out.push(Spec {
        name: "power_channel",
        description: "GRBL cutting moves carrying the spindle/laser power channel beside every \
                      field it is adjacent to on the wire — exercises the DRY0 enc_ver 2 column \
                      (which follows `manual_gcode`), the DRY1 row position (which follows \
                      `control_points`), flag bit 19, and the S / M3 / M5 words.",
        feature_tags: &[
            "power",
            "grbl",
            "enc-ver-2",
            "channels",
            "orientation",
            "control-points",
            "manual-gcode",
        ],
        frozen: true,
        emit: Some(EmitParams {
            flavor: FirmwareFlavor::Grbl,
            ..EmitParams::default()
        }),
        ir: tp(vec![
            // Power beside the four older channels and an orientation: in a `DRY0` body the power
            // column is last of the channels, and in a `DRY1` row the orientation bit precedes it.
            Segment {
                temperature: Some(215.0),
                fan: Some(0.6),
                flow: Some(1.05),
                tool: Some(1),
                power: Some(600.0),
                orientation: Some([0.0, 0.3826834, 0.9238795]),
                ..base()
            },
            // A spline: `control_points` is the field immediately before `power` in a `DRY1` row.
            Segment {
                start: [
                    Some(Length::mm(10.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(20.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                kind: SegmentKind::Spline,
                control_points: Some(vec![
                    [Length::mm(13.0), Length::mm(4.0), Length::mm(0.2)],
                    [Length::mm(17.0), Length::mm(4.0), Length::mm(0.2)],
                    [Length::mm(20.0), Length::mm(0.0), Length::mm(0.2)],
                ]),
                length: Length::mm(12.5),
                volume: Volume(1.0),
                power: Some(300.0),
                ..base()
            },
            // A verbatim block commanded dark: `manual_gcode` is the column immediately before
            // `power` in a `DRY0` body, and the `M5` must precede the block it guards.
            Segment {
                start: [
                    Some(Length::mm(20.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(20.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                speed: Feedrate(0.0),
                length: Length::mm(0.0),
                volume: Volume(0.0),
                filament: Length::mm(0.0),
                kind: SegmentKind::ManualGcode,
                manual_gcode: Some("M117 cut complete".to_string()),
                power: Some(0.0),
                ..base()
            },
        ]),
    });

    out.push(Spec {
        name: "five_axis",
        description: "An extruding line carrying a non-trivial toolframe orientation (5-axis).",
        feature_tags: &["orientation", "five-axis"],
        frozen: false,
        emit: Some(EmitParams {
            five_axis: true,
            ..EmitParams::default()
        }),
        ir: tp(vec![Segment {
            orientation: Some([0.0, 0.3826834, 0.9238795]),
            ..base()
        }]),
    });

    out.push(Spec {
        name: "meta_header",
        description: "A minimal line carrying a full Meta header (generator, units, source_hash, \
                      invariants).",
        feature_tags: &["meta", "provenance"],
        frozen: true,
        emit: Some(EmitParams::default()),
        ir: Toolpath {
            version: 0,
            meta: Some(Meta {
                generator: Some("dry 0.2.0".to_string()),
                units: Some("mm".to_string()),
                source_hash: Some("0123456789abcdef".to_string()),
                invariants: vec!["monotonic_z".to_string(), "bounds".to_string()],
            }),
            segments: vec![base()],
        },
    });

    out.push(Spec {
        name: "empty",
        description:
            "An empty toolpath (zero segments). Serializes as {\"version\":0,\"segments\":[]}.",
        feature_tags: &["empty", "edge-case"],
        frozen: true,
        emit: None,
        ir: tp(vec![]),
    });

    out
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Regenerate one vector's artifacts from its seed. Returns (filename, bytes) pairs in a stable order.
fn artifacts(spec: &Spec) -> Vec<(&'static str, Vec<u8>)> {
    let mut files: Vec<(&'static str, Vec<u8>)> = Vec::new();
    files.push(("input.json", spec.ir.to_json().into_bytes()));
    files.push(("expected.dry0", spec.ir.to_bytes()));
    files.push(("expected.dry1", spec.ir.to_streaming_bytes()));
    let metrics = simulate(&spec.ir);
    let metrics_json = serde_json::to_string_pretty(&metrics).expect("metrics serialise") + "\n";
    files.push(("metrics.json", metrics_json.into_bytes()));
    if let Some(params) = &spec.emit {
        let gcode = emit(&spec.ir, params).join("\n") + "\n";
        files.push(("expected.gcode", gcode.into_bytes()));
    }
    files
}

fn vector_json(spec: &Spec) -> Vec<u8> {
    let value = serde_json::json!({
        "name": spec.name,
        "description": spec.description,
        "feature_tags": spec.feature_tags,
        "ir_version": spec.ir.version,
        "frozen": spec.frozen,
        "has_gcode": spec.emit.is_some(),
        "emit_params": spec.emit.as_ref().map(|p| serde_json::json!({
            "relative_e": p.relative_e,
            "travel_g1_e0": p.travel_g1_e0,
            "five_axis": p.five_axis,
            "kinematics": format!("{:?}", p.kinematics),
            "flavor": format!("{:?}", p.flavor),
        })),
    });
    (serde_json::to_string_pretty(&value).unwrap() + "\n").into_bytes()
}

#[test]
fn spec_vectors_match_or_update() {
    let update = update_mode();
    let dir = vectors_dir();
    let specs = specs();
    let mut manifest_vectors = Vec::new();

    for spec in &specs {
        let vdir = dir.join(spec.name);
        let mut files = artifacts(spec);
        files.push(("vector.json", vector_json(spec)));

        // Round-trip sanity: the committed/seed JSON decodes back to the seed exactly.
        let parsed: Toolpath =
            serde_json::from_slice(&files.iter().find(|(n, _)| *n == "input.json").unwrap().1)
                .unwrap_or_else(|e| panic!("[{}] input.json re-parse: {e}", spec.name));
        assert_eq!(parsed, spec.ir, "[{}] JSON round-trip != seed", spec.name);

        let mut artifact_hashes = serde_json::Map::new();
        for (fname, bytes) in &files {
            let path = vdir.join(fname);
            if update {
                fs::create_dir_all(&vdir).unwrap();
                fs::write(&path, bytes).unwrap();
            } else {
                let committed = fs::read(&path).unwrap_or_else(|_| {
                    panic!(
                        "[{}] missing {fname} — run `UPDATE_VECTORS=1 cargo test -p dry-core \
                         --test spec_vectors` to (re)generate vectors",
                        spec.name
                    )
                });
                assert_eq!(
                    &committed, bytes,
                    "[{}] {fname} drifted from the engine output",
                    spec.name
                );
            }
            artifact_hashes.insert(fname.to_string(), sha256_hex(bytes).into());
        }

        // A frozen vector must always decode from both binary forms (back-compat regression).
        if spec.frozen {
            let from0 = Toolpath::from_bytes(&spec.ir.to_bytes())
                .unwrap_or_else(|e| panic!("[{}] frozen DRY0 decode: {e:?}", spec.name));
            let from1 = Toolpath::from_bytes(&spec.ir.to_streaming_bytes())
                .unwrap_or_else(|e| panic!("[{}] frozen DRY1 decode: {e:?}", spec.name));
            assert_eq!(from0, spec.ir, "[{}] frozen DRY0 != seed", spec.name);
            assert_eq!(from1, spec.ir, "[{}] frozen DRY1 != seed", spec.name);
        }

        manifest_vectors.push(serde_json::json!({
            "name": spec.name,
            "description": spec.description,
            "feature_tags": spec.feature_tags,
            "frozen": spec.frozen,
            "ir_version": spec.ir.version,
            "has_gcode": spec.emit.is_some(),
            "artifacts": artifact_hashes,
        }));
    }

    let manifest = serde_json::json!({
        "spec": "docs/10-dry-ir-v0-spec.md",
        "schema": "spec/dry-ir-v0.schema.json",
        "ir_version": 0u32,
        "vectors": manifest_vectors,
    });
    let manifest_bytes = (serde_json::to_string_pretty(&manifest).unwrap() + "\n").into_bytes();
    let manifest_path = dir.join("MANIFEST.json");
    if update {
        fs::create_dir_all(&dir).unwrap();
        fs::write(&manifest_path, &manifest_bytes).unwrap();
    } else {
        let committed = fs::read(&manifest_path).expect("MANIFEST.json exists");
        assert_eq!(committed, manifest_bytes, "MANIFEST.json drifted");
    }

    if update {
        generate_negatives(&dir);
    }

    eprintln!("spec vectors: {} vectors checked", specs.len());
}

/// Author the negative (must-reject / documented-failure) vectors by mutating the minimal_line
/// encodings in their *uncompressed headers* (the only robustly hand-editable region) plus two JSON
/// cases. The independent Python validator additionally synthesizes unknown-kind/unknown-flag bodies.
fn generate_negatives(dir: &std::path::Path) {
    let ndir = dir.join("_negative");
    fs::create_dir_all(&ndir).unwrap();
    let seed = tp(vec![base()]);
    let dry0 = seed.to_bytes();
    let dry1 = seed.to_streaming_bytes();

    // DRY0 header: magic[0..4], enc_ver[4], ir_ver[5..9], n[9..13], body_len[13..17].
    let mut bad_magic = dry0.clone();
    bad_magic[0..4].copy_from_slice(b"DRYX");
    fs::write(ndir.join("bad_magic.dry0"), &bad_magic).unwrap();

    let mut bad_enc = dry0.clone();
    bad_enc[4] = 9;
    fs::write(ndir.join("unsupported_enc_ver.dry0"), &bad_enc).unwrap();

    let mut bad_body_len = dry0.clone();
    bad_body_len[13..17].copy_from_slice(&1u32.to_le_bytes()); // inflate bound too small
    fs::write(ndir.join("bad_body_len.dry0"), &bad_body_len).unwrap();

    let mut trailing_dry0 = dry0.clone();
    trailing_dry0.push(0xff);
    fs::write(ndir.join("trailing.dry0"), &trailing_dry0).unwrap();

    // DRY1 header: magic[0..4], enc_ver[4], ir_ver[5..9], n[9..13], block_size[13..17].
    let mut zero_block = dry1.clone();
    zero_block[13..17].copy_from_slice(&0u32.to_le_bytes());
    fs::write(ndir.join("block_size_zero.dry1"), &zero_block).unwrap();

    let truncated = &dry1[..dry1.len().min(10)];
    fs::write(ndir.join("truncated.dry1"), truncated).unwrap();

    let mut trailing_dry1 = dry1.clone();
    trailing_dry1.push(0xff);
    fs::write(ndir.join("trailing.dry1"), &trailing_dry1).unwrap();

    fs::write(
        ndir.join("unknown_kind.json"),
        b"{\"version\":0,\"segments\":[{\"start\":[0.0,0.0,0.0],\"end\":[1.0,0.0,0.0],\"travel\":false,\"speed\":1500.0,\"length\":1.0,\"volume\":0.0,\"filament\":0.0,\"width\":null,\"height\":null,\"kind\":\"frobnicate\"}]}\n",
    )
    .unwrap();

    // Unknown object keys MUST be ignored (forward-compat) — this case is expected to be ACCEPTED.
    fs::write(
        ndir.join("unknown_key_accepted.json"),
        b"{\"version\":0,\"future_field\":true,\"segments\":[{\"start\":[0.0,0.0,0.0],\"end\":[1.0,0.0,0.0],\"travel\":false,\"speed\":1500.0,\"length\":1.0,\"volume\":0.0,\"filament\":0.0,\"width\":null,\"height\":null}]}\n",
    )
    .unwrap();

    let index = serde_json::json!([
        {"file": "bad_magic.dry0", "format": "binary", "expect": "reject", "reason": "wrong magic (spec section 11)"},
        {"file": "unsupported_enc_ver.dry0", "format": "binary", "expect": "reject", "reason": "unsupported enc_ver"},
        {"file": "bad_body_len.dry0", "format": "binary", "expect": "reject", "reason": "inflated length != declared body_len"},
        {"file": "trailing.dry0", "format": "binary", "expect": "reject", "reason": "trailing bytes after DRY0 DEFLATE stream"},
        {"file": "block_size_zero.dry1", "format": "binary", "expect": "reject", "reason": "DRY1 block_size == 0"},
        {"file": "truncated.dry1", "format": "binary", "expect": "reject", "reason": "truncated stream"},
        {"file": "trailing.dry1", "format": "binary", "expect": "reject", "reason": "trailing bytes after DRY1 blocks"},
        {"file": "unknown_kind.json", "format": "json", "expect": "reject", "reason": "unknown SegmentKind string"},
        {"file": "unknown_key_accepted.json", "format": "json", "expect": "accept", "reason": "unknown object keys are ignored (forward-compat)"}
    ]);
    fs::write(
        ndir.join("INDEX.json"),
        serde_json::to_string_pretty(&index).unwrap() + "\n",
    )
    .unwrap();
}

#[test]
fn negative_vectors_are_rejected() {
    // In bless mode the generator (a separate test) writes _negative/ concurrently; nothing to verify.
    if update_mode() {
        return;
    }
    let ndir = vectors_dir().join("_negative");
    let index_path = ndir.join("INDEX.json");
    let index: serde_json::Value = serde_json::from_slice(
        &fs::read(&index_path).expect("run UPDATE_VECTORS=1 to generate _negative vectors"),
    )
    .unwrap();

    for case in index.as_array().unwrap() {
        let file = case["file"].as_str().unwrap();
        let format = case["format"].as_str().unwrap();
        let expect = case["expect"].as_str().unwrap();
        let bytes = fs::read(ndir.join(file)).unwrap();

        let accepted = match format {
            "binary" => Toolpath::from_bytes(&bytes).is_ok(),
            "json" => serde_json::from_slice::<Toolpath>(&bytes).is_ok(),
            other => panic!("unknown format {other}"),
        };
        match expect {
            "reject" => assert!(!accepted, "{file} should have been rejected"),
            "accept" => assert!(accepted, "{file} should have been accepted"),
            other => panic!("unknown expect {other}"),
        }
    }
}

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
    emit, resolve_checked, simulate, verify, Contracts, Design, EmitParams, Feedrate, Length, Meta,
    Op, ResolveParams, Segment, SegmentKind, Toolpath, Volume,
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
    /// The L1 design the IR was *resolved from*, for vectors authored at the design tier rather than
    /// as a hand-built IR seed. `None` keeps the original contract (the `ir` below is the seed).
    /// When present it is written out as `design.json` next to `input.json`, so a binding that builds
    /// outside this workspace can drive the same op list and diff its own g-code and metrics against
    /// the committed ones — `web/smoke.cjs` does exactly that.
    design: Option<DesignSource>,
    description: &'static str,
    feature_tags: &'static [&'static str],
    frozen: bool,
    emit: Option<EmitParams>,
    ir: Toolpath,
}

/// An L1 design plus the parameters it resolves under — the design-tier seed of a vector.
struct DesignSource {
    ops: Vec<Op>,
    params: ResolveParams,
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

// ---------------------------------------------------------------------------------------------
// five_axis_drape — the one design-tier vector: a path draped over a dome, tool along the normal.
// ---------------------------------------------------------------------------------------------

/// Radius of the dome the `five_axis_drape` path is draped over (mm). The dome is the upper half of
/// a sphere of this radius centred on the origin, which is also the machine origin.
const DOME_RADIUS_MM: f64 = 25.0;

/// The `(x, y)` offsets from the dome axis at which the path is sampled, in path order.
///
/// Every entry is chosen so that `x² + y² + z² = DOME_RADIUS_MM²` has an **integer** `z`: `sqrt` is
/// the one transcendental-looking operation here and IEEE-754 requires it to be correctly rounded, so
/// on a perfect square it is exact on every platform. That keeps the whole vector — surface points and
/// surface normals alike — free of any accumulated-rounding provenance question, and it is why
/// `the_drape_vector_normals_are_exactly_unit` can assert bit-exact unit length rather than a
/// tolerance. The sequence spirals in towards the apex: the polar tilt falls monotonically to zero
/// while the azimuth rises monotonically, so both rotary axes move in one direction only.
const DOME_SAMPLES_MM: [(f64, f64); 5] = [
    (20.0, 9.0),
    (16.0, 12.0),
    (9.0, 12.0),
    (0.0, 7.0),
    (0.0, 0.0),
];

/// The surface point and the outward unit normal of the dome at the offset `(dx, dy)`.
///
/// For the sphere `x² + y² + z² = R²` the outward normal at a point is the (normalised) radius vector,
/// so the normal is just the point over `R` — no gradient, no finite difference, no epsilon.
fn dome_point_and_normal(dx: f64, dy: f64) -> ([f64; 3], [f64; 3]) {
    let z = libm::sqrt(DOME_RADIUS_MM * DOME_RADIUS_MM - dx * dx - dy * dy);
    (
        [dx, dy, z],
        [dx / DOME_RADIUS_MM, dy / DOME_RADIUS_MM, z / DOME_RADIUS_MM],
    )
}

/// The L1 design of the `five_axis_drape` vector.
///
/// A travel to the first sample point (no orientation — the tool is not on the surface yet), then one
/// extruding move per remaining sample, each preceded by an `orient` to the dome normal **at that
/// move's destination**. The convention is deliberate and is the whole content of the fixture: an
/// `orient` op sets state that the *following* motion carries, so "orient, then move" means the tool
/// is held along the surface normal of the point it arrives at.
fn dome_drape_design() -> Design {
    let (first, _) = dome_point_and_normal(DOME_SAMPLES_MM[0].0, DOME_SAMPLES_MM[0].1);
    let mut ops = vec![
        Op::Geometry {
            width: Some(0.6),
            height: Some(0.3),
        },
        Op::Speed { print: 900.0 },
        Op::Extruder { on: false },
        Op::Move {
            x: Some(first[0]),
            y: Some(first[1]),
            z: Some(first[2]),
        },
        Op::Extruder { on: true },
    ];
    for (dx, dy) in DOME_SAMPLES_MM.iter().skip(1) {
        let (point, normal) = dome_point_and_normal(*dx, *dy);
        ops.push(Op::Orient {
            i: normal[0],
            j: normal[1],
            k: normal[2],
        });
        ops.push(Op::Move {
            x: Some(point[0]),
            y: Some(point[1]),
            z: Some(point[2]),
        });
    }
    Design { ops }
}

fn specs() -> Vec<Spec> {
    let mut out = vec![Spec {
        name: "minimal_line",
        design: None,
        description: "One extruding line, all channels unset — the byte-identity baseline.",
        feature_tags: &["line", "no-channels"],
        frozen: true,
        emit: Some(EmitParams::default()),
        ir: tp(vec![base()]),
    }];

    out.push(Spec {
        name: "travel_and_line",
        design: None,
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
        design: None,
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
        design: None,
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
        design: None,
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
        design: None,
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
        design: None,
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
        design: None,
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
        design: None,
        description:
            "An extruding line with every process channel set: temperature, fan, flow, tool.",
        feature_tags: &["temperature", "fan", "flow", "tool"],
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
        name: "five_axis",
        design: None,
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

    // The only vector in this corpus that starts life as an L1 *design*: its IR is what `resolve`
    // makes of `dome_drape_design()`, not a hand-built seed. It is also the only oriented design in
    // any frozen corpus here — `conformance/gallery/` is FullControl-oracle output and FullControl is
    // 3-axis, so an oriented design cannot live there without borrowing an authority no oracle grants
    // it. See the description for exactly what does and does not back this one.
    let drape = DesignSource {
        ops: dome_drape_design().ops,
        params: ResolveParams::default(),
    };
    let drape_ir = resolve_checked(
        &Design {
            ops: drape.ops.clone(),
        },
        &drape.params,
    )
    .expect("the dome-drape design resolves");
    out.push(Spec {
        name: "five_axis_drape",
        design: Some(drape),
        description: "A five-point path draped over a 25 mm dome: each extruding move carries the \
                      dome's outward surface normal at its destination as the toolframe orientation, \
                      so both rotary axes sweep monotonically and the last move ends pointing +Z. \
                      NOT oracle-backed — FullControl is 3-axis, so nothing outside Dry produces this \
                      path. What backs it: the IR is `resolve`'s own output for the committed \
                      `design.json` (not hand-written), the orientations are the closed-form sphere \
                      normal `point / radius` at integer lattice points where that is exactly unit in \
                      binary64, and `metrics.json` / `expected.gcode` are this engine's `simulate` / \
                      `emit`. Complements `five_axis`, which is a single IR-authored segment of \
                      constant orientation: this one changes orientation four times, so it pins the \
                      emit-a-rotary-word-only-when-it-changes rule, and it mixes an unoriented travel \
                      with oriented extrusion.",
        feature_tags: &[
            "orientation",
            "five-axis",
            "non-planar",
            "l1-design",
            "no-oracle",
        ],
        frozen: false,
        emit: Some(EmitParams {
            five_axis: true,
            ..EmitParams::default()
        }),
        ir: drape_ir,
    });

    out.push(Spec {
        name: "meta_header",
        design: None,
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
        design: None,
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
    if let Some(source) = &spec.design {
        // Serialised as a typed struct rather than through `serde_json::Value`, whose map is a
        // BTreeMap: going through `Value` would alphabetise the keys and bury each op's `"op"` tag
        // in the middle of its own object.
        #[derive(serde::Serialize)]
        struct DesignDocument<'a> {
            ops: &'a [Op],
            resolve_params: &'a ResolveParams,
        }
        let doc = DesignDocument {
            ops: &source.ops,
            resolve_params: &source.params,
        };
        let json = serde_json::to_string_pretty(&doc).expect("design serialise") + "\n";
        files.push(("design.json", json.into_bytes()));
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

        // A design-tier vector publishes the op list a foreign binding is expected to drive. That is
        // only worth publishing if it round-trips *and* still resolves to the IR beside it: a
        // `design.json` that resolved to something else would send every out-of-workspace consumer
        // chasing a g-code diff that is not there.
        if spec.design.is_some() {
            let doc: serde_json::Value =
                serde_json::from_slice(&files.iter().find(|(n, _)| *n == "design.json").unwrap().1)
                    .unwrap_or_else(|e| panic!("[{}] design.json re-parse: {e}", spec.name));
            let ops: Vec<Op> = serde_json::from_value(doc["ops"].clone())
                .unwrap_or_else(|e| panic!("[{}] design.json ops: {e}", spec.name));
            let params: ResolveParams = serde_json::from_value(doc["resolve_params"].clone())
                .unwrap_or_else(|e| panic!("[{}] design.json resolve_params: {e}", spec.name));
            let resolved = resolve_checked(&Design { ops }, &params)
                .unwrap_or_else(|e| panic!("[{}] design.json does not resolve: {e}", spec.name));
            assert_eq!(
                resolved, spec.ir,
                "[{}] design.json resolves to something other than input.json",
                spec.name
            );
        }

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

/// The dome normals are unit vectors *exactly*, not within a tolerance.
///
/// This is the reason the sample offsets are integer lattice points of the sphere: `verify`'s
/// `orientation-not-unit` rule admits `|n| - 1` up to 1e-6, and a fixture that merely passed that
/// would leave open whether the orientations were derived or typed. Bit-exactness closes it.
#[test]
fn the_drape_vector_normals_are_exactly_unit() {
    for (dx, dy) in DOME_SAMPLES_MM {
        let (point, n) = dome_point_and_normal(dx, dy);
        let mag = libm::sqrt(n[0] * n[0] + n[1] * n[1] + n[2] * n[2]);
        assert_eq!(mag, 1.0, "normal at {point:?} has magnitude {mag}");
    }
}

/// Every extruding segment of the drape vector carries the dome normal at the point it ends on, and
/// the toolpath draws no `verify` finding — including none from `orientation-not-unit`.
#[test]
fn the_drape_vector_segments_carry_the_dome_normal() {
    let ir = resolve_checked(&dome_drape_design(), &ResolveParams::default())
        .expect("the dome-drape design resolves");

    // segment 0 is the travel onto the surface and predates the first `orient`.
    assert!(ir.segments[0].travel);
    assert_eq!(ir.segments[0].orientation, None);

    let oriented = &ir.segments[1..];
    assert_eq!(oriented.len(), DOME_SAMPLES_MM.len() - 1);
    for (segment, (dx, dy)) in oriented.iter().zip(DOME_SAMPLES_MM.iter().skip(1)) {
        let (point, normal) = dome_point_and_normal(*dx, *dy);
        let end = segment.end.map(|axis| axis.expect("explicit endpoint").0);
        assert_eq!(end, point, "segment does not end on the sampled dome point");
        assert_eq!(
            segment.orientation,
            Some(normal),
            "segment ending at {point:?} does not carry the dome normal"
        );
    }

    let report = verify(&ir, &Contracts::default());
    assert!(
        report.findings.is_empty(),
        "the drape vector must verify clean: {:?}",
        report.findings
    );
}

/// The committed `expected.gcode` really does carry the orientation into rotary words.
///
/// A 5-axis vector whose g-code happened to drop the orientation would still round-trip, still hash,
/// and still look fine — so re-derive every A/B word from the documented AB-head convention
/// (`B = atan2(i, k)`, `A = atan2(j, hypot(i, k))`, degrees) and check the committed line against it.
/// The comparison bound is the emitter's own print precision (six decimal places, so at most 5e-7 of
/// rounding), not a numeric tolerance in the engine.
#[test]
fn the_drape_vector_gcode_carries_the_rotary_words() {
    let path = vectors_dir().join("five_axis_drape/expected.gcode");
    let gcode = fs::read_to_string(&path).expect("five_axis_drape/expected.gcode exists");

    // The A/B state the machine is in after each emitted line, modal: a word is emitted only when it
    // changes, so the expected value has to be tracked across lines, not read off one.
    let mut want: Vec<(f64, f64)> = Vec::new();
    for (dx, dy) in DOME_SAMPLES_MM.iter().skip(1) {
        let (_, [i, j, k]) = dome_point_and_normal(*dx, *dy);
        let b = libm::atan2(i, k).to_degrees();
        let a = libm::atan2(j, libm::hypot(i, k)).to_degrees();
        want.push((a, b));
    }

    let mut a_state = 0.0_f64;
    let mut b_state = 0.0_f64;
    let mut seen = 0usize;
    let mut a_words = 0usize;
    let mut b_words = 0usize;
    for line in gcode.lines().filter(|l| l.starts_with("G1")) {
        for word in line.split_whitespace() {
            let (axis, value) = word.split_at(1);
            match axis {
                "A" => {
                    a_state = value.parse().expect("A word parses");
                    a_words += 1;
                }
                "B" => {
                    b_state = value.parse().expect("B word parses");
                    b_words += 1;
                }
                _ => {}
            }
        }
        let &(a, b) = want
            .get(seen)
            .unwrap_or_else(|| panic!("more G1 lines than oriented moves: `{line}`"));
        assert!(
            (a_state - a).abs() <= 1e-6 && (b_state - b).abs() <= 1e-6,
            "line {seen} `{line}` implies A{a_state} B{b_state}, the dome normal implies A{a} B{b}"
        );
        seen += 1;
    }
    assert_eq!(seen, want.len(), "expected one G1 per oriented move");
    // Modality is part of what this vector exists to pin. Four oriented moves, but only three A words
    // and three B words: normals 1 and 2 share an A (both have j = 0.48, so the polar tilt off the
    // j axis is the same and only the azimuth moves), and normals 3 and 4 share B = 0 (both lie in
    // the i = 0 plane). A single-segment vector cannot exercise this at all.
    assert_eq!((a_words, b_words), (3, 3), "rotary-word modality changed");
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

//! OpenToolpath (`.otp`) native package container packaging and emission.
//!
//! Conforms to:
//! - Normative Standard: `docs/29-opentoolpath-spec.md`
//! - JSON Schema (Draft 2020-12): `spec/opentoolpath-v1.schema.json`
//! - ADR 0001 (Formal Assurance Constitution) & ADR 0002 (Numeric Ingress & Emission Gates)

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{self, Write};

use super::EmitParams;
use crate::ir::{Segment, Toolpath};

pub const OTP_MIMETYPE: &str = "application/vnd.opentoolpath+zip";
pub const OTP_SPEC_VERSION: &str = "1.0";
pub const OTP_SCHEMA_URL: &str = "https://opentoolpath.org/spec/v1/opentoolpath-v1.schema.json";

/// Configuration frame for OpenToolpath package generation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OtpFrame {
    pub package_id: Option<String>,
    pub created_at: Option<String>,
    pub domain: Option<String>,
    pub sub_type: Option<String>,
    pub description: Option<String>,
    pub payload_format: Option<String>,
    pub conformance_level: Option<String>,
    pub tools: Option<Vec<OtpToolDefinition>>,
    pub context: Option<OtpContextDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpManifest {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub otp_version: String,
    pub package_id: String,
    pub created_at: String,
    pub generator: OtpGenerator,
    pub process: OtpProcess,
    pub entrypoints: OtpEntrypoints,
    pub digests: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conformance: Option<OtpConformance>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpGenerator {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpProcess {
    pub domain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpEntrypoints {
    pub payload: String,
    pub payload_format: String,
    pub tools: String,
    pub context: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpConformance {
    pub level: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invariants: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpToolsCatalog {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub schema_version: String,
    pub tools: Vec<OtpToolDefinition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpToolDefinition {
    pub id: u32,
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<OtpToolGeometry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offsets: Option<OtpToolOffsets>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<OtpToolCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thermal: Option<OtpToolThermal>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpToolGeometry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diameter: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flute_length: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overall_length: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flute_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orifice_diameter: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filament_diameter: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpToolOffsets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length_offset: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius_offset: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpToolCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_rpm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_feedrate: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpToolThermal {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_temperature: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpContextDescriptor {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub schema_version: String,
    pub machine: OtpMachine,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_coordinates: Option<OtpWorkCoordinates>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stock: Option<OtpStock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpMachine {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub kinematics: OtpKinematics,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpKinematics {
    pub topology: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub axes: Vec<OtpAxis>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpAxis {
    pub name: String,
    #[serde(rename = "type")]
    pub axis_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_velocity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis_vector: Option<[f64; 3]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpWorkCoordinates {
    pub active_system: String,
    pub offsets: BTreeMap<String, BTreeMap<String, f64>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpStock {
    #[serde(rename = "type")]
    pub stock_type: String,
    pub dimensions: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<[f64; 3]>,
}

/// Statistics collected during OpenToolpath emission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OtpEmitStats {
    pub segments_count: usize,
    pub tools_count: usize,
    pub total_bytes_written: usize,
}

// ---------------------------------------------------------------------------
// CRC32 implementation (IEEE 802.3 polynomial 0xEDB88320)
// ---------------------------------------------------------------------------

const fn make_crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

const CRC32_TABLE: [u32; 256] = make_crc32_table();

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFFu32;
    for &byte in data {
        crc = (crc >> 8) ^ CRC32_TABLE[((crc ^ (byte as u32)) & 0xFF) as usize];
    }
    !crc
}

pub fn sha256_digest(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    let mut hex = String::with_capacity(64 + 7);
    hex.push_str("sha256:");
    for byte in hash {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

// ---------------------------------------------------------------------------
// Pure-Rust ZIP Archive Builder (PKZIP 2.04g compatible)
// ---------------------------------------------------------------------------

struct ZipEntryMeta {
    name: String,
    uncompressed_size: u32,
    compressed_size: u32,
    crc: u32,
    compression_method: u16,
    local_header_offset: u32,
}

pub struct ZipArchiveWriter<W: Write> {
    writer: W,
    offset: usize,
    entries: Vec<ZipEntryMeta>,
}

impl<W: Write> ZipArchiveWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            offset: 0,
            entries: Vec::new(),
        }
    }

    /// Add an entry to the ZIP archive. If `deflate` is true, attempts DEFLATE compression.
    pub fn add_file(&mut self, name: &str, data: &[u8], deflate: bool) -> io::Result<()> {
        let uncompressed_size = data.len() as u32;
        let crc = crc32(data);
        let (compression_method, payload) = if deflate {
            let compressed = miniz_oxide::deflate::compress_to_vec(data, 6);
            if compressed.len() < data.len() {
                (8u16, compressed)
            } else {
                (0u16, data.to_vec())
            }
        } else {
            (0u16, data.to_vec())
        };

        let compressed_size = payload.len() as u32;
        let local_header_offset = self.offset as u32;
        let name_bytes = name.as_bytes();

        // Local file header (30 bytes + name length)
        self.writer.write_all(&0x04034b50u32.to_le_bytes())?;
        self.writer.write_all(&20u16.to_le_bytes())?; // version needed to extract (2.0)
        self.writer.write_all(&(1u16 << 11).to_le_bytes())?; // UTF-8 filename flag
        self.writer.write_all(&compression_method.to_le_bytes())?;
        self.writer.write_all(&0u16.to_le_bytes())?; // mod time
        self.writer.write_all(&0u16.to_le_bytes())?; // mod date
        self.writer.write_all(&crc.to_le_bytes())?;
        self.writer.write_all(&compressed_size.to_le_bytes())?;
        self.writer.write_all(&uncompressed_size.to_le_bytes())?;
        self.writer
            .write_all(&(name_bytes.len() as u16).to_le_bytes())?;
        self.writer.write_all(&0u16.to_le_bytes())?; // extra field length
        self.writer.write_all(name_bytes)?;
        self.writer.write_all(&payload)?;

        self.offset += 30 + name_bytes.len() + payload.len();

        self.entries.push(ZipEntryMeta {
            name: name.to_string(),
            uncompressed_size,
            compressed_size,
            crc,
            compression_method,
            local_header_offset,
        });

        Ok(())
    }

    /// Write central directory and End of Central Directory (EOCD).
    pub fn finish(mut self) -> io::Result<usize> {
        let cd_start = self.offset as u32;

        for entry in &self.entries {
            let name_bytes = entry.name.as_bytes();
            // Central directory file header (46 bytes + name length)
            self.writer.write_all(&0x02014b50u32.to_le_bytes())?;
            self.writer.write_all(&0x0314u16.to_le_bytes())?; // version made by (UNIX + 2.0)
            self.writer.write_all(&20u16.to_le_bytes())?; // version needed (2.0)
            self.writer.write_all(&(1u16 << 11).to_le_bytes())?; // UTF-8 filename flag
            self.writer
                .write_all(&entry.compression_method.to_le_bytes())?;
            self.writer.write_all(&0u16.to_le_bytes())?; // mod time
            self.writer.write_all(&0u16.to_le_bytes())?; // mod date
            self.writer.write_all(&entry.crc.to_le_bytes())?;
            self.writer
                .write_all(&entry.compressed_size.to_le_bytes())?;
            self.writer
                .write_all(&entry.uncompressed_size.to_le_bytes())?;
            self.writer
                .write_all(&(name_bytes.len() as u16).to_le_bytes())?;
            self.writer.write_all(&0u16.to_le_bytes())?; // extra field length
            self.writer.write_all(&0u16.to_le_bytes())?; // comment length
            self.writer.write_all(&0u16.to_le_bytes())?; // disk number start
            self.writer.write_all(&0u16.to_le_bytes())?; // internal file attributes
            self.writer.write_all(&0x81a40000u32.to_le_bytes())?; // external file attributes (-rw-r--r--)
            self.writer
                .write_all(&entry.local_header_offset.to_le_bytes())?;
            self.writer.write_all(name_bytes)?;

            self.offset += 46 + name_bytes.len();
        }

        let cd_size = (self.offset as u32) - cd_start;
        let count = self.entries.len() as u16;

        // End of central directory record (22 bytes)
        self.writer.write_all(&0x06054b50u32.to_le_bytes())?;
        self.writer.write_all(&0u16.to_le_bytes())?; // disk number
        self.writer.write_all(&0u16.to_le_bytes())?; // start disk
        self.writer.write_all(&count.to_le_bytes())?; // records on this disk
        self.writer.write_all(&count.to_le_bytes())?; // total records
        self.writer.write_all(&cd_size.to_le_bytes())?;
        self.writer.write_all(&cd_start.to_le_bytes())?;
        self.writer.write_all(&0u16.to_le_bytes())?; // comment length

        self.offset += 22;
        self.writer.flush()?;

        Ok(self.offset)
    }
}

// ---------------------------------------------------------------------------
// OpenToolpath Emission Engine
// ---------------------------------------------------------------------------

use crate::codec::CodecError;

/// Emit a stream of segments as a conforming `.otp` package archive to `writer`.
pub fn emit_otp_to_writer<I, W>(
    stream: I,
    params: &EmitParams,
    writer: &mut W,
) -> Result<OtpEmitStats, CodecError>
where
    I: IntoIterator<Item = Result<Segment, CodecError>>,
    W: Write,
{
    let segments: Vec<Segment> = stream.into_iter().collect::<Result<Vec<_>, _>>()?;
    let opt_frame = &params.otp_frame;

    // 1. Build Payload Toolpath
    let payload_format = opt_frame
        .payload_format
        .as_deref()
        .unwrap_or("json")
        .to_ascii_lowercase();

    let (payload_filename, payload_bytes) = match payload_format.as_str() {
        "json" => {
            let tp = Toolpath {
                version: 1,
                meta: Some(crate::ir::Meta {
                    generator: Some(format!("dry {}", env!("CARGO_PKG_VERSION"))),
                    units: Some("mm".to_string()),
                    source_hash: None,
                    invariants: vec![
                        "FM1.L2.WELL_FORMED.VALIDATION".to_string(),
                        "FM1.RESOLVE_CHANNELS".to_string(),
                        "FM1.DEPOSITION.CONSERVATION".to_string(),
                    ],
                }),
                segments: segments.clone(),
            };
            let json_str = serde_json::to_string_pretty(&tp).map_err(|e| {
                CodecError::Other(format!("failed to serialize OTP toolpath JSON: {e}"))
            })?;
            ("payload/toolpath.json".to_string(), json_str.into_bytes())
        }
        "dry0" => {
            let tp = Toolpath {
                version: 0,
                meta: None,
                segments: segments.clone(),
            };
            let bytes = tp.try_to_bytes().map_err(|e| {
                CodecError::Other(format!("failed to serialize DRY0 binary payload: {e}"))
            })?;
            ("payload/toolpath.dry0".to_string(), bytes)
        }
        "dry1" => {
            let tp = Toolpath {
                version: 0,
                meta: None,
                segments: segments.clone(),
            };
            let bytes = tp.try_to_streaming_bytes().map_err(|e| {
                CodecError::Other(format!("failed to serialize DRY1 streaming payload: {e}"))
            })?;
            ("payload/toolpath.dry1".to_string(), bytes)
        }
        other => {
            return Err(CodecError::Other(format!(
                "unsupported OpenToolpath payload format: {other} (expected json, dry0, or dry1)"
            )))
        }
    };

    // 2. Build Physical Tools Catalog
    let tools = if let Some(ref user_tools) = opt_frame.tools {
        user_tools.clone()
    } else {
        // Infer active tool IDs from segments
        let mut tool_ids = BTreeMap::new();
        for seg in &segments {
            if let Some(tid) = seg.tool {
                tool_ids.insert(tid, ());
            }
        }
        if tool_ids.is_empty() {
            let kind = if params.flavor.is_cnc() {
                "endmill"
            } else if params.flavor.is_robot() {
                "spindle"
            } else {
                "fff_nozzle"
            };
            vec![OtpToolDefinition {
                id: 1,
                name: format!("Default {kind}"),
                kind: kind.to_string(),
                geometry: Some(OtpToolGeometry {
                    diameter: Some(if params.flavor.is_cnc() { 10.0 } else { 0.4 }),
                    corner_radius: Some(0.0),
                    flute_length: if params.flavor.is_cnc() {
                        Some(30.0)
                    } else {
                        None
                    },
                    overall_length: if params.flavor.is_cnc() {
                        Some(75.0)
                    } else {
                        None
                    },
                    flute_count: if params.flavor.is_cnc() {
                        Some(4)
                    } else {
                        None
                    },
                    orifice_diameter: if !params.flavor.is_cnc() {
                        Some(0.4)
                    } else {
                        None
                    },
                    filament_diameter: if !params.flavor.is_cnc() {
                        Some(1.75)
                    } else {
                        None
                    },
                }),
                offsets: Some(OtpToolOffsets {
                    length_offset: Some(0.0),
                    radius_offset: Some(0.0),
                }),
                capabilities: Some(OtpToolCapabilities {
                    max_rpm: Some(24000.0),
                    max_feedrate: Some(12000.0),
                }),
                thermal: if !params.flavor.is_cnc() {
                    Some(OtpToolThermal {
                        max_temperature: Some(300.0),
                    })
                } else {
                    None
                },
            }]
        } else {
            tool_ids
                .into_keys()
                .map(|id| OtpToolDefinition {
                    id,
                    name: format!("Tool {id}"),
                    kind: if params.flavor.is_cnc() {
                        "endmill".to_string()
                    } else if params.flavor.is_robot() {
                        "spindle".to_string()
                    } else {
                        "fff_nozzle".to_string()
                    },
                    geometry: Some(OtpToolGeometry {
                        diameter: Some(if params.flavor.is_cnc() || params.flavor.is_robot() {
                            6.0
                        } else {
                            0.4
                        }),
                        corner_radius: Some(0.0),
                        flute_length: None,
                        overall_length: None,
                        flute_count: None,
                        orifice_diameter: None,
                        filament_diameter: None,
                    }),
                    offsets: None,
                    capabilities: None,
                    thermal: None,
                })
                .collect()
        }
    };

    let tools_catalog = OtpToolsCatalog {
        schema: Some(OTP_SCHEMA_URL.to_string()),
        schema_version: OTP_SPEC_VERSION.to_string(),
        tools: tools.clone(),
    };
    let tools_bytes = serde_json::to_string_pretty(&tools_catalog)
        .map_err(|e| CodecError::Other(format!("failed to serialize tools catalog: {e}")))?
        .into_bytes();

    // 3. Build Operational Machine Context
    let context_desc = if let Some(ref user_ctx) = opt_frame.context {
        user_ctx.clone()
    } else {
        let topology = if params.five_axis {
            match params.kinematics {
                crate::emit::Kinematics::Bc { .. } => "table_table_bc",
                crate::emit::Kinematics::Ac { .. } | crate::emit::Kinematics::Ab { .. } => {
                    "table_table_ac"
                }
            }
        } else if params.flavor.is_robot() {
            "robot_6dof"
        } else {
            "cartesian_3axis"
        };

        let mut axes = vec![
            OtpAxis {
                name: "X".to_string(),
                axis_type: "linear".to_string(),
                min: Some(-500.0),
                max: Some(500.0),
                max_velocity: Some(30000.0),
                axis_vector: Some([1.0, 0.0, 0.0]),
            },
            OtpAxis {
                name: "Y".to_string(),
                axis_type: "linear".to_string(),
                min: Some(-500.0),
                max: Some(500.0),
                max_velocity: Some(30000.0),
                axis_vector: Some([0.0, 1.0, 0.0]),
            },
            OtpAxis {
                name: "Z".to_string(),
                axis_type: "linear".to_string(),
                min: Some(0.0),
                max: Some(600.0),
                max_velocity: Some(20000.0),
                axis_vector: Some([0.0, 0.0, 1.0]),
            },
        ];

        if params.five_axis {
            match params.kinematics {
                crate::emit::Kinematics::Bc { .. } => {
                    axes.push(OtpAxis {
                        name: "B".to_string(),
                        axis_type: "rotary".to_string(),
                        min: Some(-120.0),
                        max: Some(120.0),
                        max_velocity: Some(3600.0),
                        axis_vector: Some([0.0, 1.0, 0.0]),
                    });
                    axes.push(OtpAxis {
                        name: "C".to_string(),
                        axis_type: "rotary".to_string(),
                        min: Some(-360.0),
                        max: Some(360.0),
                        max_velocity: Some(7200.0),
                        axis_vector: Some([0.0, 0.0, 1.0]),
                    });
                }
                crate::emit::Kinematics::Ac { .. } | crate::emit::Kinematics::Ab { .. } => {
                    axes.push(OtpAxis {
                        name: "A".to_string(),
                        axis_type: "rotary".to_string(),
                        min: Some(-120.0),
                        max: Some(120.0),
                        max_velocity: Some(3600.0),
                        axis_vector: Some([1.0, 0.0, 0.0]),
                    });
                    axes.push(OtpAxis {
                        name: "C".to_string(),
                        axis_type: "rotary".to_string(),
                        min: Some(-360.0),
                        max: Some(360.0),
                        max_velocity: Some(7200.0),
                        axis_vector: Some([0.0, 0.0, 1.0]),
                    });
                }
            }
        }

        let mut g54_offsets = BTreeMap::new();
        g54_offsets.insert("x".to_string(), 0.0);
        g54_offsets.insert("y".to_string(), 0.0);
        g54_offsets.insert("z".to_string(), 0.0);

        let mut wcs_offsets = BTreeMap::new();
        wcs_offsets.insert("G54".to_string(), g54_offsets);

        OtpContextDescriptor {
            schema: Some(OTP_SCHEMA_URL.to_string()),
            schema_version: OTP_SPEC_VERSION.to_string(),
            machine: OtpMachine {
                vendor: Some("Generic".to_string()),
                model: Some(topology.to_string()),
                kinematics: OtpKinematics {
                    topology: topology.to_string(),
                    axes,
                },
            },
            work_coordinates: Some(OtpWorkCoordinates {
                active_system: "G54".to_string(),
                offsets: wcs_offsets,
            }),
            stock: None,
        }
    };

    let context_bytes = serde_json::to_string_pretty(&context_desc)
        .map_err(|e| CodecError::Other(format!("failed to serialize context descriptor: {e}")))?
        .into_bytes();

    // 4. Compute Digests
    let mut digests = BTreeMap::new();
    digests.insert(payload_filename.clone(), sha256_digest(&payload_bytes));
    digests.insert("tools.json".to_string(), sha256_digest(&tools_bytes));
    digests.insert("context.json".to_string(), sha256_digest(&context_bytes));

    // 5. Build Manifest
    let domain = opt_frame.domain.clone().unwrap_or_else(|| {
        if params.flavor.is_cnc() {
            "subtractive".to_string()
        } else if params.flavor.is_robot() {
            "robotics".to_string()
        } else {
            "additive".to_string()
        }
    });

    let package_id = opt_frame.package_id.clone().unwrap_or_else(|| {
        // Deterministic package ID derived from payload digest (12 hex chars in node field)
        let payload_hash = &digests[&payload_filename][7..19];
        format!("urn:uuid:00000000-0000-4000-8000-{payload_hash}")
    });

    let created_at = opt_frame
        .created_at
        .clone()
        .unwrap_or_else(|| "2026-09-08T00:00:00Z".to_string());

    let manifest = OtpManifest {
        schema: Some(OTP_SCHEMA_URL.to_string()),
        otp_version: OTP_SPEC_VERSION.to_string(),
        package_id,
        created_at,
        generator: OtpGenerator {
            name: "dry".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            source_hash: None,
        },
        process: OtpProcess {
            domain,
            sub_type: opt_frame.sub_type.clone(),
            description: opt_frame.description.clone(),
        },
        entrypoints: OtpEntrypoints {
            payload: payload_filename.clone(),
            payload_format: payload_format.clone(),
            tools: "tools.json".to_string(),
            context: "context.json".to_string(),
        },
        digests,
        conformance: Some(OtpConformance {
            level: opt_frame
                .conformance_level
                .clone()
                .unwrap_or_else(|| "strict".to_string()),
            invariants: vec![
                "FM1.L2.WELL_FORMED.VALIDATION".to_string(),
                "FM1.RESOLVE_CHANNELS".to_string(),
                "FM1.DEPOSITION.CONSERVATION".to_string(),
            ],
        }),
    };

    let manifest_bytes = serde_json::to_string_pretty(&manifest)
        .map_err(|e| CodecError::Other(format!("failed to serialize manifest: {e}")))?
        .into_bytes();

    // 6. Write ZIP Archive
    let mut zip = ZipArchiveWriter::new(writer);

    // Entry 0: uncompressed mimetype (offset 30, exactly 32 bytes)
    zip.add_file("mimetype", OTP_MIMETYPE.as_bytes(), false)
        .map_err(|e| {
            CodecError::Other(format!("failed to write mimetype to OTP container: {e}"))
        })?;

    // Entry 1: manifest.json (deflated)
    zip.add_file("manifest.json", &manifest_bytes, true)
        .map_err(|e| {
            CodecError::Other(format!(
                "failed to write manifest.json to OTP container: {e}"
            ))
        })?;

    // Entry 2: payload (deflated)
    zip.add_file(&payload_filename, &payload_bytes, true)
        .map_err(|e| CodecError::Other(format!("failed to write payload to OTP container: {e}")))?;

    // Entry 3: tools.json (deflated)
    zip.add_file("tools.json", &tools_bytes, true)
        .map_err(|e| {
            CodecError::Other(format!("failed to write tools.json to OTP container: {e}"))
        })?;

    // Entry 4: context.json (deflated)
    zip.add_file("context.json", &context_bytes, true)
        .map_err(|e| {
            CodecError::Other(format!(
                "failed to write context.json to OTP container: {e}"
            ))
        })?;

    let total_bytes = zip
        .finish()
        .map_err(|e| CodecError::Other(format!("failed to finalize OTP zip container: {e}")))?;

    Ok(OtpEmitStats {
        segments_count: segments.len(),
        tools_count: tools.len(),
        total_bytes_written: total_bytes,
    })
}

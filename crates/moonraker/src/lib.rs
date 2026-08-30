//! `dry-moonraker` — Dry's Moonraker upload client. The only network code for the upload feature;
//! mirrors `dry-llm`'s feature-gated, ureq-based structure. `dry-core` stays pure.

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::Duration;

const MAX_JSON_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_ERROR_SNIPPET_BYTES: u64 = 500;

/// Connection to a Moonraker host. `api_key` is sent as `X-Api-Key` when present.
pub struct MoonrakerConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    /// End-to-end timeout applied to each Moonraker request.
    pub timeout: Duration,
}

#[derive(Debug)]
pub enum MoonrakerError {
    Http(u16, String),
    Transport(String),
    Decode(String),
    InvalidInput(String),
    Rejected(String),
}
impl std::fmt::Display for MoonrakerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoonrakerError::Http(c, b) => write!(f, "Moonraker returned HTTP {c}: {b}"),
            MoonrakerError::Transport(m) => write!(f, "network error reaching Moonraker: {m}"),
            MoonrakerError::Decode(m) => write!(f, "could not parse Moonraker response: {m}"),
            MoonrakerError::InvalidInput(m) => write!(f, "invalid Moonraker request: {m}"),
            MoonrakerError::Rejected(m) => write!(f, "Moonraker rejected the request: {m}"),
        }
    }
}
impl std::error::Error for MoonrakerError {}

#[derive(Debug, Deserialize)]
pub struct UploadResponse {
    pub filename: String,
}
#[derive(Debug, Deserialize)]
pub struct PrintResponse {
    pub job_started: bool,
}

/// Real-time operational telemetry and thermal status from Moonraker/Klipper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrinterLiveStatus {
    pub state: String,
    pub nozzle_temp_c: f64,
    pub bed_temp_c: f64,
    pub progress: f64,
}

/// Typed Moonraker WebSocket notification event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum PrinterEvent {
    StatusUpdate {
        state: Option<String>,
        progress: Option<f64>,
        nozzle_temp_c: Option<f64>,
        bed_temp_c: Option<f64>,
    },
    GcodeResponse {
        message: String,
    },
    ProcStatUpdate {
        cpu_usage: f64,
        memory_used_kb: u64,
    },
    Unknown {
        method: String,
    },
}

/// A printer instance registered in a Moonraker fleet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetMember {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub tags: Vec<String>,
}

/// Anomaly detected in live printer telemetry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryAnomaly {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub metric_name: String,
    pub observed_value: f64,
}

/// Manages multiple Moonraker machines and provides unified fleet status and anomaly detection.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FleetManager {
    pub members: Vec<FleetMember>,
}

impl FleetManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_member(&mut self, member: FleetMember) {
        self.members.push(member);
    }

    pub fn get_member(&self, id: &str) -> Option<&FleetMember> {
        self.members.iter().find(|m| m.id == id || m.name == id)
    }

    /// Check a printer's telemetry for thermal and process anomalies.
    pub fn detect_anomalies(
        &self,
        telemetry: &PrinterLiveStatus,
        target_nozzle_temp: f64,
        target_bed_temp: f64,
    ) -> Vec<TelemetryAnomaly> {
        let mut anomalies = Vec::new();

        // Check nozzle temperature runaway or under-temp when printing
        if telemetry.state == "printing" && target_nozzle_temp > 0.0 {
            let nozzle_error = (telemetry.nozzle_temp_c - target_nozzle_temp).abs();
            if nozzle_error > 15.0 {
                anomalies.push(TelemetryAnomaly {
                    severity: "critical".into(),
                    code: "NOZZLE_THERMAL_DEVIATION".into(),
                    message: format!(
                        "Nozzle temperature ({:.1}°C) deviates from target ({:.1}°C) by {:.1}°C",
                        telemetry.nozzle_temp_c, target_nozzle_temp, nozzle_error
                    ),
                    metric_name: "nozzle_temp_c".into(),
                    observed_value: telemetry.nozzle_temp_c,
                });
            }
        }

        // Check bed temperature runaway
        if telemetry.state == "printing" && target_bed_temp > 0.0 {
            let bed_error = (telemetry.bed_temp_c - target_bed_temp).abs();
            if bed_error > 10.0 {
                anomalies.push(TelemetryAnomaly {
                    severity: "warning".into(),
                    code: "BED_THERMAL_DEVIATION".into(),
                    message: format!(
                        "Bed temperature ({:.1}°C) deviates from target ({:.1}°C) by {:.1}°C",
                        telemetry.bed_temp_c, target_bed_temp, bed_error
                    ),
                    metric_name: "bed_temp_c".into(),
                    observed_value: telemetry.bed_temp_c,
                });
            }
        }

        anomalies
    }
}

/// Fixed multipart boundary — deterministic for testing; long+unique to avoid g-code collisions.
pub const MULTIPART_BOUNDARY: &str = "dry7c0d3moonrakerboundary8f3a1e9d";

/// Build a `multipart/form-data` body with a single `file` part. ureq has no multipart helper.
pub fn build_multipart(filename: &str, bytes: &[u8]) -> Vec<u8> {
    let header = format!(
        "--{MULTIPART_BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
Content-Type: application/octet-stream\r\n\r\n"
    );
    let footer = format!("\r\n--{MULTIPART_BOUNDARY}--\r\n");
    let mut body = Vec::with_capacity(header.len() + bytes.len() + footer.len());
    body.extend_from_slice(header.as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(footer.as_bytes());
    body
}

/// Join a base URL (trailing `/` trimmed) with an absolute path.
pub fn join_url(base: &str, path: &str) -> String {
    format!("{}{}", base.trim_end_matches('/'), path)
}

fn decode_upload(v: &serde_json::Value) -> Result<UploadResponse, MoonrakerError> {
    v["item"]["path"]
        .as_str()
        .map(|p| UploadResponse {
            filename: p.to_string(),
        })
        .ok_or_else(|| MoonrakerError::Decode(format!("no item.path in upload response: {v}")))
}

fn decode_print_start(v: &serde_json::Value) -> Result<PrintResponse, MoonrakerError> {
    if let Some(job_started) = v
        .get("result")
        .or_else(|| v.get("ok"))
        .or_else(|| v.get("job_started"))
        .and_then(serde_json::Value::as_bool)
    {
        return if job_started {
            Ok(PrintResponse { job_started })
        } else {
            Err(MoonrakerError::Rejected(format!(
                "print start response reported failure: {v}"
            )))
        };
    }

    if let Some(status) = v
        .get("result")
        .or_else(|| v.get("status"))
        .and_then(serde_json::Value::as_str)
    {
        return if matches!(
            status.to_ascii_lowercase().as_str(),
            "ok" | "success" | "started" | "starting" | "print_started"
        ) {
            Ok(PrintResponse { job_started: true })
        } else {
            Err(MoonrakerError::Rejected(format!(
                "print start returned status {status:?}"
            )))
        };
    }

    if let Some(error) = v.get("error") {
        let detail = error
            .get("message")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| error.to_string());
        return Err(MoonrakerError::Rejected(detail));
    }

    if v.is_object() {
        return Err(MoonrakerError::Decode(format!(
            "unexpected Moonraker print start response object: {v}"
        )));
    }
    Err(MoonrakerError::Decode(format!(
        "unexpected Moonraker print start response: {v}"
    )))
}

fn post(
    cfg: &MoonrakerConfig,
    path: &str,
    content_type: &str,
    body: &[u8],
) -> Result<serde_json::Value, MoonrakerError> {
    let mut req = ureq::post(&join_url(&cfg.base_url, path))
        .timeout(cfg.timeout)
        .set("Content-Type", content_type);
    if let Some(k) = &cfg.api_key {
        req = req.set("X-Api-Key", k);
    }
    match req.send_bytes(body) {
        Ok(r) => {
            let bytes = read_response_limited(r.into_reader(), MAX_JSON_RESPONSE_BYTES)?;
            serde_json::from_slice(&bytes)
                .map_err(|e| MoonrakerError::Decode(format!("invalid JSON: {e}")))
        }
        Err(ureq::Error::Status(code, r)) => {
            let mut bytes = Vec::new();
            r.into_reader()
                .take(MAX_ERROR_SNIPPET_BYTES)
                .read_to_end(&mut bytes)
                .map_err(|error| MoonrakerError::Transport(error.to_string()))?;
            Err(MoonrakerError::Http(
                code,
                String::from_utf8_lossy(&bytes).into_owned(),
            ))
        }
        Err(ureq::Error::Transport(t)) => Err(MoonrakerError::Transport(t.to_string())),
    }
}

fn get(cfg: &MoonrakerConfig, path: &str) -> Result<serde_json::Value, MoonrakerError> {
    let mut req = ureq::get(&join_url(&cfg.base_url, path)).timeout(cfg.timeout);
    if let Some(k) = &cfg.api_key {
        req = req.set("X-Api-Key", k);
    }
    match req.call() {
        Ok(r) => {
            let bytes = read_response_limited(r.into_reader(), MAX_JSON_RESPONSE_BYTES)?;
            serde_json::from_slice(&bytes)
                .map_err(|e| MoonrakerError::Decode(format!("invalid JSON: {e}")))
        }
        Err(ureq::Error::Status(code, r)) => {
            let mut bytes = Vec::new();
            r.into_reader()
                .take(MAX_ERROR_SNIPPET_BYTES)
                .read_to_end(&mut bytes)
                .map_err(|error| MoonrakerError::Transport(error.to_string()))?;
            Err(MoonrakerError::Http(
                code,
                String::from_utf8_lossy(&bytes).into_owned(),
            ))
        }
        Err(ureq::Error::Transport(t)) => Err(MoonrakerError::Transport(t.to_string())),
    }
}

fn read_response_limited(reader: impl Read, limit: usize) -> Result<Vec<u8>, MoonrakerError> {
    let mut bytes = Vec::new();
    reader
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| MoonrakerError::Transport(error.to_string()))?;
    if bytes.len() > limit {
        return Err(MoonrakerError::Decode(format!(
            "response exceeds {limit} bytes"
        )));
    }
    Ok(bytes)
}

fn validate_filename(filename: &str) -> Result<(), MoonrakerError> {
    if filename.is_empty() {
        return Err(MoonrakerError::InvalidInput(
            "upload filename must not be empty".into(),
        ));
    }
    if filename
        .chars()
        .any(|character| matches!(character, '\r' | '\n' | '"'))
    {
        return Err(MoonrakerError::InvalidInput(
            "upload filename contains an unsafe multipart header character".into(),
        ));
    }
    Ok(())
}

/// POST the g-code to `/server/files/upload` as multipart/form-data. Network.
pub fn upload_file(
    cfg: &MoonrakerConfig,
    filename: &str,
    bytes: &[u8],
) -> Result<UploadResponse, MoonrakerError> {
    validate_filename(filename)?;
    let body = build_multipart(filename, bytes);
    let ct = format!("multipart/form-data; boundary={MULTIPART_BOUNDARY}");
    decode_upload(&post(cfg, "/server/files/upload", &ct, &body)?)
}

/// Start a print of an already-uploaded file via `/printer/print/start`. Network.
pub fn start_print(cfg: &MoonrakerConfig, filename: &str) -> Result<PrintResponse, MoonrakerError> {
    let body = serde_json::json!({ "filename": filename }).to_string();
    let response = post(
        cfg,
        "/printer/print/start",
        "application/json",
        body.as_bytes(),
    )?;
    decode_print_start(&response)
}

/// Query the live operational and thermal status of the printer from Moonraker. Network.
pub fn get_printer_status(cfg: &MoonrakerConfig) -> Result<PrinterLiveStatus, MoonrakerError> {
    let response = get(
        cfg,
        "/printer/objects/query?print_stats&extruder&heater_bed",
    )?;
    decode_printer_status(&response)
}

/// Send live dynamic G-code script / pressure advance tuning command to Klipper via Moonraker. Network.
pub fn set_pressure_advance(cfg: &MoonrakerConfig, advance: f64) -> Result<bool, MoonrakerError> {
    if !advance.is_finite() || advance < 0.0 {
        return Err(MoonrakerError::InvalidInput(
            "pressure advance must be non-negative and finite".into(),
        ));
    }
    let script = format!("SET_PRESSURE_ADVANCE ADVANCE={advance:.5}");
    let body = serde_json::json!({ "script": script }).to_string();
    let response = post(
        cfg,
        "/printer/gcode/script",
        "application/json",
        body.as_bytes(),
    )?;
    if response.get("error").is_some() {
        let msg = response["error"]["message"]
            .as_str()
            .unwrap_or("command rejected");
        return Err(MoonrakerError::Rejected(msg.to_string()));
    }
    Ok(true)
}

fn decode_printer_status(v: &serde_json::Value) -> Result<PrinterLiveStatus, MoonrakerError> {
    let status = v
        .get("result")
        .and_then(|r| r.get("status"))
        .ok_or_else(|| {
            MoonrakerError::Decode("missing result.status in Moonraker response".into())
        })?;

    let state = status
        .get("print_stats")
        .and_then(|ps| ps.get("state"))
        .and_then(|s| s.as_str())
        .unwrap_or("standby")
        .to_string();

    let progress = status
        .get("print_stats")
        .and_then(|ps| ps.get("progress"))
        .and_then(|p| p.as_f64())
        .unwrap_or(0.0);

    let nozzle_temp_c = status
        .get("extruder")
        .and_then(|ex| ex.get("temperature"))
        .and_then(|t| t.as_f64())
        .unwrap_or(0.0);

    let bed_temp_c = status
        .get("heater_bed")
        .and_then(|hb| hb.get("temperature"))
        .and_then(|t| t.as_f64())
        .unwrap_or(0.0);

    Ok(PrinterLiveStatus {
        state,
        nozzle_temp_c,
        bed_temp_c,
        progress,
    })
}

/// Parse a Moonraker WebSocket JSON-RPC notification frame.
pub fn parse_websocket_event(json_text: &str) -> Result<PrinterEvent, MoonrakerError> {
    let v: serde_json::Value = serde_json::from_str(json_text)
        .map_err(|e| MoonrakerError::Decode(format!("invalid json event: {e}")))?;

    let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = v.get("params").and_then(|p| p.as_array());

    match method {
        "notify_status_update" => {
            if let Some(first_param) = params.and_then(|arr| arr.first()) {
                let state = first_param
                    .get("print_stats")
                    .and_then(|ps| ps.get("state"))
                    .and_then(|s| s.as_str())
                    .map(str::to_string);
                let progress = first_param
                    .get("display_status")
                    .and_then(|ds| ds.get("progress"))
                    .and_then(|p| p.as_f64());
                let nozzle_temp_c = first_param
                    .get("extruder")
                    .and_then(|e| e.get("temperature"))
                    .and_then(|t| t.as_f64());
                let bed_temp_c = first_param
                    .get("heater_bed")
                    .and_then(|b| b.get("temperature"))
                    .and_then(|t| t.as_f64());

                Ok(PrinterEvent::StatusUpdate {
                    state,
                    progress,
                    nozzle_temp_c,
                    bed_temp_c,
                })
            } else {
                Ok(PrinterEvent::Unknown {
                    method: method.to_string(),
                })
            }
        }
        "notify_gcode_response" => {
            let msg = params
                .and_then(|arr| arr.first())
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            Ok(PrinterEvent::GcodeResponse { message: msg })
        }
        "notify_proc_stat_update" => {
            let cpu = params
                .and_then(|arr| arr.first())
                .and_then(|s| s.get("system_cpu_usage"))
                .and_then(|c| c.as_f64())
                .unwrap_or(0.0);
            let mem = params
                .and_then(|arr| arr.first())
                .and_then(|s| s.get("system_memory"))
                .and_then(|m| m.get("used"))
                .and_then(|u| u.as_u64())
                .unwrap_or(0);
            Ok(PrinterEvent::ProcStatUpdate {
                cpu_usage: cpu,
                memory_used_kb: mem,
            })
        }
        _ => Ok(PrinterEvent::Unknown {
            method: method.to_string(),
        }),
    }
}

/// Emergency stop trigger (M112 shutdown via Moonraker `/printer/emergency_stop`). Network.
pub fn emergency_stop(cfg: &MoonrakerConfig) -> Result<bool, MoonrakerError> {
    let response = post(cfg, "/printer/emergency_stop", "application/json", b"{}")?;
    if response.get("error").is_some() {
        let msg = response["error"]["message"]
            .as_str()
            .unwrap_or("emergency stop rejected");
        return Err(MoonrakerError::Rejected(msg.to_string()));
    }
    Ok(true)
}

/// Pause the currently active print job via `/printer/print/pause`. Network.
pub fn pause_print(cfg: &MoonrakerConfig) -> Result<bool, MoonrakerError> {
    let response = post(cfg, "/printer/print/pause", "application/json", b"{}")?;
    if response.get("error").is_some() {
        let msg = response["error"]["message"]
            .as_str()
            .unwrap_or("pause rejected");
        return Err(MoonrakerError::Rejected(msg.to_string()));
    }
    Ok(true)
}

/// Resume a paused print job via `/printer/print/resume`. Network.
pub fn resume_print(cfg: &MoonrakerConfig) -> Result<bool, MoonrakerError> {
    let response = post(cfg, "/printer/print/resume", "application/json", b"{}")?;
    if response.get("error").is_some() {
        let msg = response["error"]["message"]
            .as_str()
            .unwrap_or("resume rejected");
        return Err(MoonrakerError::Rejected(msg.to_string()));
    }
    Ok(true)
}

/// Cancel the active print job via `/printer/print/cancel`. Network.
pub fn cancel_print(cfg: &MoonrakerConfig) -> Result<bool, MoonrakerError> {
    let response = post(cfg, "/printer/print/cancel", "application/json", b"{}")?;
    if response.get("error").is_some() {
        let msg = response["error"]["message"]
            .as_str()
            .unwrap_or("cancel rejected");
        return Err(MoonrakerError::Rejected(msg.to_string()));
    }
    Ok(true)
}

/// Build a JSON-RPC 2.0 message for subscribing to printer objects over WebSocket.
pub fn subscribe_objects_rpc(request_id: u64, objects: &[&str]) -> String {
    let mut obj_map = serde_json::Map::new();
    for obj in objects {
        obj_map.insert(obj.to_string(), serde_json::Value::Null);
    }
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "printer.objects.subscribe",
        "params": {
            "objects": obj_map
        },
        "id": request_id
    })
    .to_string()
}

/// Stream a chunk of G-code commands to Moonraker `/printer/gcode/script`. Network.
pub fn stream_gcode_chunk(
    cfg: &MoonrakerConfig,
    gcode_lines: &[String],
) -> Result<usize, MoonrakerError> {
    if gcode_lines.is_empty() {
        return Ok(0);
    }
    let script = gcode_lines.join("\n");
    let body = serde_json::json!({ "script": script }).to_string();
    let response = post(
        cfg,
        "/printer/gcode/script",
        "application/json",
        body.as_bytes(),
    )?;
    if response.get("error").is_some() {
        let msg = response["error"]["message"]
            .as_str()
            .unwrap_or("stream chunk rejected");
        return Err(MoonrakerError::Rejected(msg.to_string()));
    }
    Ok(gcode_lines.len())
}

/// Dynamic closed-loop pressure advance calculation based on nozzle temperature compensation and print speed.
pub fn calculate_auto_tuned_pressure_advance(
    base_advance: f64,
    nozzle_temp_c: f64,
    target_temp_c: f64,
    speed_factor: f64,
) -> f64 {
    let mut advance = base_advance;
    if target_temp_c > 100.0 && nozzle_temp_c > 100.0 {
        let temp_delta = nozzle_temp_c - target_temp_c;
        let temp_scale = (1.0 - temp_delta * 0.005).clamp(0.7, 1.3);
        advance *= temp_scale;
    }
    let speed_scale = speed_factor.clamp(0.5, 2.0);
    (advance * speed_scale).clamp(0.0, 0.2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn serve_once(
        response_body: &'static str,
    ) -> (MoonrakerConfig, std::thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut content_length = None;
            loop {
                let mut chunk = [0u8; 4096];
                let read = socket.read(&mut chunk).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                if content_length.is_none() {
                    if let Some(header_end) =
                        request.windows(4).position(|window| window == b"\r\n\r\n")
                    {
                        let headers = String::from_utf8_lossy(&request[..header_end]);
                        content_length = headers.lines().find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        });
                    }
                }
                if let Some(header_end) =
                    request.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    if request.len() >= header_end + 4 + content_length.unwrap_or(0) {
                        break;
                    }
                }
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            socket.write_all(response.as_bytes()).unwrap();
            request
        });
        (
            MoonrakerConfig {
                base_url: format!("http://{address}"),
                api_key: Some("test-key".into()),
                timeout: Duration::from_secs(2),
            },
            handle,
        )
    }

    #[test]
    fn multipart_wraps_the_file_part() {
        let body = build_multipart("part.gcode", b"G1 X0\n");
        let s = String::from_utf8_lossy(&body);
        assert!(s.contains(&format!("--{MULTIPART_BOUNDARY}")));
        assert!(s.contains(r#"Content-Disposition: form-data; name="file"; filename="part.gcode""#));
        assert!(s.contains("application/octet-stream"));
        assert!(s.contains("G1 X0"));
        assert!(s.trim_end().ends_with(&format!("--{MULTIPART_BOUNDARY}--")));
    }
    #[test]
    fn join_url_trims_trailing_slash() {
        assert_eq!(
            join_url("http://voron.local/", "/server/files/upload"),
            "http://voron.local/server/files/upload"
        );
        assert_eq!(
            join_url("http://voron.local", "/server/files/upload"),
            "http://voron.local/server/files/upload"
        );
    }
    #[test]
    fn upload_response_decodes_filename() {
        // Moonraker upload returns { "item": { "path": "part.gcode", ... }, ... }
        let v: serde_json::Value =
            serde_json::from_str(r#"{"item":{"path":"part.gcode"}}"#).unwrap();
        assert_eq!(decode_upload(&v).unwrap().filename, "part.gcode");
    }
    #[test]
    fn upload_file_sends_authenticated_multipart_and_decodes_response() {
        let (cfg, server) = serve_once(r#"{"item":{"path":"gcodes/part.gcode"}}"#);
        let response = upload_file(&cfg, "part.gcode", b"G1 X1\n").unwrap();
        assert_eq!(response.filename, "gcodes/part.gcode");

        let request = String::from_utf8_lossy(&server.join().unwrap()).into_owned();
        assert!(request.starts_with("POST /server/files/upload HTTP/1.1"));
        assert!(request.to_ascii_lowercase().contains("x-api-key: test-key"));
        assert!(request.contains(r#"filename="part.gcode""#));
        assert!(request.ends_with(&format!("--{MULTIPART_BOUNDARY}--\r\n")));
    }
    #[test]
    fn missing_path_is_decode_error() {
        let v: serde_json::Value = serde_json::from_str(r#"{"item":{}}"#).unwrap();
        assert!(matches!(decode_upload(&v), Err(MoonrakerError::Decode(_))));
    }
    #[test]
    fn start_print_decodes_ok_response() {
        let v: serde_json::Value = serde_json::from_str(r#"{"result":"ok"}"#).unwrap();
        assert!(decode_print_start(&v).is_ok_and(|response| response.job_started));
    }
    #[test]
    fn start_print_false_is_error() {
        let v: serde_json::Value = serde_json::from_str(r#"{"result":false}"#).unwrap();
        assert!(matches!(
            decode_print_start(&v),
            Err(MoonrakerError::Rejected(_))
        ));
    }
    #[test]
    fn start_print_error_key_is_rejected() {
        let v: serde_json::Value =
            serde_json::from_str(r#"{"error":{"message":"already printing"}}"#).unwrap();
        assert!(matches!(
            decode_print_start(&v),
            Err(MoonrakerError::Rejected(message)) if message == "already printing"
        ));
    }
    #[test]
    fn unsafe_multipart_filename_is_rejected() {
        assert!(matches!(
            validate_filename("safe.gcode\"\r\nX-Evil: yes"),
            Err(MoonrakerError::InvalidInput(_))
        ));
    }
    #[test]
    fn oversized_response_is_rejected() {
        let bytes = vec![b'x'; 17];
        assert!(matches!(
            read_response_limited(std::io::Cursor::new(bytes), 16),
            Err(MoonrakerError::Decode(message)) if message.contains("exceeds 16 bytes")
        ));
    }
    #[test]
    fn decode_printer_status_parses_temperatures_and_state() {
        let json = serde_json::json!({
            "result": {
                "status": {
                    "print_stats": {
                        "state": "printing",
                        "progress": 0.42
                    },
                    "extruder": {
                        "temperature": 215.5
                    },
                    "heater_bed": {
                        "temperature": 60.0
                    }
                }
            }
        });
        let status = decode_printer_status(&json).unwrap();
        assert_eq!(status.state, "printing");
        assert_eq!(status.progress, 0.42);
        assert_eq!(status.nozzle_temp_c, 215.5);
        assert_eq!(status.bed_temp_c, 60.0);
    }
    #[test]
    fn invalid_pressure_advance_is_rejected() {
        let cfg = MoonrakerConfig {
            base_url: "http://localhost:7125".into(),
            api_key: None,
            timeout: Duration::from_secs(1),
        };
        assert!(matches!(
            set_pressure_advance(&cfg, -0.5),
            Err(MoonrakerError::InvalidInput(_))
        ));
        assert!(matches!(
            set_pressure_advance(&cfg, f64::NAN),
            Err(MoonrakerError::InvalidInput(_))
        ));
    }
    #[test]
    fn test_parse_websocket_event() {
        let event_json = r#"{
            "jsonrpc": "2.0",
            "method": "notify_status_update",
            "params": [
                {
                    "print_stats": { "state": "printing" },
                    "display_status": { "progress": 0.75 },
                    "extruder": { "temperature": 220.0 },
                    "heater_bed": { "temperature": 65.0 }
                },
                1725000000.0
            ]
        }"#;
        let parsed =
            parse_websocket_event(event_json).expect("should parse websocket status event");
        match parsed {
            PrinterEvent::StatusUpdate {
                state,
                progress,
                nozzle_temp_c,
                bed_temp_c,
            } => {
                assert_eq!(state, Some("printing".to_string()));
                assert_eq!(progress, Some(0.75));
                assert_eq!(nozzle_temp_c, Some(220.0));
                assert_eq!(bed_temp_c, Some(65.0));
            }
            _ => panic!("Expected StatusUpdate event"),
        }
    }
    #[test]
    fn test_calculate_auto_tuned_pressure_advance() {
        // Base advance 0.040 at nominal temp 210 -> tuned advance = 0.040
        let nominal = calculate_auto_tuned_pressure_advance(0.040, 210.0, 210.0, 1.0);
        assert!((nominal - 0.040).abs() < 1e-5);

        // 1.5x speed factor increases advance
        let high_speed = calculate_auto_tuned_pressure_advance(0.040, 210.0, 210.0, 1.5);
        assert!((high_speed - 0.060).abs() < 1e-5);

        // Overheated nozzle (230°C vs 210°C target) lowers advance to reduce ooze
        let hot = calculate_auto_tuned_pressure_advance(0.040, 230.0, 210.0, 1.0);
        assert!(hot < nominal);
    }
    #[test]
    fn test_subscribe_objects_rpc() {
        let rpc = subscribe_objects_rpc(42, &["toolhead", "extruder", "heater_bed"]);
        assert!(rpc.contains("\"id\":42"));
        assert!(rpc.contains("\"method\":\"printer.objects.subscribe\""));
        assert!(rpc.contains("\"toolhead\":null"));
        assert!(rpc.contains("\"extruder\":null"));
    }

    #[test]
    fn test_fleet_manager_anomaly_detection() {
        let mut fleet = FleetManager::new();
        fleet.add_member(FleetMember {
            id: "printer-01".into(),
            name: "Voron 2.4".into(),
            base_url: "http://192.168.1.100".into(),
            api_key: None,
            tags: vec!["abs".into(), "enclosed".into()],
        });

        assert_eq!(fleet.members.len(), 1);
        assert!(fleet.get_member("printer-01").is_some());

        let nominal_status = PrinterLiveStatus {
            state: "printing".into(),
            nozzle_temp_c: 245.0,
            bed_temp_c: 100.0,
            progress: 0.50,
        };

        // Nominal temperature matches target -> 0 anomalies
        let anomalies_ok = fleet.detect_anomalies(&nominal_status, 245.0, 100.0);
        assert!(anomalies_ok.is_empty());

        // Severe temperature drop (thermal runaway risk) -> anomaly flagged
        let runaway_status = PrinterLiveStatus {
            state: "printing".into(),
            nozzle_temp_c: 210.0,
            bed_temp_c: 100.0,
            progress: 0.50,
        };
        let anomalies_bad = fleet.detect_anomalies(&runaway_status, 245.0, 100.0);
        assert_eq!(anomalies_bad.len(), 1);
        assert_eq!(anomalies_bad[0].code, "NOZZLE_THERMAL_DEVIATION");
        assert_eq!(anomalies_bad[0].severity, "critical");
    }
}

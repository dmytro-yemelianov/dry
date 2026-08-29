//! `dry-moonraker` — Dry's Moonraker upload client. The only network code for the upload feature;
//! mirrors `dry-llm`'s feature-gated, ureq-based structure. `dry-core` stays pure.

use serde::Deserialize;
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
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PrinterLiveStatus {
    pub state: String,
    pub nozzle_temp_c: f64,
    pub bed_temp_c: f64,
    pub progress: f64,
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

fn get(
    cfg: &MoonrakerConfig,
    path: &str,
) -> Result<serde_json::Value, MoonrakerError> {
    let mut req = ureq::get(&join_url(&cfg.base_url, path))
        .timeout(cfg.timeout);
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
        .ok_or_else(|| MoonrakerError::Decode("missing result.status in Moonraker response".into()))?;

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
}

//! `dry-moonraker` — Dry's Moonraker upload client. The only network code for the upload feature;
//! mirrors `dry-llm`'s feature-gated, ureq-based structure. `dry-core` stays pure.

use serde::Deserialize;

/// Connection to a Moonraker host. `api_key` is sent as `X-Api-Key` when present.
pub struct MoonrakerConfig {
    pub base_url: String,
    pub api_key: Option<String>,
}

#[derive(Debug)]
pub enum MoonrakerError {
    Http(u16, String),
    Transport(String),
    Decode(String),
}
impl std::fmt::Display for MoonrakerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoonrakerError::Http(c, b) => write!(f, "Moonraker returned HTTP {c}: {b}"),
            MoonrakerError::Transport(m) => write!(f, "network error reaching Moonraker: {m}"),
            MoonrakerError::Decode(m) => write!(f, "could not parse Moonraker response: {m}"),
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

fn post(
    cfg: &MoonrakerConfig,
    path: &str,
    content_type: &str,
    body: &[u8],
) -> Result<serde_json::Value, MoonrakerError> {
    let mut req = ureq::post(&join_url(&cfg.base_url, path)).set("Content-Type", content_type);
    if let Some(k) = &cfg.api_key {
        req = req.set("X-Api-Key", k);
    }
    match req.send_bytes(body) {
        Ok(r) => r
            .into_json()
            .map_err(|e| MoonrakerError::Decode(format!("invalid JSON: {e}"))),
        Err(ureq::Error::Status(code, r)) => Err(MoonrakerError::Http(
            code,
            r.into_string()
                .unwrap_or_default()
                .chars()
                .take(500)
                .collect(),
        )),
        Err(ureq::Error::Transport(t)) => Err(MoonrakerError::Transport(t.to_string())),
    }
}

/// POST the g-code to `/server/files/upload` as multipart/form-data. Network.
pub fn upload_file(
    cfg: &MoonrakerConfig,
    filename: &str,
    bytes: &[u8],
) -> Result<UploadResponse, MoonrakerError> {
    let body = build_multipart(filename, bytes);
    let ct = format!("multipart/form-data; boundary={MULTIPART_BOUNDARY}");
    decode_upload(&post(cfg, "/server/files/upload", &ct, &body)?)
}

/// Start a print of an already-uploaded file via `/printer/print/start`. Network.
pub fn start_print(cfg: &MoonrakerConfig, filename: &str) -> Result<PrintResponse, MoonrakerError> {
    let body = serde_json::json!({ "filename": filename }).to_string();
    let _ = post(
        cfg,
        "/printer/print/start",
        "application/json",
        body.as_bytes(),
    )?;
    Ok(PrintResponse { job_started: true })
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn missing_path_is_decode_error() {
        let v: serde_json::Value = serde_json::from_str(r#"{"item":{}}"#).unwrap();
        assert!(matches!(decode_upload(&v), Err(MoonrakerError::Decode(_))));
    }
}

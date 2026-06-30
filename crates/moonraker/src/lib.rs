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
}

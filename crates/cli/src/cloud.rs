use serde_json::Value;
use std::env;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

pub const DEFAULT_CLOUD_URL: &str = "https://cloud.dry.yemelianov.dev";

const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const NOT_LOGGED_IN: &str = "not logged in — run `dry auth login`";
const MAX_JOB_WAIT: Duration = Duration::from_secs(10 * 60);

#[derive(Debug)]
pub struct CloudError {
    message: String,
}

impl CloudError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn not_logged_in() -> Self {
        Self::new(NOT_LOGGED_IN)
    }
}

impl fmt::Display for CloudError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CloudError {}

pub fn resolve_cloud_url(cli_value: Option<&str>) -> String {
    cli_value
        .map(str::to_owned)
        .or_else(|| {
            env::var("DRY_CLOUD_URL")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| DEFAULT_CLOUD_URL.to_owned())
        .trim_end_matches('/')
        .to_owned()
}

pub fn login(base_url: &str) -> Result<(), CloudError> {
    let response = ureq::post(&endpoint(base_url, "/v1/auth/device"))
        .timeout(Duration::from_secs(30))
        .call()
        .map_err(|error| request_error("device authorization failed", error))?;
    let device = response_json(response, "device authorization")?;
    let device_code = required_string(&device, "device_code", "device authorization")?;
    let user_code = required_string(&device, "user_code", "device authorization")?;
    let verification_uri_complete =
        required_string(&device, "verification_uri_complete", "device authorization")?;
    let expires_in = required_u64(&device, "expires_in", "device authorization")?;
    let mut interval = required_u64(&device, "interval", "device authorization")?;

    println!("Code: {user_code}");
    println!("Open: {verification_uri_complete}");
    std::io::stdout()
        .flush()
        .map_err(|error| CloudError::new(format!("cannot write login instructions: {error}")))?;

    let deadline = Instant::now() + Duration::from_secs(expires_in);
    loop {
        if interval > 0 {
            let sleep_for = Duration::from_secs(interval);
            if Instant::now() + sleep_for >= deadline {
                return Err(CloudError::new("device authorization expired"));
            }
            thread::sleep(sleep_for);
        }

        match poll_token(base_url, &device_code)? {
            TokenPoll::Granted(token) => {
                store_token(&token)?;
                println!("Logged in.");
                return Ok(());
            }
            TokenPoll::Pending => {}
            TokenPoll::SlowDown => interval = interval.saturating_add(5),
            TokenPoll::Expired => return Err(CloudError::new("device authorization expired")),
        }

        if Instant::now() >= deadline {
            return Err(CloudError::new("device authorization expired"));
        }
    }
}

pub fn status(base_url: &str) -> Result<(), CloudError> {
    let token = load_token()?;
    let me = get_authed_json(base_url, "/v1/me", &token, "account status")?;
    let usage = get_authed_json(base_url, "/v1/usage", &token, "usage status")?;

    let account_id = required_string(&me, "account_id", "account status")?;
    let email = required_string(&me, "email", "account status")?;
    let jobs = usage
        .pointer("/month/jobs")
        .and_then(Value::as_u64)
        .ok_or_else(|| CloudError::new("usage status response omitted month.jobs"))?;
    let bytes = usage
        .pointer("/month/bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| CloudError::new("usage status response omitted month.bytes"))?;
    let jobs_quota = usage
        .pointer("/quotas/jobs_per_month")
        .and_then(Value::as_u64)
        .ok_or_else(|| CloudError::new("usage status response omitted quotas.jobs_per_month"))?;

    println!("{email} ({account_id}) — {jobs}/{jobs_quota} jobs, {bytes} bytes this month");
    Ok(())
}

pub fn logout() -> Result<(), CloudError> {
    let path = token_path()?;
    match fs::remove_file(&path) {
        Ok(()) => println!("Logged out."),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            println!("Already logged out.")
        }
        Err(error) => {
            return Err(CloudError::new(format!(
                "cannot remove {}: {error}",
                path.display()
            )))
        }
    }
    if env::var("DRY_TOKEN").is_ok_and(|token| !token.trim().is_empty()) {
        eprintln!("warning: DRY_TOKEN is still set and takes precedence");
    }
    Ok(())
}

pub fn verify(
    base_url: &str,
    file: &str,
    printer: &str,
    pack_version: Option<&str>,
    json: bool,
) -> Result<ExitCode, CloudError> {
    let token = load_token()?;
    let gcode =
        fs::read(file).map_err(|error| CloudError::new(format!("cannot read {file}: {error}")))?;

    let auth = format!("Bearer {token}");
    let submit_url = endpoint(base_url, "/v1/jobs/verify");
    let mut request = ureq::post(&submit_url)
        .timeout(Duration::from_secs(120))
        .set("Authorization", &auth)
        .set("Content-Type", "text/plain")
        .query("pack", printer);
    if let Some(version) = pack_version {
        request = request.query("version", version);
    }
    let response = request
        .send_bytes(&gcode)
        .map_err(|error| request_error("job submission failed", error))?;
    let submitted = response_json(response, "job submission")?;
    let id = required_string(&submitted, "id", "job submission")?;
    let status_path = required_string(&submitted, "status_url", "job submission")?;
    if !status_path.starts_with('/') {
        return Err(CloudError::new(
            "job submission returned a non-relative status_url",
        ));
    }
    let status_url = endpoint(base_url, &status_path);

    let deadline = Instant::now() + MAX_JOB_WAIT;
    let mut delay = Duration::from_secs(1);
    let report = loop {
        let job = get_authed_url_json(&status_url, &token, "job status")?;
        match job.get("status").and_then(Value::as_str) {
            Some("done") => {
                break job
                    .get("report")
                    .cloned()
                    .ok_or_else(|| CloudError::new("completed job omitted report"))?
            }
            Some("error") => {
                let message = job
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("cloud verification failed");
                let stage = job.get("stage").and_then(Value::as_str);
                return Err(CloudError::new(match stage {
                    Some(stage) => format!("cloud verification failed at {stage}: {message}"),
                    None => format!("cloud verification failed: {message}"),
                }));
            }
            Some("queued" | "running") => {}
            Some(status) => {
                return Err(CloudError::new(format!(
                    "job status response contained unknown status {status:?}"
                )))
            }
            None => return Err(CloudError::new("job status response omitted status")),
        }

        if Instant::now() + delay >= deadline {
            return Err(CloudError::new(format!(
                "cloud verification timed out after {} seconds",
                MAX_JOB_WAIT.as_secs()
            )));
        }
        thread::sleep(delay);
        delay = std::cmp::min(delay.saturating_mul(2), Duration::from_secs(5));
    };

    let findings = report
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| CloudError::new("verification report omitted findings"))?;
    let error_count = findings
        .iter()
        .filter(|finding| finding.get("severity").and_then(Value::as_str) == Some("error"))
        .count();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|error| CloudError::new(format!("cannot encode report: {error}")))?
        );
    } else {
        let verdict = if error_count == 0 { "OK" } else { "FAILED" };
        println!(
            "cloud verify: {file} — {verdict} ({} finding(s), {error_count} error(s))",
            findings.len()
        );
        println!("job: {status_url} ({id})");
    }

    Ok(if error_count == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

enum TokenPoll {
    Granted(String),
    Pending,
    SlowDown,
    Expired,
}

fn poll_token(base_url: &str, device_code: &str) -> Result<TokenPoll, CloudError> {
    let result = ureq::post(&endpoint(base_url, "/v1/auth/token"))
        .timeout(Duration::from_secs(30))
        .send_form(&[("grant_type", DEVICE_GRANT), ("device_code", device_code)]);

    let payload = match result {
        Ok(response) => {
            let payload = response_json(response, "token exchange")?;
            let token = required_string(&payload, "access_token", "token exchange")?;
            return Ok(TokenPoll::Granted(token));
        }
        Err(ureq::Error::Status(_, response)) => response_json(response, "token exchange")?,
        Err(error) => return Err(request_error("token exchange failed", error)),
    };

    match payload.get("error").and_then(Value::as_str) {
        Some("authorization_pending") => Ok(TokenPoll::Pending),
        Some("slow_down") => Ok(TokenPoll::SlowDown),
        Some("expired_token") => Ok(TokenPoll::Expired),
        Some(error) => Err(CloudError::new(format!("token exchange failed: {error}"))),
        None => Err(CloudError::new(
            "token exchange error response omitted error",
        )),
    }
}

fn load_token() -> Result<String, CloudError> {
    if let Ok(token) = env::var("DRY_TOKEN") {
        let token = token.trim();
        if !token.is_empty() {
            return Ok(token.to_owned());
        }
    }

    let path = token_path()?;
    match fs::read_to_string(&path) {
        Ok(token) if !token.trim().is_empty() => Ok(token.trim().to_owned()),
        Ok(_) => Err(CloudError::not_logged_in()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(CloudError::not_logged_in())
        }
        Err(error) => Err(CloudError::new(format!(
            "cannot read {}: {error}",
            path.display()
        ))),
    }
}

fn store_token(token: &str) -> Result<(), CloudError> {
    let path = token_path()?;
    let parent = path
        .parent()
        .ok_or_else(|| CloudError::new("cloud token path has no parent directory"))?;
    fs::create_dir_all(parent)
        .map_err(|error| CloudError::new(format!("cannot create {}: {error}", parent.display())))?;

    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .map_err(|error| CloudError::new(format!("cannot write {}: {error}", path.display())))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| {
                CloudError::new(format!(
                    "cannot secure token file {}: {error}",
                    path.display()
                ))
            })?;
    }
    writeln!(file, "{token}")
        .map_err(|error| CloudError::new(format!("cannot write {}: {error}", path.display())))
}

fn token_path() -> Result<PathBuf, CloudError> {
    if let Ok(path) = env::var("XDG_CONFIG_HOME") {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path).join("dry").join("cloud-token"));
        }
    }
    dirs::config_dir()
        .map(|path| path.join("dry").join("cloud-token"))
        .ok_or_else(|| CloudError::new("cannot determine the user configuration directory"))
}

fn get_authed_json(
    base_url: &str,
    path: &str,
    token: &str,
    context: &str,
) -> Result<Value, CloudError> {
    get_authed_url_json(&endpoint(base_url, path), token, context)
}

fn get_authed_url_json(url: &str, token: &str, context: &str) -> Result<Value, CloudError> {
    let auth = format!("Bearer {token}");
    let response = ureq::get(url)
        .timeout(Duration::from_secs(30))
        .set("Authorization", &auth)
        .call()
        .map_err(|error| request_error(&format!("{context} failed"), error))?;
    response_json(response, context)
}

fn endpoint(base_url: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

fn response_json(response: ureq::Response, context: &str) -> Result<Value, CloudError> {
    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|error| CloudError::new(format!("cannot read {context} response: {error}")))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| CloudError::new(format!("{context} returned invalid JSON: {error}")))
}

fn request_error(context: &str, error: ureq::Error) -> CloudError {
    match error {
        ureq::Error::Status(401, _) => CloudError::not_logged_in(),
        ureq::Error::Status(status, response) => {
            let detail = response_json(response, context).ok().and_then(|body| {
                body.get("detail")
                    .or_else(|| body.get("error"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
            match detail {
                Some(detail) => CloudError::new(format!("{context}: HTTP {status} ({detail})")),
                None => CloudError::new(format!("{context}: HTTP {status}")),
            }
        }
        ureq::Error::Transport(error) => CloudError::new(format!("{context}: {error}")),
    }
}

fn required_string(value: &Value, field: &str, context: &str) -> Result<String, CloudError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| CloudError::new(format!("{context} response omitted {field}")))
}

fn required_u64(value: &Value, field: &str, context: &str) -> Result<u64, CloudError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| CloudError::new(format!("{context} response omitted {field}")))
}

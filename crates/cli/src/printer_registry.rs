use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fmt;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

pub const DEFAULT_REGISTRY_URL: &str = "https://api.dry.yemelianov.dev";
const MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug)]
pub struct RegistryError(String);

impl RegistryError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Default)]
pub struct SearchFilter {
    pub text: Option<String>,
    pub vendor: Vec<String>,
    pub firmware: Vec<String>,
    pub kinematics: Vec<String>,
    pub material: Vec<String>,
    pub nozzle_diameter_mm: Option<f64>,
    pub build_x_mm: Option<f64>,
    pub build_y_mm: Option<f64>,
    pub build_z_mm: Option<f64>,
    pub macro_ids: Vec<String>,
    pub hardware_categories: Vec<String>,
}

pub struct ProfileSelector {
    pub version: Option<String>,
    pub material_id: Option<String>,
    pub nozzle_diameter_mm: Option<f64>,
    pub profile_id: Option<String>,
}

pub fn search(base_url: &str, filter: SearchFilter, first: usize) -> Result<Value, RegistryError> {
    let mut where_filter = serde_json::Map::new();
    insert_option(&mut where_filter, "text", filter.text);
    insert_vec(&mut where_filter, "vendor", filter.vendor);
    insert_vec(&mut where_filter, "firmware", uppercase(filter.firmware));
    insert_vec(
        &mut where_filter,
        "kinematics",
        uppercase(filter.kinematics),
    );
    insert_vec(&mut where_filter, "material", filter.material);
    insert_number(
        &mut where_filter,
        "nozzleDiameterMm",
        filter.nozzle_diameter_mm,
    );
    insert_vec(&mut where_filter, "providesMacros", filter.macro_ids);
    insert_vec(
        &mut where_filter,
        "hardwareCategories",
        filter.hardware_categories,
    );
    let mut volume = serde_json::Map::new();
    insert_number(&mut volume, "xMm", filter.build_x_mm);
    insert_number(&mut volume, "yMm", filter.build_y_mm);
    insert_number(&mut volume, "zMm", filter.build_z_mm);
    if !volume.is_empty() {
        where_filter.insert("minimumBuildVolume".into(), Value::Object(volume));
    }

    let data = graph(
        base_url,
        r#"
          query SearchPrinters($where: PrinterFilter, $first: Int) {
            printers(where: $where, first: $first) {
              totalCount
              pageInfo { hasNextPage endCursor }
              nodes {
                id name vendor model variant kind
                versions {
                  version trustLevel supportStatus publishedAt
                  capabilities {
                    firmware { flavor version versionRange }
                    machine {
                      kinematics
                      buildVolume {
                        x { sizeMm } y { sizeMm } z { sizeMm }
                      }
                      maxAccelerationMmS2
                    }
                    materials { id family status }
                    hardware { role component { id category manufacturer model } }
                    macroBindings {
                      configuredName enabled
                      definition { id name purpose }
                    }
                  }
                }
              }
            }
          }
        "#,
        json!({ "where": Value::Object(where_filter), "first": first }),
    )?;
    data.get("printers")
        .cloned()
        .ok_or_else(|| RegistryError::new("registry response omitted printers"))
}

pub fn inspect(
    base_url: &str,
    id: &str,
    version: Option<&str>,
) -> Result<Option<Value>, RegistryError> {
    let data = graph(
        base_url,
        r#"
          query InspectPrinter($id: ID!, $version: String) {
            printer(id: $id, version: $version) {
              id name vendor model variant kind
              versions {
                version trustLevel supportStatus publishedAt
                packSha256 packUrl capabilitiesUrl
                capabilities {
                  firmware {
                    flavor version versionRange supportedCommands configurationSource
                  }
                  machine {
                    bedShape kinematics
                    buildVolume {
                      x { minMm maxMm sizeMm }
                      y { minMm maxMm sizeMm }
                      z { minMm maxMm sizeMm }
                    }
                    maxAccelerationMmS2 maxJunctionVelocityMmS
                  }
                  hardware {
                    role quantity
                    component {
                      id category manufacturer model revision
                      specificationsJson interfaces
                    }
                  }
                  materials {
                    id family status propertiesJson
                    filaments {
                      id manufacturer name sku color diameterMm densityGcm3 abrasive
                      nozzleTemperatureC { min max }
                      bedTemperatureC { min max }
                      drying { temperatureC durationHours }
                    }
                  }
                  processPresets {
                    id materialId filamentId nozzleDiameterMm layerHeightMm lineWidthMm
                    maxVolumetricFlowMm3S nozzleTemperatureC bedTemperatureC settingsJson
                  }
                  slicerMappings { slicer versionRange presetId settingsJson }
                  macroBindings {
                    configuredName enabled argumentsJson
                    definition { id name purpose }
                    implementation {
                      id firmware version sourceUrl sha256
                      parameters { name type required unit defaultValue }
                      requirements
                    }
                  }
                }
                profiles {
                  id materialId filamentId processPresetId
                  nozzleDiameterMm profileUrl sha256
                }
                proofs { id type status trustLevel url sha256 generatedAt }
                provenance {
                  id kind title url path retrievedAt confidence
                }
              }
            }
          }
        "#,
        json!({ "id": id, "version": version }),
    )?;
    Ok(data
        .get("printer")
        .filter(|value| !value.is_null())
        .cloned())
}

pub fn resolve_profile(
    base_url: &str,
    printer_id: &str,
    selector: &ProfileSelector,
) -> Result<Option<Value>, RegistryError> {
    let data = graph(
        base_url,
        r#"
          query ResolveProfile(
            $id: ID!
            $version: String
            $materialId: ID
            $nozzleDiameterMm: Float
          ) {
            printer(id: $id, version: $version) {
              versions {
                profiles(
                  materialId: $materialId
                  nozzleDiameterMm: $nozzleDiameterMm
                ) {
                  id materialId filamentId processPresetId
                  nozzleDiameterMm profileUrl sha256
                }
              }
            }
          }
        "#,
        json!({
            "id": printer_id,
            "version": selector.version,
            "materialId": selector.material_id,
            "nozzleDiameterMm": selector.nozzle_diameter_mm,
        }),
    )?;
    let Some(printer) = data.get("printer").filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let mut matches = printer
        .get("versions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|version| {
            version
                .get("profiles")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter(|profile| {
            selector
                .profile_id
                .as_deref()
                .is_none_or(|id| profile.get("id").and_then(Value::as_str) == Some(id))
        })
        .cloned()
        .collect::<Vec<_>>();
    match matches.len() {
        0 => Ok(None),
        1 => Ok(matches.pop()),
        count => Err(RegistryError::new(format!(
            "{count} profiles match; add --profile, --material, --nozzle, or --version"
        ))),
    }
}

pub fn download_profile(profile: &Value, out: Option<&Path>) -> Result<Vec<u8>, RegistryError> {
    let url = profile
        .get("profileUrl")
        .and_then(Value::as_str)
        .ok_or_else(|| RegistryError::new("resolved profile omitted profileUrl"))?;
    let expected = profile
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| RegistryError::new("resolved profile omitted sha256"))?;
    let response = ureq::get(url)
        .timeout(Duration::from_secs(30))
        .call()
        .map_err(http_error)?;
    let bytes = read_limited(response)?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(RegistryError::new(format!(
            "profile SHA-256 mismatch: expected {expected}, received {actual}"
        )));
    }
    serde_json::from_slice::<Value>(&bytes)
        .map_err(|error| RegistryError::new(format!("profile is not valid JSON: {error}")))?;
    if let Some(path) = out {
        std::fs::write(path, &bytes).map_err(|error| {
            RegistryError::new(format!("cannot write {}: {error}", path.display()))
        })?;
    }
    Ok(bytes)
}

fn graph(base_url: &str, query: &str, variables: Value) -> Result<Value, RegistryError> {
    let url = format!("{}/graphql", base_url.trim_end_matches('/'));
    let response = ureq::post(&url)
        .timeout(Duration::from_secs(30))
        .set("Content-Type", "application/json")
        .send_json(json!({ "query": query, "variables": variables }))
        .map_err(http_error)?;
    let envelope: Value = serde_json::from_slice(&read_limited(response)?)
        .map_err(|error| RegistryError::new(format!("registry returned invalid JSON: {error}")))?;
    if let Some(errors) = envelope.get("errors").and_then(Value::as_array) {
        let messages = errors
            .iter()
            .filter_map(|error| error.get("message").and_then(Value::as_str))
            .collect::<Vec<_>>();
        return Err(RegistryError::new(if messages.is_empty() {
            "registry returned a GraphQL error".into()
        } else {
            messages.join("; ")
        }));
    }
    envelope
        .get("data")
        .cloned()
        .ok_or_else(|| RegistryError::new("registry response omitted data"))
}

fn read_limited(response: ureq::Response) -> Result<Vec<u8>, RegistryError> {
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| RegistryError::new(format!("cannot read registry response: {error}")))?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(RegistryError::new("registry response exceeded 8 MiB"));
    }
    Ok(bytes)
}

fn http_error(error: ureq::Error) -> RegistryError {
    match error {
        ureq::Error::Status(code, response) => {
            let detail = read_limited(response)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .unwrap_or_default();
            RegistryError::new(format!(
                "registry returned HTTP {code}{}",
                if detail.is_empty() {
                    String::new()
                } else {
                    format!(": {}", detail.chars().take(500).collect::<String>())
                }
            ))
        }
        ureq::Error::Transport(error) => {
            RegistryError::new(format!("cannot reach printer registry: {error}"))
        }
    }
}

fn uppercase(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.to_ascii_uppercase())
        .collect()
}

fn insert_option(map: &mut serde_json::Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value {
        map.insert(key.into(), Value::String(value));
    }
}

fn insert_vec(map: &mut serde_json::Map<String, Value>, key: &str, value: Vec<String>) {
    if !value.is_empty() {
        map.insert(
            key.into(),
            Value::Array(value.into_iter().map(Value::String).collect()),
        );
    }
}

fn insert_number(map: &mut serde_json::Map<String, Value>, key: &str, value: Option<f64>) {
    if let Some(value) = value {
        map.insert(key.into(), json!(value));
    }
}

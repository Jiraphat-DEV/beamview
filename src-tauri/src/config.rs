use std::collections::HashMap;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    pub schema_version: u32,
    pub last_video_device_id: Option<String>,
    pub last_audio_device_id: Option<String>,
    pub theme: Theme,
    pub hotkeys: HashMap<String, String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            last_video_device_id: None,
            last_audio_device_id: None,
            theme: Theme::System,
            hotkeys: default_hotkeys(),
        }
    }
}

pub fn default_hotkeys() -> HashMap<String, String> {
    HashMap::from([
        ("fullscreen".into(), "Cmd+F".into()),
        ("mute".into(), "Cmd+M".into()),
        ("settings".into(), "Cmd+,".into()),
        ("quit".into(), "Cmd+Q".into()),
    ])
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Config directory unavailable on this platform")]
    DirectoryUnavailable,
    #[error("Config parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Unsupported config schema version: {0}")]
    UnsupportedSchemaVersion(u64),
}

/// Resolve the platform-specific config path.
///
/// macOS: `~/Library/Application Support/com.beamview.app/config.json`.
/// The path is computed but not created — `save` makes the parent dir on demand.
pub fn default_config_path() -> Result<PathBuf, ConfigError> {
    let dirs =
        ProjectDirs::from("com", "beamview", "app").ok_or(ConfigError::DirectoryUnavailable)?;
    Ok(dirs.config_dir().join("config.json"))
}

/// Load config from `path`, returning `AppConfig::default()` if the file is absent.
///
/// Schema migration is driven by the top-level `schema_version` field:
///
/// - Missing (implicit v0) or `1`: parsed as the current struct. Missing fields
///   fall back to `AppConfig::default()` via the container-level `#[serde(default)]`.
/// - Anything higher: returns `UnsupportedSchemaVersion` so callers can prompt
///   the user instead of silently downgrading data written by a newer build.
pub fn load(path: &Path) -> Result<AppConfig, ConfigError> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let text = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let version = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    match version {
        0 | 1 => Ok(serde_json::from_value(value)?),
        other => Err(ConfigError::UnsupportedSchemaVersion(other)),
    }
}

/// Atomically persist `config` to `path`.
///
/// Writes a sibling `<path>.tmp` and renames it into place so a crash mid-write
/// cannot leave a half-formed JSON file on disk (POSIX `rename(2)` is atomic;
/// same behaviour on Windows for same-volume renames).
pub fn save(config: &AppConfig, path: &Path) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Delete the config file and return the default so the UI can refresh state.
pub fn reset(path: &Path) -> Result<AppConfig, ConfigError> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(AppConfig::default())
}

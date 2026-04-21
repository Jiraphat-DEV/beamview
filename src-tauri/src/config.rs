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
    /// True once the user has completed (or dismissed) the first-run
    /// Welcome flow. Gates the welcome screen rendering in the UI.
    /// Added in Milestone 6; absent in older configs → serde default (false).
    pub welcome_dismissed: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            last_video_device_id: None,
            last_audio_device_id: None,
            theme: Theme::System,
            hotkeys: default_hotkeys(),
            welcome_dismissed: false,
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_is_current_schema() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(cfg.theme, Theme::System);
        assert!(cfg.last_video_device_id.is_none());
        assert!(cfg.last_audio_device_id.is_none());
        for key in ["fullscreen", "mute", "settings", "quit"] {
            assert!(cfg.hotkeys.contains_key(key), "missing hotkey {key}");
        }
    }

    #[test]
    fn round_trip_preserves_fields() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let cfg = AppConfig {
            schema_version: CURRENT_SCHEMA_VERSION,
            last_video_device_id: Some("video-abc".into()),
            last_audio_device_id: Some("audio-xyz".into()),
            theme: Theme::Dark,
            hotkeys: default_hotkeys(),
            welcome_dismissed: true,
        };
        save(&cfg, &path).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded, cfg);
    }

    #[test]
    fn load_without_welcome_dismissed_defaults_false() {
        // Older configs (pre-Milestone 6) have no welcome_dismissed key —
        // serde(default) should yield `false` so first-run UX re-runs.
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"schema_version": 1, "theme": "light"}"#).unwrap();
        let cfg = load(&path).unwrap();
        assert!(!cfg.welcome_dismissed);
        assert_eq!(cfg.theme, Theme::Light);
    }

    #[test]
    fn load_missing_returns_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("never-written.json");
        let cfg = load(&path).unwrap();
        assert_eq!(cfg, AppConfig::default());
    }

    #[test]
    fn load_unsupported_schema_errors() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"schema_version": 999}"#).unwrap();
        match load(&path) {
            Err(ConfigError::UnsupportedSchemaVersion(999)) => {}
            other => panic!("expected UnsupportedSchemaVersion(999), got {other:?}"),
        }
    }

    #[test]
    fn load_without_schema_version_parses_as_v0_and_fills_defaults() {
        // Treating missing schema_version as implicit v0 — migrate forward by
        // relying on #[serde(default)] on AppConfig to fill in missing fields.
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"theme": "dark"}"#).unwrap();
        let cfg = load(&path).unwrap();
        assert_eq!(cfg.theme, Theme::Dark);
        assert_eq!(cfg.schema_version, CURRENT_SCHEMA_VERSION);
        assert!(cfg.hotkeys.contains_key("fullscreen"));
    }

    #[test]
    fn atomic_write_cleans_up_tmp_on_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        save(&AppConfig::default(), &path).unwrap();
        let tmp = path.with_extension("json.tmp");
        assert!(path.exists(), "final file should exist");
        assert!(!tmp.exists(), "tmp file should be renamed away");
    }

    #[test]
    fn save_creates_parent_dir_if_missing() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a/b/c/config.json");
        save(&AppConfig::default(), &nested).unwrap();
        assert!(nested.exists());
    }

    #[test]
    fn reset_deletes_file_and_returns_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        save(&AppConfig::default(), &path).unwrap();
        assert!(path.exists());
        let cfg = reset(&path).unwrap();
        assert_eq!(cfg, AppConfig::default());
        assert!(!path.exists());
    }

    #[test]
    fn reset_on_missing_file_is_ok() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("never.json");
        let cfg = reset(&path).unwrap();
        assert_eq!(cfg, AppConfig::default());
    }

    #[test]
    fn theme_serde_roundtrip() {
        for t in [Theme::Light, Theme::Dark, Theme::System] {
            let s = serde_json::to_string(&t).unwrap();
            let back: Theme = serde_json::from_str(&s).unwrap();
            assert_eq!(t, back);
        }
        // Verify the lowercase rename.
        assert_eq!(serde_json::to_string(&Theme::Dark).unwrap(), "\"dark\"");
    }
}

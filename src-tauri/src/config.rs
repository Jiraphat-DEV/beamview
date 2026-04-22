use std::collections::HashMap;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

/// A rectangular region within a video frame (pixel coordinates, top-left
/// origin).  Mirrors `translation::types::Region` without importing from
/// that module so `config.rs` stays platform-agnostic (OCR is macOS-only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ConfigRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Where the translated subtitle renders in the app window.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubtitlePosition {
    /// Separate panel below the video — does NOT cover game content.
    /// New default; best for gameplay where every pixel of the game
    /// matters.
    #[default]
    PanelBelow,
    /// Absolutely-positioned overlay at the bottom of the video frame
    /// (original M4 behaviour).  Preserved for users who prefer the
    /// compact look and are willing to accept a small bottom strip of
    /// game content being covered.
    OverlayBottom,
}

/// Translation feature configuration (added in schema v2).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TranslationConfig {
    /// Whether the 1-fps sampling loop is enabled on startup.
    pub enabled: bool,
    /// Last-used subtitle region in the video's native coordinate space.
    pub region: Option<ConfigRegion>,
    /// Frames per second for the sampler (0.5 | 1.0 | 2.0). Default 1.0.
    #[serde(default = "default_fps")]
    pub fps: f32,
    /// Show the English caption above the Thai overlay.
    pub show_english_caption: bool,
    /// Where the subtitle overlay/panel renders.  Added after initial
    /// M4 testing showed the overlay covered game content users cared
    /// about; new default is `PanelBelow`.  Missing from older v2
    /// configs → serde default (`panel_below`).
    #[serde(default)]
    pub subtitle_position: SubtitlePosition,
    /// Which translation model is active.  Default `"nllb-200-distilled-600M"`
    /// for back-compat with configs written before the model picker existed.
    /// Field is optional with a default — no schema version bump needed.
    #[serde(default = "default_active_model_id")]
    pub active_model_id: String,
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            region: None,
            fps: 1.0,
            // Default ON — the EN caption anchors the Thai translation to
            // its source sentence, which matters because translation lag
            // (~1–2 s) means the English subtitle on-screen has usually
            // moved on by the time the Thai overlay appears.  Seeing EN +
            // TH together in the overlay lets the user pair them visually
            // instead of guessing which English line is being translated.
            show_english_caption: true,
            subtitle_position: SubtitlePosition::PanelBelow,
            active_model_id: default_active_model_id(),
        }
    }
}

fn default_fps() -> f32 {
    1.0
}

fn default_active_model_id() -> String {
    "nllb-200-distilled-600M".to_owned()
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
    /// Translation feature settings (added in schema v2).
    pub translation: TranslationConfig,
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
            translation: TranslationConfig::default(),
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
/// - Missing (implicit v0), `1`, or `2`: parsed as the current struct.  Missing
///   fields fall back to defaults via the container-level `#[serde(default)]`.
///   v1 configs are parsed into v2 layout (the `translation` sub-struct is
///   absent in v1, so `#[serde(default)]` fills it in) and then saved back so
///   the file on disk reflects the new schema version.
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
        0 | 1 => {
            // Parse into the current struct — #[serde(default)] fills in the
            // new `translation` block that was absent in v1.
            let mut cfg: AppConfig = serde_json::from_value(value)?;
            // Bump the version so the on-disk file stays up to date.
            cfg.schema_version = CURRENT_SCHEMA_VERSION;
            // Best-effort write-back; if it fails we continue with the
            // in-memory value (next launch will migrate again — harmless).
            let _ = save(&cfg, path);
            Ok(cfg)
        }
        2 => Ok(serde_json::from_value(value)?),
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
            translation: TranslationConfig {
                enabled: true,
                region: Some(ConfigRegion {
                    x: 0,
                    y: 756,
                    width: 1920,
                    height: 324,
                }),
                fps: 2.0,
                show_english_caption: true,
                subtitle_position: SubtitlePosition::OverlayBottom,
                active_model_id: "nllb-200-distilled-600M".into(),
            },
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

    /// Loading a v2 config without `active_model_id` (written before model
    /// picker was added) must fill in the default value "nllb-200-distilled-600M".
    #[test]
    fn active_model_id_defaults_to_nllb() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(
            &path,
            r#"{"schema_version": 2, "translation": {"enabled": false}}"#,
        )
        .unwrap();
        let cfg = load(&path).unwrap();
        assert_eq!(
            cfg.translation.active_model_id, "nllb-200-distilled-600M",
            "active_model_id must default to nllb-200-distilled-600M for back-compat"
        );
    }

    /// Round-trip `active_model_id` when set to the fast model.
    #[test]
    fn active_model_id_round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut cfg = AppConfig::default();
        cfg.translation.active_model_id = "m2m100-418M".into();
        save(&cfg, &path).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.translation.active_model_id, "m2m100-418M");
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

    /// Loading a v1 JSON fixture must:
    ///  1. succeed (no error),
    ///  2. produce schema_version == 2,
    ///  3. fill `translation` with defaults (enabled=false, region=None, fps=1.0),
    ///  4. save back to disk so the file now reflects schema_version 2.
    #[test]
    fn v1_migrates_to_v2() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        // Write a realistic v1 fixture (no `translation` field).
        std::fs::write(
            &path,
            r#"{
  "schema_version": 1,
  "last_video_device_id": "cam-001",
  "last_audio_device_id": null,
  "theme": "dark",
  "hotkeys": {},
  "welcome_dismissed": true
}"#,
        )
        .unwrap();

        let cfg = load(&path).unwrap();

        // Version must be bumped.
        assert_eq!(
            cfg.schema_version, 2,
            "schema_version must be 2 after migration"
        );
        // User data preserved.
        assert_eq!(cfg.last_video_device_id.as_deref(), Some("cam-001"));
        assert_eq!(cfg.theme, Theme::Dark);
        assert!(cfg.welcome_dismissed);
        // Translation defaults applied.
        assert!(!cfg.translation.enabled, "enabled must default to false");
        assert!(
            cfg.translation.region.is_none(),
            "region must default to None"
        );
        assert!(
            (cfg.translation.fps - 1.0).abs() < f32::EPSILON,
            "fps must default to 1.0"
        );
        // New default: EN caption is ON so users can pair EN→TH visually
        // despite the ~1–2 s translation lag.  Existing v2 configs that
        // already persisted `false` continue to round-trip as `false`.
        assert!(cfg.translation.show_english_caption);

        // The on-disk file must also now show schema_version 2.
        let on_disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            on_disk["schema_version"], 2,
            "disk file must be bumped to v2"
        );
    }

    /// Loading a v2 file with a populated TranslationConfig must round-trip exactly.
    #[test]
    fn v2_round_trip_with_translation_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let cfg = AppConfig {
            schema_version: 2,
            last_video_device_id: None,
            last_audio_device_id: None,
            theme: Theme::System,
            hotkeys: default_hotkeys(),
            welcome_dismissed: false,
            translation: TranslationConfig {
                enabled: false,
                region: Some(ConfigRegion {
                    x: 100,
                    y: 800,
                    width: 1720,
                    height: 200,
                }),
                fps: 0.5,
                show_english_caption: false,
                subtitle_position: SubtitlePosition::PanelBelow,
                active_model_id: "m2m100-418M".into(),
            },
        };
        save(&cfg, &path).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded, cfg);
        assert_eq!(loaded.translation.fps, 0.5_f32);
        assert_eq!(loaded.translation.region.unwrap().x, 100);
    }
}

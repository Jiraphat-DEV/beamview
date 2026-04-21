use std::path::PathBuf;

use tauri::{AppHandle, Runtime};

use crate::config::{self, AppConfig};

fn config_path() -> Result<PathBuf, String> {
    config::default_config_path().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_config() -> Result<AppConfig, String> {
    let path = config_path()?;
    let cfg = config::load(&path).map_err(|e| e.to_string())?;
    log::info!("config loaded from {}", path.display());
    Ok(cfg)
}

#[tauri::command]
pub fn save_config(config: AppConfig) -> Result<(), String> {
    let path = config_path()?;
    config::save(&config, &path).map_err(|e| e.to_string())?;
    log::info!("config saved to {}", path.display());
    Ok(())
}

#[tauri::command]
pub fn reset_config() -> Result<AppConfig, String> {
    let path = config_path()?;
    let cfg = config::reset(&path).map_err(|e| e.to_string())?;
    log::info!("config reset: {}", path.display());
    Ok(cfg)
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn quit_app<R: Runtime>(app: AppHandle<R>) {
    log::info!("quit requested");
    app.exit(0);
}

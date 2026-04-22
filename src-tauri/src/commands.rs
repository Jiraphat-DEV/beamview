use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Mutex;

use crate::config::{self, AppConfig};
use crate::translation::{
    engine::{ModelStatusHandle, TranslationEngine},
    EngineError, ModelStatus, OcrTranslateResult, Region,
};

/// Shared state type for the translation engine.
pub type TranslationEngineState = Arc<Mutex<TranslationEngine>>;

const MAIN_WINDOW_LABEL: &str = "main";

fn main_window<R: Runtime>(app: &AppHandle<R>) -> Result<tauri::WebviewWindow<R>, String> {
    app.get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| format!("window {MAIN_WINDOW_LABEL} not found"))
}

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

#[tauri::command]
pub fn is_fullscreen<R: Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    let window = main_window(&app)?;
    window.is_fullscreen().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_fullscreen<R: Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    let window = main_window(&app)?;
    let current = window.is_fullscreen().map_err(|e| e.to_string())?;
    let next = !current;
    window.set_fullscreen(next).map_err(|e| e.to_string())?;
    log::info!("fullscreen toggled: {current} -> {next}");
    Ok(next)
}

// ── Translation commands (M3) ─────────────────────────────────────────────────

/// OCR a captured JPEG frame and translate the recognised English text to Thai.
///
/// `jpeg_bytes` — raw JPEG bytes (Tauri serialises `Vec<u8>` as a JSON array
/// of numbers, which matches `Array.from(Uint8Array)` on the frontend).
///
/// Returns `None` when the frame contains no text or is a near-duplicate of
/// the previous frame (the frontend should keep the last translation visible).
#[tauri::command]
pub async fn ocr_translate(
    state: tauri::State<'_, TranslationEngineState>,
    jpeg_bytes: Vec<u8>,
    region: Option<Region>,
) -> Result<Option<OcrTranslateResult>, String> {
    let mut engine = state.lock().await;
    engine
        .ocr_translate(jpeg_bytes, region)
        .await
        .map_err(|e: EngineError| e.to_string())
}

/// Return the current model status without downloading anything.
///
/// Reads from the `ModelStatusHandle` (an `Arc<RwLock<ModelStatus>>`) that is
/// stored as separate Tauri state.  This never acquires the engine mutex, so
/// it cannot block for 5 minutes during a large download (carry-over B).
#[tauri::command]
pub fn get_translation_model_status(
    status: tauri::State<'_, ModelStatusHandle>,
) -> Result<ModelStatus, String> {
    Ok(status.read().unwrap().clone())
}

/// Download the NLLB-200 model files (first-run, ~900 MB) and load the
/// translator into memory.
///
/// Emits `model-download-progress` events with `ModelStatus::Downloading`
/// payloads while the download is in progress.  The final event payload is
/// `ModelStatus::Ready` on success.
#[tauri::command]
pub async fn download_translation_model(
    app: AppHandle,
    state: tauri::State<'_, TranslationEngineState>,
    status: tauri::State<'_, ModelStatusHandle>,
) -> Result<(), String> {
    let mut engine = state.lock().await;
    engine
        .ensure_ready(&app, &status.inner().clone())
        .await
        .map_err(|e: EngineError| e.to_string())
}

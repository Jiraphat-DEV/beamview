use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Mutex;

use crate::config::{self, AppConfig};
use crate::translation::{
    engine::{ModelInfo, ModelStatusHandle, TranslationEngine},
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

// ── Translation commands ──────────────────────────────────────────────────────

/// OCR a captured JPEG frame and translate the recognised English text to Thai.
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
/// Reads from the `ModelStatusHandle` — never acquires the engine mutex.
#[tauri::command]
pub fn get_translation_model_status(
    status: tauri::State<'_, ModelStatusHandle>,
) -> Result<ModelStatus, String> {
    Ok(status.read().unwrap().clone())
}

/// Return metadata for all models in the catalogue (Part C).
///
/// Each entry carries `installed`, `is_active`, and `installed_size_bytes` so
/// the UI can render the full model picker without additional calls.
#[tauri::command]
pub async fn list_translation_models(
    state: tauri::State<'_, TranslationEngineState>,
) -> Result<Vec<ModelInfo>, String> {
    let engine = state.lock().await;
    Ok(engine.list_models())
}

/// Download a specific model by ID.
///
/// Emits `model-download-progress` events with payload
/// `{ model_id: string, status: ModelStatus }`.
///
/// If `model_id` is the active model and no translator is loaded, auto-loads
/// the translator after download completes.
#[tauri::command]
pub async fn download_translation_model(
    app: AppHandle,
    state: tauri::State<'_, TranslationEngineState>,
    status: tauri::State<'_, ModelStatusHandle>,
    model_id: Option<String>,
) -> Result<(), String> {
    let mut engine = state.lock().await;

    // Legacy callers (frontend pre-model-picker) pass no model_id — default
    // to the active model so the old behaviour is preserved.
    match model_id.as_deref() {
        None | Some("") => engine
            .ensure_ready(&app, &status.inner().clone())
            .await
            .map_err(|e: EngineError| e.to_string()),
        Some(id) => engine
            .download_model(&app, &status.inner().clone(), id)
            .await
            .map_err(|e: EngineError| e.to_string()),
    }
}

/// Delete a model's files from disk.
///
/// Returns an error string when the model is currently active and loaded.
/// The frontend should refuse to call this for the active model and show a
/// tooltip instead, but the backend enforces the constraint regardless.
#[tauri::command]
pub async fn delete_translation_model(
    state: tauri::State<'_, TranslationEngineState>,
    model_id: String,
) -> Result<(), String> {
    let mut engine = state.lock().await;
    engine
        .delete_model(&model_id)
        .await
        .map_err(|e: EngineError| e.to_string())
}

/// Switch the active translation model to `model_id`.
///
/// Unloads the current translator and loads the new one from disk.
/// Returns an error string if the model is not installed.
///
/// The frontend should persist `active_model_id` to config after a successful
/// call so it survives app restart.
#[tauri::command]
pub async fn set_active_translation_model(
    state: tauri::State<'_, TranslationEngineState>,
    model_id: String,
) -> Result<(), String> {
    let mut engine = state.lock().await;
    engine
        .set_active_model(&model_id)
        .await
        .map_err(|e: EngineError| e.to_string())
}

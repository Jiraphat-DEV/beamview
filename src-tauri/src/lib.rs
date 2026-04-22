mod commands;
mod config;
mod logging;
mod menu;
pub mod translation;

use std::sync::Arc;

use tauri::Manager;
use tokio::sync::Mutex;

use commands::TranslationEngineState;
use translation::engine::{ModelStatusHandle, TranslationEngine};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::install_panic_hook();

    tauri::Builder::default()
        .plugin(logging::build_plugin())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Load config to read the persisted active_model_id.  We do a
            // best-effort load — if config is absent or unparseable we fall
            // back to the default model so the app still starts cleanly.
            let active_model_id = config::default_config_path()
                .ok()
                .and_then(|p| config::load(&p).ok())
                .map(|cfg| cfg.translation.active_model_id)
                .unwrap_or_else(|| "nllb-200-distilled-600M".to_owned());

            log::info!("[setup] active_model_id from config: '{active_model_id}'");

            let (engine, status_handle) = TranslationEngine::new(active_model_id).map_err(|e| {
                log::error!("TranslationEngine::new failed: {e}");
                e
            })?;

            let state: TranslationEngineState = Arc::new(Mutex::new(engine));
            app.manage(state);

            // Register the non-blocking status handle separately so
            // get_translation_model_status never waits on the engine mutex.
            let handle: ModelStatusHandle = status_handle;
            app.manage(handle);

            let handle = app.handle().clone();
            let menu = menu::build(&handle)?;
            app.set_menu(menu)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            menu::handle_event(app, event.id.0.as_str());
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_config,
            commands::save_config,
            commands::reset_config,
            commands::get_app_version,
            commands::quit_app,
            commands::is_fullscreen,
            commands::toggle_fullscreen,
            // Translation IPC
            commands::ocr_translate,
            commands::get_translation_model_status,
            commands::download_translation_model,
            // Model picker (M5.5)
            commands::list_translation_models,
            commands::delete_translation_model,
            commands::set_active_translation_model,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

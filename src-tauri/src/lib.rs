mod commands;
mod config;
mod logging;
mod menu;
pub mod translation;

use std::sync::Arc;

use tauri::Manager;
use tokio::sync::Mutex;

use commands::TranslationEngineState;
use translation::engine::TranslationEngine;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::install_panic_hook();

    tauri::Builder::default()
        .plugin(logging::build_plugin())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Initialise the TranslationEngine.  If the model-store directory
            // cannot be resolved (e.g. Application Support is missing) this
            // returns an Err and setup() fails loudly — the app will not start.
            let engine = TranslationEngine::new().map_err(|e| {
                log::error!("TranslationEngine::new failed: {e}");
                e
            })?;
            let state: TranslationEngineState = Arc::new(Mutex::new(engine));
            app.manage(state);

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
            // M3 — translation IPC
            commands::ocr_translate,
            commands::get_translation_model_status,
            commands::download_translation_model,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

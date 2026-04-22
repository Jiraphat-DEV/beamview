//! Native macOS menu bar.
//!
//! App menu: About + Preferences + Quit (+ standard macOS items).
//! Translation menu: Toggle Translation (Cmd+T).
//! Edit menu: standard Cut/Copy/Paste/Undo/Redo for webview text fields.
//!
//! Custom items emit string events that the frontend subscribes to:
//!  - `menu://preferences`       → opens SettingsModal
//!  - `menu://translation-toggle` → toggles translation (same as Cmd+T)

use tauri::menu::{
    AboutMetadataBuilder, Menu, MenuItem, PredefinedMenuItem, Submenu, SubmenuBuilder,
};
use tauri::{AppHandle, Emitter, Runtime};

pub const PREFERENCES_EVENT: &str = "menu://preferences";
pub const TRANSLATION_TOGGLE_EVENT: &str = "menu://translation-toggle";

const PREFERENCES_ID: &str = "beamview-preferences";
const TRANSLATION_TOGGLE_ID: &str = "beamview-translation-toggle";

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let about_metadata = AboutMetadataBuilder::new()
        .name(Some("Beamview"))
        .version(Some(env!("CARGO_PKG_VERSION")))
        .copyright(Some("© 2026 Jiraphat"))
        .build();

    let preferences = MenuItem::with_id(
        app,
        PREFERENCES_ID,
        "Preferences…",
        true,
        Some("CmdOrCtrl+,"),
    )?;

    let app_menu = SubmenuBuilder::new(app, "Beamview")
        .item(&PredefinedMenuItem::about(
            app,
            Some("About Beamview"),
            Some(about_metadata),
        )?)
        .separator()
        .item(&preferences)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;

    // Translation submenu — Cmd+T is registered as a web-layer hotkey in
    // App.svelte; the menu item emits the same intent event so the menu bar
    // works even when the webview doesn't have focus.
    let translation_toggle = MenuItem::with_id(
        app,
        TRANSLATION_TOGGLE_ID,
        "Toggle Translation",
        true,
        Some("CmdOrCtrl+T"),
    )?;

    let translation_menu = SubmenuBuilder::new(app, "Translation")
        .item(&translation_toggle)
        .build()?;

    Menu::with_items(app, &[&app_menu, &edit_menu, &translation_menu])
}

/// Dispatch menu clicks. Custom items emit frontend events; predefined items
/// are handled by Tauri directly.
pub fn handle_event<R: Runtime>(app: &AppHandle<R>, event_id: &str) {
    match event_id {
        PREFERENCES_ID => match app.emit(PREFERENCES_EVENT, ()) {
            Ok(()) => log::info!("preferences menu clicked — emitted {PREFERENCES_EVENT}"),
            Err(e) => log::warn!("failed to emit preferences event: {e}"),
        },
        TRANSLATION_TOGGLE_ID => match app.emit(TRANSLATION_TOGGLE_EVENT, ()) {
            Ok(()) => {
                log::info!("translation toggle menu clicked — emitted {TRANSLATION_TOGGLE_EVENT}")
            }
            Err(e) => log::warn!("failed to emit translation-toggle event: {e}"),
        },
        _ => {}
    }
}

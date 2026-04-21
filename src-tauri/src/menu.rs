//! Native macOS menu bar (stub).
//!
//! Phase 1 keeps the menu intentionally minimal: App menu with About +
//! Preferences + Quit, plus the standard Edit submenu so text fields
//! inside the webview get the expected Cut/Copy/Paste shortcuts.
//!
//! The `Preferences…` item emits the string event `menu://preferences`
//! which the frontend subscribes to (Milestone 6, where SettingsModal
//! lands). Until then the event is received and logged but not acted on.

use tauri::menu::{
    AboutMetadataBuilder, Menu, MenuItem, PredefinedMenuItem, Submenu, SubmenuBuilder,
};
use tauri::{AppHandle, Emitter, Runtime};

pub const PREFERENCES_EVENT: &str = "menu://preferences";
const PREFERENCES_ID: &str = "beamview-preferences";

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

    Menu::with_items(app, &[&app_menu, &edit_menu])
}

/// Dispatch menu clicks. Only the `Preferences…` item is custom — everything
/// else is a `PredefinedMenuItem` handled by Tauri.
pub fn handle_event<R: Runtime>(app: &AppHandle<R>, event_id: &str) {
    if event_id == PREFERENCES_ID {
        if let Err(e) = app.emit(PREFERENCES_EVENT, ()) {
            log::warn!("failed to emit preferences event: {e}");
        }
    }
}

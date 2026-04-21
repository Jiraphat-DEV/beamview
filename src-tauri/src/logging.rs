use tauri::Wry;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

/// Build the `tauri-plugin-log` plugin with the Beamview profile.
///
/// Targets:
/// - stdout (visible in `pnpm tauri dev`)
/// - `<log-dir>/beamview.log` (on macOS: `~/Library/Logs/com.beamview.app/beamview.log`)
/// - the webview console (so `tracing::info!` from Rust shows up alongside TS `console.log`)
///
/// Log level: Debug on debug builds, Info in release. Files keep rotating
/// at 5 MB each with `KeepAll` so history survives long sessions.
pub fn build_plugin() -> tauri::plugin::TauriPlugin<Wry> {
    tauri_plugin_log::Builder::new()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir {
                file_name: Some("beamview".into()),
            }),
            Target::new(TargetKind::Webview),
        ])
        .rotation_strategy(RotationStrategy::KeepAll)
        .max_file_size(5 * 1024 * 1024)
        .level(if cfg!(debug_assertions) {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .build()
}

/// Route Rust panics through the `tracing` layer so they reach the log file
/// before the process aborts. Call once during startup.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        log::error!("panic: {info}");
    }));
}

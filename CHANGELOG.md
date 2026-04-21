# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Tauri v2 scaffold with plain Svelte 5 + TypeScript frontend (Vite 6)
- Project metadata: `com.beamview.app` identifier, Entertainment category,
  window defaults (1280×720, min 640×360)
- macOS bundle config targeting macOS 13+
- `src-tauri/Info.plist` with `NSCameraUsageDescription` and
  `NSMicrophoneUsageDescription` (required to avoid crash on
  `getUserMedia()` for capture card access)
- LICENSE (MIT), README, CHANGELOG, `.editorconfig`
- **Rust shell (Milestone 2)**: `config.rs` with `AppConfig`, atomic
  save (`tmp` + `rename`), schema-version migration, and `ConfigError`
  via `thiserror`
- IPC commands (`load_config`, `save_config`, `reset_config`,
  `get_app_version`, `quit_app`) returning `Result<T, String>` for
  TS-side error handling
- `logging.rs` wires `tauri-plugin-log` with stdout + file
  (`~/Library/Logs/com.beamview.app/beamview.log`) + webview console
  targets; panic hook routes Rust panics through `log::error!`
- Native macOS menu stub (`Beamview`, `Edit` submenus); Preferences…
  emits a `menu://preferences` event for the future SettingsModal
- Unit tests for config round-trip, migration, atomic write, reset
- TypeScript IPC layer (`src/lib/ipc/{types,commands,index}.ts`)
  mirroring the Rust surface

### Decisions

- **Svelte flavor:** plain Svelte + Vite instead of the SvelteKit boilerplate
  that `create-tauri-app` ships. A single-window app does not need the Kit
  adapter, `$app/*` imports, or file-based routing. Spec §4 already describes
  the plain Svelte layout (`src/main.ts` + `src/App.svelte`).
- **Vite version:** follow whatever `create-tauri-app` bundles. The draft
  spec pinned "Vite 5.x"; the scaffold gave us Vite 6. Updated spec §2.1
  accordingly — no reason to downgrade since Tauri v2 fully supports Vite 6.
- **Tauri plugin-opener:** removed from the template. Not used in Phase 1.
- **`log` crate (not `tracing`) for Rust logging in Milestone 2.**
  `tauri-plugin-log` hooks the `log` macros directly; a `tracing → log`
  bridge is unnecessary overhead for Phase 1. `tracing` stays in
  `Cargo.toml` for when structured spans become useful.
- **Rust `Info.plist` as a file, not inline JSON.** Tauri v2's
  `bundle.macOS.infoPlist` config key expects a path string. Added
  `src-tauri/Info.plist` which Tauri auto-discovers and merges with
  the default bundle plist at build time.

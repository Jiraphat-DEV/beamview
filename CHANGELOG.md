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
- **UI shell (Milestone 3)**: design token system (`src/app.css`) with
  brand palette (sumi/stone/mist/paper/vermilion + dark variants),
  typography stack (Helvetica Neue / Inter / JetBrains Mono), motion
  (cubic-bezier(0.4,0,0.2,1), 150/250/400ms), spacing scale
- Theme store (`src/lib/stores/theme.svelte.ts`) — Svelte 5 runes
  class with `pref`, `resolved`, `ready`. Subscribes to
  `prefers-color-scheme` so the UI tracks macOS dark-mode toggles
- Layout components (`src/lib/components/`): `TitleBar`, `ActionBar`
  (Fullscreen + Settings Lucide icons), `EmptyState`
- `App.svelte` restructured to three-region layout
  (TitleBar / main / ActionBar) with EmptyState placeholder
- `main.ts` sets `documentElement[data-theme]` synchronously before
  mount to prevent a light-mode flash on dark systems
- `$lib` alias in `vite.config.ts` matching the tsconfig path mapping
- **Capture pipeline (Milestone 4)**: `$lib/capture/constraints.ts`
  builds MediaStreamConstraints with the critical
  `echoCancellation:false / noiseSuppression:false /
  autoGainControl:false` audio trio per spec §5.3
- `$lib/capture/devices.ts` — `enumerateCaptureDevices()`,
  `requestPermission()` probe, `preferDevice()` fallback picker
- `$lib/audio/context.ts` — Web Audio routing (AudioContext
  latencyHint:'interactive', 48 kHz) with a Gain node for
  future volume/mute
- `devices` store (refresh, restoreSelection) and `stream`
  store (acquire / release / status machine with kind-classified
  `StreamError`). Track `ended` event flips status to
  `error` with kind `disconnected`
- `VideoView.svelte` — muted `<video>` bound to `stream.value`
  with `disablepictureinpicture` + `disableremoteplayback`
- `DevicePicker.svelte` — native `<dialog>` with video + audio
  dropdowns, Grant-access fallback when labels are empty, and
  Refresh button
- `App.svelte` wires the picker, auto-acquires the saved device
  on startup when labels indicate media permission is granted
  (spec §17.1 recommendation B), and renders an error panel with
  a "Choose device" recovery when the stream fails
- **Fullscreen + hotkeys (Milestone 5)**: Rust `toggle_fullscreen`
  and `is_fullscreen` commands with
  `core:window:allow-set-fullscreen` + `allow-is-fullscreen`
  capability grants
- `$lib/hotkeys/registry.ts` — single window keydown listener with
  a priority-ordered binding list so Esc closes the top modal
  before exiting fullscreen (spec §17.2). Inputs, textareas, and
  contenteditable targets are excluded automatically
- `$lib/stores/ui.svelte.ts` — `muted` state + `modalStack` for the
  Esc priority stack; `toggleMute` drives the `audio/context`
  GainNode in place without tearing down the Web Audio graph
- Hotkey bindings wired in `App.svelte`: Cmd+F + F11 toggle
  fullscreen, Cmd+M mutes, Cmd+, opens settings placeholder, Esc
  closes top modal else exits fullscreen
- `menu://preferences` event (emitted by the native menu since
  Milestone 2) now has a frontend listener — both the menu item
  and Cmd+, hit the same handler
- `TitleBar` gains a `muted` status label so mute state is visible
  without opening devtools
- **Settings + Welcome (Milestone 6)**: AppConfig gains
  `welcome_dismissed: bool` so the first-run Welcome flow only
  shows until the user completes it. `serde(default)` keeps older
  configs working — covered by a new Rust test
- `WelcomeScreen.svelte` (first-run explainer with Grant access /
  Skip); auto-acquire is now gated on `welcome_dismissed` so we
  never fire `getUserMedia` before the user sees the rationale
- `SettingsModal.svelte` with Video / Audio / About tabs, local
  form state + `dirty` flag driving an explicit Save button per
  spec §8.6. Volume + mute live-apply through `ui` store;
  `theme` + device IDs persist through Rust `saveConfig`
- `ErrorOverlay.svelte` replaces the inline error-panel in App
  with a reusable component that takes title / message / up to
  two actions
- `Toast.svelte` + `ui.showToast(msg, kind)` surface non-blocking
  confirmations (Save succeeded, device switch failed, etc.)
  bottom-right above the ActionBar

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

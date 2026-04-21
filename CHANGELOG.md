# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-04-21

Initial MVP — macOS-only, unsigned. Proof that Beamview can replace
OBS + QuickTime for the narrow use case of playing a console through
a capture card on a Mac.

### Added

- **Scaffold (Milestone 1)**: Tauri v2 + plain Svelte 5 + TypeScript
  (Vite 6), MIT LICENSE, README, CHANGELOG, GitHub Actions CI
  (lint + typecheck + rust tests + tauri debug build), prettier +
  eslint + rustfmt + clippy chain.
- **Rust shell (Milestone 2)**: `AppConfig` with atomic `save()`
  (tmp + rename), `thiserror`-based `ConfigError`, schema-version
  migration; IPC commands `load_config`, `save_config`,
  `reset_config`, `get_app_version`, `quit_app`; `tauri-plugin-log`
  wired to stdout + `~/Library/Logs/com.beamview.app/beamview.log`
  + webview console with a panic hook; native macOS menu (Beamview,
  Edit) with Preferences… emitting `menu://preferences`; Rust unit
  tests for config round-trip, migration, atomic write, reset.
- **UI shell (Milestone 3)**: full design-token system with the
  brand palette (sumi / stone / mist / paper / vermilion + dark
  variants), typography stack (Helvetica Neue / Inter / JetBrains
  Mono), motion + spacing scale; theme store with `prefers-color-scheme`
  subscription; `TitleBar`, `ActionBar`, `EmptyState`; `$lib` alias.
- **Capture pipeline (Milestone 4)**: `getUserMedia` with the
  critical audio trio off (`echoCancellation`, `noiseSuppression`,
  `autoGainControl` = false) per spec §5.3; Web Audio pipeline
  (AudioContext + GainNode) so `<video>` stays muted; `devices`
  and `stream` stores with a `StreamError` kind machine;
  `VideoView.svelte` and `DevicePicker.svelte`; auto-acquire of
  the last-used device on startup (spec §17.1).
- **Fullscreen + hotkeys (Milestone 5)**: Rust `toggle_fullscreen`
  and `is_fullscreen` commands; `$lib/hotkeys/registry.ts` with an
  Esc priority stack; Cmd+F / F11 (fullscreen), Cmd+M (mute),
  Cmd+, (settings), Esc (top modal else exit fullscreen);
  `menu://preferences` bridge so the menu item and Cmd+, fire the
  same handler.
- **Settings + Welcome (Milestone 6)**: `welcome_dismissed: bool`
  config field gating first-run UI; `WelcomeScreen.svelte`;
  `SettingsModal.svelte` with Video / Audio / About tabs, local
  form state + `dirty` flag + explicit Save per spec §8.6;
  `ErrorOverlay.svelte` + `Toast.svelte` + `ui.showToast()`.
- **Polish (Milestone 7)**: Beamview brand icons across
  `src-tauri/icons/` generated from `assets/svg/app-icon-light.svg`
  via `rsvg-convert` + `tauri icon`; `LoadingState.svelte` during
  stream acquisition; auto-hide TitleBar + ActionBar after 2 s of
  inactivity while the stream is active and no modal is open
  (spec §5.4.1); favicon wired into `index.html`.
- **Release prep (Milestone 8)**: GitHub Actions `release.yml`
  triggered by `v*` tag — builds the macOS bundle and attaches
  the `.dmg` to a draft release via `tauri-action`.
  [`docs/MANUAL_TEST.md`](docs/MANUAL_TEST.md) captures the
  spec §9.3 ten-point acceptance checklist.

### Known issues

- App is unsigned. macOS Gatekeeper blocks the first launch — users
  must Ctrl-click → Open once.
- `pnpm tauri dev` runs the binary raw without the `.app` wrapper,
  so macOS camera/mic permission prompts never appear. For capture
  testing use `pnpm tauri build --debug` and open the bundled `.app`.
- Rebuilds can invalidate cached TCC permissions — users may need
  to re-grant camera + microphone access each time the binary hash
  changes.

### Decisions

- **Plain Svelte over SvelteKit.** `create-tauri-app` ships the
  Kit boilerplate; adapter-static + file-based routing buy nothing
  for a single-window app. Spec §4 already describes the plain
  layout (`src/main.ts` + `src/App.svelte`).
- **Vite 6 over pinned 5.x.** The scaffold uses 6; Tauri v2 fully
  supports it. No reason to downgrade.
- **`log` crate over `tracing` for Phase 1.** `tauri-plugin-log`
  hooks the `log` macros directly; a `tracing → log` bridge is
  unnecessary overhead. `tracing` stays in `Cargo.toml` for future
  structured spans.
- **`Info.plist` as a file, not inline.** Tauri v2's
  `bundle.macOS.infoPlist` key expects a path string. Added
  `src-tauri/Info.plist` which Tauri auto-merges at build.
- **Dropped `tauri-plugin-opener`** — unused in Phase 1.

[Unreleased]: https://github.com/Jiraphat-DEV/beamview/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Jiraphat-DEV/beamview/releases/tag/v0.1.0

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

### Decisions

- **Svelte flavor:** plain Svelte + Vite instead of the SvelteKit boilerplate
  that `create-tauri-app` ships. A single-window app does not need the Kit
  adapter, `$app/*` imports, or file-based routing. Spec §4 already describes
  the plain Svelte layout (`src/main.ts` + `src/App.svelte`).
- **Vite version:** follow whatever `create-tauri-app` bundles. The draft
  spec pinned "Vite 5.x"; the scaffold gave us Vite 6. Updated spec §2.1
  accordingly — no reason to downgrade since Tauri v2 fully supports Vite 6.
- **Tauri plugin-opener:** removed from the template. Not used in Phase 1.

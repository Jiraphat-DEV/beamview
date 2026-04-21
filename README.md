<div align="center">
  <img src="assets/png/readme-banner.png" alt="Beamview" width="720" />
</div>

<p align="center">
  <em>Beam your game. See it instantly.</em>
</p>

<p align="center">
  Low-latency HDMI capture card viewer for desktop gamers — built with Tauri v2 and Svelte 5.
</p>

---

## Status

**v0.1.0** — first MVP release. macOS Apple Silicon only, unsigned.

Beamview is a side project that replaces OBS + QuickTime for one
specific job: playing a console game through a capture card on a
Mac. Phase 1 ships with `< 100 ms` end-to-end latency as the target,
a minimal UI that auto-hides while you play, and nothing in the way.

No recording, no streaming, no scene switcher — that is the point.

## Install

1. Download `Beamview_0.1.0_aarch64.dmg` from the [latest release](https://github.com/Jiraphat-DEV/beamview/releases).
2. Open the `.dmg` and drag `Beamview.app` into `/Applications`.
3. The first launch shows a Gatekeeper warning ("Apple could not
   verify…") because the build is unsigned. **Ctrl-click**
   `Beamview.app` in `/Applications` → **Open** → confirm the
   warning once. Subsequent launches work normally.
4. On first run you will see the **Welcome** screen — click _Grant
   access_ and allow camera + microphone in the macOS prompts.
5. Pick your capture card in the device picker. Video + audio will
   be on screen within a couple of seconds.

### Requirements

- macOS 13 or newer (tested on MacBook Air M4).
- A USB video capture card that enumerates as a UVC/UAC device
  (e.g. **UGREEN 15389**, Elgato HD60 S+, AVerMedia Live Gamer).
- Apple Silicon (`aarch64`) — Intel builds not currently shipped.

## Features

- Native macOS fullscreen via `Cmd+F`.
- `Cmd+M` mute without tearing down the audio pipeline.
- TitleBar + ActionBar auto-hide after 2 seconds while the stream is
  live so the video fills the window.
- System dark mode tracks live — palette flips in place.
- Remembers the last-used device and auto-acquires it on next launch.
- Audio passes through untouched — `echoCancellation`,
  `noiseSuppression`, and `autoGainControl` are all disabled so
  game audio stays intact.
- Unified log file at `~/Library/Logs/com.beamview.app/beamview.log`.

## Keyboard shortcuts

| Shortcut        | Action                                    |
| --------------- | ----------------------------------------- |
| `Cmd+F` / `F11` | Toggle fullscreen                         |
| `Cmd+M`         | Mute / unmute audio                       |
| `Cmd+,`         | Open Settings                             |
| `Cmd+Q`         | Quit                                      |
| `Esc`           | Close the top modal, else exit fullscreen |

## Develop

```bash
pnpm install
pnpm tauri dev
```

Vite serves the frontend at `http://localhost:1420`. Tauri launches
a native window that loads it.

**Note:** `pnpm tauri dev` runs the binary raw, without a `.app`
wrapper. macOS will not trigger the camera/mic permission dialog
from this mode, so the capture pipeline can not be exercised
end-to-end. Use `pnpm tauri build --debug` + `open
src-tauri/target/debug/bundle/macos/Beamview.app` when you need
to test capture.

### Scripts

| Command            | Purpose                                                  |
| ------------------ | -------------------------------------------------------- |
| `pnpm dev`         | Vite dev server only                                     |
| `pnpm tauri dev`   | Full app, no `.app` wrapper, no media permission dialogs |
| `pnpm build`       | Production frontend build                                |
| `pnpm tauri build` | Bundled `.dmg` (macOS)                                   |
| `pnpm typecheck`   | `svelte-check` TypeScript + Svelte validation            |
| `pnpm lint`        | Prettier + ESLint + `cargo fmt --check` + clippy         |
| `pnpm format`      | Auto-fix formatting (Prettier + `cargo fmt`)             |

### Project layout

```
beamview/
├── assets/              # Brand master files (SVG + PNG)
├── docs/
│   └── MANUAL_TEST.md   # 10-point acceptance checklist
├── src/                 # Svelte + TypeScript frontend
│   ├── App.svelte
│   ├── app.css          # Design tokens
│   └── lib/             # components / stores / capture / audio / ipc / hotkeys
├── src-tauri/           # Rust backend (Tauri shell)
│   ├── Info.plist       # Camera + microphone usage descriptions
│   └── src/
│       ├── config.rs
│       ├── commands.rs
│       ├── logging.rs
│       └── menu.rs
├── static/              # Copied as-is to the frontend bundle
└── index.html           # Vite entry
```

## Brand

Minimal Japanese. Palette is Sumi ink, Stone, Mist, Paper, and
Vermilion accent. Everything is `--bv-*` CSS variables defined in
`src/app.css`. Brand master assets live under `assets/`.

## Contributing

- Conventional Commits — `feat`, `fix`, `docs`, `chore`,
  `refactor`, `test`, `perf`, `build`, `ci`.
- PRs target `main`. CI (`lint`, `typecheck`, `test-rust`,
  `build-check`) must be green before merge.
- Run through [`docs/MANUAL_TEST.md`](docs/MANUAL_TEST.md) before
  tagging a release.

## Known issues

- **Unsigned** — Gatekeeper blocks the first launch. Ctrl-click →
  Open workaround described in the install steps.
- **`pnpm tauri dev` cannot prompt for camera access** — raw binary,
  no `.app` wrapper. Use `pnpm tauri build --debug` to test capture.
- **Rebuilding invalidates TCC permission** — each `tauri build`
  changes the binary hash; macOS may force you to re-allow camera
  and microphone after a rebuild.

## License

[MIT](LICENSE) © 2026 Jiraphat

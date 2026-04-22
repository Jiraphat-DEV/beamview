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

**v0.2.0** — adds offline real-time EN→TH subtitle translation for
Nintendo Switch JRPGs on top of the v0.1.0 capture viewer. macOS
Apple Silicon only, unsigned.

Beamview is a side project that replaces OBS + QuickTime for one
specific job: playing a console game through a capture card on a
Mac. v0.1.0 (Phase 1) shipped at **51 ms** measured end-to-end
latency on M4 — half the `< 100 ms` target. v0.2.0 (Phase 2) adds
a separate translation pipeline that runs at 1 fps off the render
path, so the 51 ms baseline is preserved with translation on.

No recording, no streaming, no scene switcher — that is the point.

## Install

1. Download `Beamview_0.2.0_aarch64.dmg` from the [latest release](https://github.com/Jiraphat-DEV/beamview/releases).
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

### Offline subtitle translation _(v0.2.0)_

- **Apple Vision OCR + NLLB-200 translator** — reads English
  subtitles inside a user-defined region of the live video, then
  renders the Thai translation as a panel below the game (or as
  an overlay on top, configurable). 100 % offline once the model
  is downloaded — no cloud calls, no API keys, no telemetry.
- **`Cmd+T`** toggles translation on/off; before the model is
  installed it opens the download modal instead.
- **Settings → การแปล** holds the toggle, region calibrator, sampling
  FPS slider (0.5 / 1 / 2), subtitle position picker
  (panel-below / overlay-bottom), EN-caption-on-TH toggle, and
  the model picker / installer.
- **First-run model download ≈ 650 MB** to
  `~/Library/Application Support/com.beamview.Beamview/models/`.
  SHA-256 verified, one automatic retry on transient network
  failure, and a `.ready` sentinel skips re-hashing on next launch.
  CoreML execution provider on Apple Silicon (`CPUAndNeuralEngine`)
  with kernel cache at `~/Library/Caches/com.beamview.Beamview/coreml/`.
- **Cache + dedup** — translations are cached by sha256 of the
  recognised English text (LRU 1 000 entries). A jaro-winkler
  near-duplicate check skips re-translating the same on-screen
  subtitle frame-after-frame. 40–60 % cache hit rate is typical
  on JRPG cutscenes.
- **Render path is untouched** — capture card video still streams
  via `<video>` + `MediaStream` at the v0.1.0 baseline of 51 ms.
  OCR + translation run on `tokio::spawn_blocking` worker threads
  at 1 fps; the sampler drops frames if a previous tick is still
  in flight.

End-to-end overlay latency on M4 MacBook Air (release build,
CoreML warm) sits at p50 ≈ 1.2 s / p95 ≈ 2.0 s — driven by the
sequential autoregressive decode loop in NLLB. Subtitle lines
typically stay on screen for 3–8 s during dialogue, so this is
usable in practice; cache hits return in < 20 ms.

## Keyboard shortcuts

| Shortcut        | Action                                                                        |
| --------------- | ----------------------------------------------------------------------------- |
| `Cmd+F` / `F11` | Toggle fullscreen                                                             |
| `Cmd+M`         | Mute / unmute audio                                                           |
| `Cmd+T`         | Toggle Thai translation overlay (opens download modal if model not installed) |
| `Cmd+,`         | Open Settings                                                                 |
| `Cmd+Q`         | Quit                                                                          |
| `Esc`           | Close the top modal, else exit fullscreen                                     |

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
| `pnpm test:rust`   | `cargo test` for the Rust backend                        |
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

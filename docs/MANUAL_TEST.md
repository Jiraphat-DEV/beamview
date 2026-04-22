# Manual test checklist

Run this before tagging a release. Tied to the ten acceptance criteria
in `BEAMVIEW_TECHNICAL_SPEC.md` §9.3 (not in-repo — the spec stays
local under `temp_project_BEAMVIEW/` and is gitignored).

Prerequisites

- macOS 13+ Apple Silicon (tested on MacBook Air M4)
- UGREEN Capture Card 15389 (or any UVC/UAC-compliant capture card)
- Nintendo Switch 2 (or equivalent console) for the latency test

---

## 1. Fresh install via `.dmg`

- [ ] `pnpm tauri build` completes with `Finished` and prints a
      `.dmg` path under `src-tauri/target/release/bundle/dmg/`.
- [ ] Double-click the `.dmg` → drag `Beamview.app` into the
      mounted Applications shortcut.
- [ ] Launch from `/Applications/Beamview.app` → Gatekeeper
      refuses with _"Apple could not verify…"_. Ctrl-click the app
      → _Open_ → confirm.
- [ ] **WelcomeScreen** appears on first run.

## 2. First-run flow

- [ ] Click _Grant access_ → macOS camera prompt → Allow.
- [ ] macOS microphone prompt → Allow.
- [ ] Welcome closes; DevicePicker opens OR EmptyState shows.
- [ ] Choose the UGREEN video + corresponding audio → _Confirm_.
- [ ] Video + audio begin within ≤ 3 seconds.

## 3. Fullscreen

- [ ] `Cmd+F` → native fullscreen (separate Space, dock + menu
      bar auto-hide). Game remains playable.
- [ ] Inside fullscreen, `Esc` or `Cmd+F` → back to windowed.
- [ ] ActionBar Fullscreen icon mirrors the shortcut.

## 4. Quit + resume

- [ ] `Cmd+Q` quits cleanly — no hang, no crash.
- [ ] Reopen the app → last-used device auto-acquires with no
      picker flash. Log line `attempting auto-acquire` → `stream active`.

## 5. Disconnect recovery

- [ ] With stream live, unplug the capture card USB.
- [ ] `ErrorOverlay` appears with _"Device disconnected"_.
- [ ] Plug the card back in → _Choose device_ → repick the UGREEN →
      video + audio resume.

## 6. 30-minute playthrough

- [ ] Play a real game for 30 minutes straight. No crash, no visible
      freeze, no audio artifacts (echo cancellation / noise suppression
      MUST stay disabled — listen specifically for sub-bass and dialogue
      clarity).

## 7. Resource usage on MacBook Air M4

Open Activity Monitor + the Beamview process:

- [ ] CPU: < 15% average while actively streaming 1080p60.
- [ ] Real Memory: < 250 MB.
- [ ] No runaway GPU utilization (< 25% in Activity Monitor GPU tab).

## 8. Latency photo test

Target: **≤ 100 ms** end-to-end.

1. Open a millisecond timer web app (e.g. `https://stopwatch.onlinealarmkur.com/`)
   in Safari on an iPhone / second screen.
2. Point the iPhone display at the Switch's camera or screen-mirror
   it into the capture card source.
3. On the Mac running Beamview, the same timer is shown through the
   capture card.
4. Take a photo with a third phone that captures both the iPhone
   (source truth) and the Mac screen (Beamview output) in frame.
5. Read the two timer values. Difference = end-to-end latency.
6. Record the measurement in the release notes.

## 9. CI is green

- [ ] `main` branch shows a green CI run with `lint`, `typecheck`,
      `test-rust`, and `build-check` all passing.

## 10. DMG builds on `macos-latest` via GitHub Actions

- [ ] Pushing a `v*` tag triggers `.github/workflows/release.yml`.
- [ ] The workflow produces a draft GitHub Release with the `.dmg`
      attached.
- [ ] Download the `.dmg` from the draft release and repeat steps
      1 – 5 on a different Mac (or the same one after deleting the
      local config + re-installing).

---

## Phase 2 — Translation (v0.2.0)

Prerequisites (in addition to Phase 1 prerequisites above):

- Model files downloaded and installed at
  `~/Library/Application Support/com.beamview.Beamview/models/nllb-200-distilled-600M/`
- Game source material: a Nintendo Switch JRPG (Xenoblade Chronicles, Fire Emblem,
  or Persona recommended) with English subtitles

### T1. Setup — fresh install simulation

- [ ] Delete the `.ready` sentinel at
      `~/Library/Application Support/com.beamview.Beamview/models/nllb-200-distilled-600M/.ready`
      to simulate a first-run user (or leave intact to test an upgrade path).

### T2. First-run model download

- [ ] Open Settings → การแปล tab.
- [ ] Click "ดาวน์โหลดโมเดล" → `ModelDownloadModal` appears with a description of the ~650 MB download.
- [ ] Click "ดาวน์โหลดโมเดล" inside the modal → progress bar advances, bytes/total and % update in real time.
- [ ] Modal auto-closes when download completes → Toast "โมเดลพร้อมใช้งาน" appears briefly.

### T3. Region calibration

- [ ] In Settings → การแปล, click "เลือกพื้นที่ subtitle".
- [ ] `RegionSelector` overlay shows a frozen frame of the current video.
- [ ] Drag a rectangle over the game's dialog / subtitle area → click Save.
- [ ] Settings panel shows a small region preview; region coordinates are non-zero.

### T4. Cmd+T toggle

- [ ] With model installed and region set, press `Cmd+T` → `TranslationOverlay` appears with Thai text within ~1–2 s.
- [ ] Press `Cmd+T` again → overlay hides + Toast "การแปลปิดแล้ว".

### T5. Cmd+T gating when model not installed

- [ ] Delete the `.ready` sentinel (see T1), restart the app.
- [ ] Press `Cmd+T` → `ModelDownloadModal` opens instead of toggling translation.
- [ ] Cancel the modal; re-install the sentinel; restart to restore normal state.

### T6. Offline mode

- [ ] With the model installed, disable Wi-Fi / disconnect network.
- [ ] Restart the app → Settings → การแปล shows "พร้อมใช้งาน" status (no re-download required).
- [ ] Press `Cmd+T` → overlay appears with Thai text confirming offline inference works.

### T7. Cache behaviour

- [ ] Play through a cutscene with at least one line that repeats (e.g. a character name card or menu prompt).
- [ ] Open DevTools (right-click → Inspect) or tail `~/Library/Logs/com.beamview.app/beamview.log`.
- [ ] Confirm at least one `[translate] cache hit` log line with latency under 20 ms for the repeated line.
- [ ] After ~60 calls, confirm a summary log line:
      `[translate] last 60 calls: N translations / M cache hits (X%) / K duplicates — median latency Y ms`

### T8. Config persistence

- [ ] Enable translation, set FPS to 2, save Settings.
- [ ] Quit and relaunch the app.
- [ ] Settings → การแปล shows the same FPS value; translation is still enabled.
- [ ] Inspect `~/Library/Application Support/com.beamview.app/config.json` — confirm
      `"schema_version": 2` and a `"translation"` block with `"enabled": true` and `"fps": 2`.

### T9. Render-path regression check

- [ ] Run the Phase 1 latency photo test (§8 above) with translation **OFF** — record the baseline (should be ~51 ms).
- [ ] Enable translation at 1 fps, run the same latency photo test with translation **ON**.
- [ ] The delta between the two measurements must be under 2 ms.
      If it exceeds 2 ms, file a bug before releasing.

### T10. v1 config migration

- [ ] Replace `~/Library/Application Support/com.beamview.app/config.json` with a v1 shape:
      `{"schema_version":1,"theme":"system","welcome_dismissed":true}` (no `"translation"` key).
- [ ] Relaunch the app.
- [ ] Confirm `config.json` is rewritten with `"schema_version": 2` and a default `"translation"` block.
- [ ] Confirm no crash and translation settings UI is functional.

---

## Release workflow

1. Merge `feat/milestone-8-release-prep` to `main`.
2. Run this checklist end-to-end. Record the latency measurement.
3. `git tag v0.1.0 && git push origin v0.1.0`.
4. Watch the `release.yml` workflow succeed.
5. Edit the draft release notes to mention the measured latency
   and any known issues for this build.
6. Publish.

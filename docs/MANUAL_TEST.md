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

## Release workflow

1. Merge `feat/milestone-8-release-prep` to `main`.
2. Run this checklist end-to-end. Record the latency measurement.
3. `git tag v0.1.0 && git push origin v0.1.0`.
4. Watch the `release.yml` workflow succeed.
5. Edit the draft release notes to mention the measured latency
   and any known issues for this build.
6. Publish.

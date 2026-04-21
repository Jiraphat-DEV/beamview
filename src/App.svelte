<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { onDestroy, onMount } from 'svelte';
  import { commands } from '$lib/ipc';
  import { logger } from '$lib/logger';
  import * as hotkeys from '$lib/hotkeys/registry';
  import { devices } from '$lib/stores/devices.svelte';
  import { stream } from '$lib/stores/stream.svelte';
  import { theme } from '$lib/stores/theme.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import TitleBar from '$lib/components/TitleBar.svelte';
  import ActionBar from '$lib/components/ActionBar.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import VideoView from '$lib/components/VideoView.svelte';
  import DevicePicker from '$lib/components/DevicePicker.svelte';

  let pickerOpen = $state(false);
  let uninstallHotkeys: (() => void) | null = null;
  let unlistenPreferences: (() => void) | null = null;

  onMount(async () => {
    await theme.init();

    try {
      const version = await commands.getAppVersion();
      logger.info(`Beamview ${version} started`);
    } catch (err) {
      logger.warn('failed to fetch app version', { err: String(err) });
    }

    await devices.refresh();

    // Auto-restore the last-used device per spec §17.1 recommendation B.
    // WKWebView on macOS may rotate / withhold deviceIds before camera
    // permission is granted, so a saved ID can be "missing" from the
    // enum yet still valid for getUserMedia. We attempt acquire directly
    // and gracefully fall back to empty state on failure.
    try {
      const cfg = await commands.loadConfig();
      devices.restoreSelection(cfg.last_video_device_id, cfg.last_audio_device_id);

      if (cfg.last_video_device_id) {
        logger.info('attempting auto-acquire', {
          videoId: cfg.last_video_device_id,
          audioId: cfg.last_audio_device_id,
          enumSize: devices.video.length,
        });
        await stream.acquire(cfg.last_video_device_id, cfg.last_audio_device_id);
        if (stream.status === 'error') {
          logger.info('auto-acquire failed — falling back to empty state', {
            kind: stream.error?.kind,
            message: stream.error?.message,
          });
          stream.release();
        } else {
          await devices.refresh();
        }
      }
    } catch (err) {
      logger.warn('auto-acquire skipped', { err: String(err) });
    }

    // Install hotkeys after stores are ready so handlers see real state.
    uninstallHotkeys = installHotkeys();

    // Bridge the native menu's Preferences… event into the frontend —
    // the SettingsModal (Milestone 6) will subscribe to the same path.
    unlistenPreferences = await listen('menu://preferences', () => {
      logger.info('preferences event received (menu)');
      handleSettings();
    });
  });

  onDestroy(() => {
    uninstallHotkeys?.();
    unlistenPreferences?.();
  });

  // Keep <html data-theme> in sync with the resolved theme.
  $effect(() => {
    if (theme.ready) {
      document.documentElement.dataset.theme = theme.resolved;
    }
  });

  // Derived title bar label for the active device.
  let activeVideoLabel = $derived(
    stream.currentVideoId
      ? (devices.video.find((d) => d.deviceId === stream.currentVideoId)?.label ?? null)
      : null,
  );

  // TitleBar status shows fullscreen hint + mute state when relevant.
  let statusLabel = $derived(ui.muted ? 'muted' : null);

  async function toggleFullscreen(): Promise<void> {
    try {
      const next = await commands.toggleFullscreen();
      logger.info('fullscreen toggled', { fullscreen: next });
    } catch (err) {
      logger.warn('toggleFullscreen failed', { err: String(err) });
    }
  }

  async function exitFullscreenIfActive(): Promise<boolean> {
    try {
      const current = await commands.isFullscreen();
      if (!current) return false;
      await commands.toggleFullscreen();
      return true;
    } catch (err) {
      logger.warn('exitFullscreen failed', { err: String(err) });
      return false;
    }
  }

  function installHotkeys(): () => void {
    const uninstall = hotkeys.install();

    const unsub = [
      // Highest priority: close the top modal on Esc.
      hotkeys.register({
        id: 'esc-close-modal',
        priority: 20,
        match: (e) => {
          if (!hotkeys.isEscape(e) || !ui.modalOpen) return false;
          const top = ui.topModal;
          if (top === 'device-picker') pickerOpen = false;
          ui.popModal();
          return true;
        },
      }),
      // Next: Esc exits fullscreen.
      hotkeys.register({
        id: 'esc-exit-fullscreen',
        priority: 10,
        match: (e) => {
          if (!hotkeys.isEscape(e)) return false;
          void exitFullscreenIfActive();
          return true;
        },
      }),
      // Cmd+F / F11 toggle fullscreen.
      hotkeys.register({
        id: 'fullscreen',
        match: (e) => {
          if (hotkeys.isMetaKey(e, 'f') || hotkeys.isF11(e)) {
            void toggleFullscreen();
            return true;
          }
          return false;
        },
      }),
      // Cmd+M toggles mute.
      hotkeys.register({
        id: 'mute',
        match: (e) => {
          if (!hotkeys.isMetaKey(e, 'm')) return false;
          ui.toggleMute();
          logger.info('mute toggled', { muted: ui.muted });
          return true;
        },
      }),
      // Cmd+, opens settings (event bridge — SettingsModal lands in M6).
      hotkeys.register({
        id: 'preferences',
        match: (e) => {
          if (!hotkeys.isMetaKey(e, ',')) return false;
          handleSettings();
          return true;
        },
      }),
    ];

    return () => {
      for (const off of unsub) off();
      uninstall();
    };
  }

  function handleSettings() {
    // SettingsModal lands in Milestone 6. For now we just log so the
    // keybinding and menu bridge are both observable in the log file.
    logger.info('settings requested');
  }

  function handleChoose() {
    pickerOpen = true;
    ui.pushModal('device-picker');
  }

  async function handlePickerConfirm(videoId: string, audioId: string | null) {
    pickerOpen = false;
    ui.popModal('device-picker');
    await stream.acquire(videoId, audioId);
    if (stream.status === 'active') {
      await rememberDeviceSelection(videoId, audioId);
    }
  }

  function handlePickerClose() {
    pickerOpen = false;
    ui.popModal('device-picker');
  }

  /** Persist the in-use device IDs so the next launch can auto-acquire
   *  them (spec §17.1 recommendation B). These are resume hints rather
   *  than user settings — other config fields still require an explicit
   *  Save action from the SettingsModal (Milestone 6). */
  async function rememberDeviceSelection(videoId: string, audioId: string | null) {
    try {
      const cfg = await commands.loadConfig();
      await commands.saveConfig({
        ...cfg,
        last_video_device_id: videoId,
        last_audio_device_id: audioId,
      });
      logger.info('saved last-used device', { videoId, audioId });
    } catch (err) {
      logger.warn('failed to save last-used device', { err: String(err) });
    }
  }
</script>

<div class="shell">
  <TitleBar deviceLabel={activeVideoLabel} status={statusLabel} />
  <main class="main">
    {#if stream.status === 'active'}
      <VideoView />
    {:else if stream.status === 'error' && stream.error}
      <div class="error-panel">
        <p class="error-title">
          {stream.error.kind === 'disconnected' ? 'Device disconnected' : 'Unable to start stream'}
        </p>
        <p class="error-body">{stream.error.message}</p>
        <button type="button" class="error-action" onclick={handleChoose}>Choose device</button>
      </div>
    {:else}
      <EmptyState onChoose={handleChoose} />
    {/if}
  </main>
  <ActionBar onFullscreen={toggleFullscreen} onSettings={handleSettings} />
</div>

<DevicePicker open={pickerOpen} onConfirm={handlePickerConfirm} onClose={handlePickerClose} />

<style>
  .shell {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bv-bg);
    color: var(--bv-text);
  }

  .main {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--bv-video-bg);
  }

  .error-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--bv-space-3);
    color: var(--bv-text);
    background: var(--bv-bg);
    padding: var(--bv-space-12);
    text-align: center;
  }

  .error-title {
    font-family: var(--bv-font-display);
    font-size: 20px;
    font-weight: 300;
    letter-spacing: 0.5px;
    margin: 0;
    color: var(--bv-accent);
  }

  .error-body {
    color: var(--bv-text-muted);
    margin: 0;
    max-width: 420px;
    font-size: 13px;
    line-height: 1.5;
  }

  .error-action {
    margin-top: var(--bv-space-2);
    padding: 10px 20px;
    border: 1px solid var(--bv-accent);
    border-radius: 4px;
    color: var(--bv-accent);
    font-size: 13px;
    transition:
      background var(--bv-dur-fast) var(--bv-ease),
      color var(--bv-dur-fast) var(--bv-ease);
  }
  .error-action:hover {
    background: var(--bv-accent);
    color: var(--bv-paper);
  }
  :root[data-theme='dark'] .error-action:hover {
    color: var(--bv-ink-dark);
  }
</style>

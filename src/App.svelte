<script lang="ts">
  import { onMount } from 'svelte';
  import { commands } from '$lib/ipc';
  import { logger } from '$lib/logger';
  import { theme } from '$lib/stores/theme.svelte';
  import { devices } from '$lib/stores/devices.svelte';
  import { stream } from '$lib/stores/stream.svelte';
  import TitleBar from '$lib/components/TitleBar.svelte';
  import ActionBar from '$lib/components/ActionBar.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import VideoView from '$lib/components/VideoView.svelte';
  import DevicePicker from '$lib/components/DevicePicker.svelte';

  let pickerOpen = $state(false);

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
    // Only auto-acquire when labels are non-empty (= media permission
    // has been granted in a previous session) — otherwise we'd silently
    // fire a permission dialog before the user has context.
    try {
      const cfg = await commands.loadConfig();
      devices.restoreSelection(cfg.last_video_device_id, cfg.last_audio_device_id);

      const savedVideo = cfg.last_video_device_id
        ? devices.video.find((d) => d.deviceId === cfg.last_video_device_id)
        : null;
      if (savedVideo && savedVideo.label) {
        logger.info('auto-acquiring last-used device', { videoId: savedVideo.deviceId });
        await stream.acquire(savedVideo.deviceId, cfg.last_audio_device_id);
      }
    } catch (err) {
      logger.warn('auto-acquire skipped', { err: String(err) });
    }
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

  function handleFullscreen() {
    // Milestone 5 wires this to commands.toggleFullscreen()
    logger.info('fullscreen requested');
  }
  function handleSettings() {
    // Milestone 6 wires this to the SettingsModal
    logger.info('settings requested');
  }
  function handleChoose() {
    pickerOpen = true;
  }
  async function handlePickerConfirm(videoId: string, audioId: string | null) {
    pickerOpen = false;
    await stream.acquire(videoId, audioId);
    if (stream.status === 'active') {
      await rememberDeviceSelection(videoId, audioId);
    }
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
  function handlePickerClose() {
    pickerOpen = false;
  }
</script>

<div class="shell">
  <TitleBar deviceLabel={activeVideoLabel} />
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
  <ActionBar onFullscreen={handleFullscreen} onSettings={handleSettings} />
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

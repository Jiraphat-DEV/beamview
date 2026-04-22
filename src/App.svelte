<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { onDestroy, onMount } from 'svelte';
  import { commands } from '$lib/ipc';
  import type { AppConfig } from '$lib/ipc';
  import { logger } from '$lib/logger';
  import * as hotkeys from '$lib/hotkeys/registry';
  import { requestPermission } from '$lib/capture/devices';
  import { devices } from '$lib/stores/devices.svelte';
  import { stream } from '$lib/stores/stream.svelte';
  import { theme } from '$lib/stores/theme.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import { translation } from '$lib/stores/translation.svelte';
  import TitleBar from '$lib/components/TitleBar.svelte';
  import ActionBar from '$lib/components/ActionBar.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import ErrorOverlay from '$lib/components/ErrorOverlay.svelte';
  import LoadingState from '$lib/components/LoadingState.svelte';
  import VideoView from '$lib/components/VideoView.svelte';
  import DevicePicker from '$lib/components/DevicePicker.svelte';
  import SettingsModal from '$lib/components/SettingsModal.svelte';
  import ModelDownloadModal from '$lib/components/ModelDownloadModal.svelte';
  import Toast from '$lib/components/Toast.svelte';
  import WelcomeScreen from '$lib/components/WelcomeScreen.svelte';

  let pickerOpen = $state(false);
  let settingsOpen = $state(false);
  let showModelDownload = $state(false);
  let grantRequesting = $state(false);
  let config = $state<AppConfig | null>(null);
  /** The <video> element from VideoView — used by RegionSelector. */
  let videoEl = $state<HTMLVideoElement | null>(null);
  let uninstallHotkeys: (() => void) | null = null;
  let unlistenPreferences: (() => void) | null = null;
  let unlistenTranslationToggle: (() => void) | null = null;

  // Auto-hide chrome after 2s of inactivity while the stream is playing
  // and no modal is open (spec §5.4.1). Any mouse move resets the timer
  // and pops the bars back in.
  const CHROME_HIDE_MS = 2000;
  let chromeHidden = $state(false);
  let hideTimer: ReturnType<typeof setTimeout> | null = null;

  function clearHideTimer() {
    if (hideTimer !== null) {
      clearTimeout(hideTimer);
      hideTimer = null;
    }
  }

  function scheduleHide() {
    clearHideTimer();
    hideTimer = setTimeout(() => {
      chromeHidden = true;
    }, CHROME_HIDE_MS);
  }

  function handlePointerActivity() {
    chromeHidden = false;
    if (stream.status === 'active' && !ui.modalOpen) {
      scheduleHide();
    }
  }

  // Show Welcome until config is loaded AND user has dismissed it.
  const showWelcome = $derived(config !== null && !config.welcome_dismissed);

  onMount(async () => {
    // Install hotkeys + menu bridge before async work so a later failure
    // can't silently swallow Cmd+F / Cmd+, etc.
    try {
      uninstallHotkeys = installHotkeys();
      logger.info('hotkeys installed');
    } catch (err) {
      logger.error('failed to install hotkeys', { err: String(err) });
    }

    try {
      unlistenPreferences = await listen('menu://preferences', () => {
        logger.info('preferences event received (menu)');
        handleSettings();
      });
      logger.info('preferences event listener registered');
    } catch (err) {
      logger.error('failed to register preferences listener', { err: String(err) });
    }

    try {
      unlistenTranslationToggle = await listen('menu://translation-toggle', () => {
        logger.info('translation-toggle event received (menu)');
        handleTranslationToggle();
      });
      logger.info('translation-toggle event listener registered');
    } catch (err) {
      logger.error('failed to register translation-toggle listener', { err: String(err) });
    }

    try {
      await theme.init();
    } catch (err) {
      logger.warn('theme init failed', { err: String(err) });
    }

    try {
      const version = await commands.getAppVersion();
      logger.info(`Beamview ${version} started`);
    } catch (err) {
      logger.warn('failed to fetch app version', { err: String(err) });
    }

    await devices.refresh();

    try {
      const cfg = await commands.loadConfig();
      config = cfg;
      devices.restoreSelection(cfg.last_video_device_id, cfg.last_audio_device_id);

      // Hydrate translation store from persisted config (M4).
      if (cfg.translation) {
        translation.enabled = cfg.translation.enabled;
        translation.fps = cfg.translation.fps ?? 1.0;
        translation.showEnglishCaption = cfg.translation.show_english_caption;
        // `subtitle_position` was added after M5 initial testing; older
        // v2 configs that predate it will have `undefined` here, in which
        // case keep the store's default (`panel_below`).
        if (cfg.translation.subtitle_position) {
          translation.subtitlePosition = cfg.translation.subtitle_position;
        }
        if (cfg.translation.region) {
          translation.setRegion(cfg.translation.region);
        }
      }

      // Only attempt auto-acquire once the welcome flow has been
      // completed — firing getUserMedia before the user has seen the
      // explanation is jarring (spec §8.1).
      if (cfg.welcome_dismissed && cfg.last_video_device_id) {
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
      logger.warn('startup config load failed', { err: String(err) });
    }
  });

  onDestroy(() => {
    uninstallHotkeys?.();
    unlistenPreferences?.();
    unlistenTranslationToggle?.();
    clearHideTimer();
    window.removeEventListener('mousemove', handlePointerActivity);
    // Carry-over C: tear down the progress event listener here (app lifetime)
    // rather than in VideoView.svelte (stream lifetime).
    translation.destroy();
  });

  // Track pointer activity globally so moving the mouse anywhere in the
  // window reveals the chrome. Attaching after mount so SSR / test
  // environments without a window don't crash.
  onMount(() => {
    window.addEventListener('mousemove', handlePointerActivity, { passive: true });
  });

  // React to status + modal changes: if we shouldn't auto-hide, clear
  // the timer and force the chrome visible. If we should, start timing.
  $effect(() => {
    const shouldAutoHide = stream.status === 'active' && !ui.modalOpen;
    if (shouldAutoHide) {
      scheduleHide();
    } else {
      clearHideTimer();
      chromeHidden = false;
    }
  });

  // Sync showModelDownload with the modal stack — the ModelDownloadModal
  // pops itself via ui.popModal('model-download') when it auto-closes.
  $effect(() => {
    if (showModelDownload && !ui.modalStack.includes('model-download')) {
      showModelDownload = false;
    }
  });

  // Keep <html data-theme> in sync with the resolved theme.
  $effect(() => {
    if (theme.ready) {
      document.documentElement.dataset.theme = theme.resolved;
    }
  });

  let activeVideoLabel = $derived(
    stream.currentVideoId
      ? (devices.video.find((d) => d.deviceId === stream.currentVideoId)?.label ?? null)
      : null,
  );

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
      hotkeys.register({
        id: 'esc-close-modal',
        priority: 20,
        match: (e) => {
          if (!hotkeys.isEscape(e) || !ui.modalOpen) return false;
          const top = ui.topModal;
          if (top === 'device-picker') pickerOpen = false;
          if (top === 'settings') settingsOpen = false;
          if (top === 'model-download') showModelDownload = false;
          // 'region-selector' pops itself via ui.popModal — no extra flag needed here
          ui.popModal();
          return true;
        },
      }),
      hotkeys.register({
        id: 'esc-exit-fullscreen',
        priority: 10,
        match: (e) => {
          if (!hotkeys.isEscape(e)) return false;
          void exitFullscreenIfActive();
          return true;
        },
      }),
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
      hotkeys.register({
        id: 'mute',
        match: (e) => {
          if (!hotkeys.isMetaKey(e, 'm')) return false;
          ui.toggleMute();
          logger.info('mute toggled', { muted: ui.muted });
          return true;
        },
      }),
      hotkeys.register({
        id: 'preferences',
        match: (e) => {
          if (!hotkeys.isMetaKey(e, ',')) return false;
          handleSettings();
          return true;
        },
      }),
      hotkeys.register({
        id: 'translation-toggle',
        priority: 15,
        match: (e) => {
          if (!hotkeys.isMetaKey(e, 't')) return false;
          handleTranslationToggle();
          return true;
        },
      }),
    ];

    return () => {
      for (const off of unsub) off();
      uninstall();
    };
  }

  /**
   * Toggle translation on/off via Cmd+T or the Translation menu item.
   *
   * If the model is not ready → open the ModelDownloadModal instead of
   * toggling, since translation cannot run without the model.
   */
  function handleTranslationToggle() {
    if (translation.modelStatus.type !== 'ready') {
      showModelDownload = true;
      ui.pushModal('model-download');
      logger.info('translation toggle — model not ready, opening download modal');
      return;
    }
    translation.toggle();
    ui.showToast(translation.enabled ? 'การแปลเปิดอยู่' : 'การแปลปิดแล้ว', 'info');
    logger.info('translation toggled', { enabled: translation.enabled });
  }

  function handleSettings() {
    logger.info('settings opened');
    settingsOpen = true;
    ui.pushModal('settings');
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

  function handleSettingsClose() {
    settingsOpen = false;
    ui.popModal('settings');
  }

  async function handleSettingsSave(newCfg: AppConfig, deviceChanged: boolean) {
    try {
      await commands.saveConfig(newCfg);
      config = newCfg;
      theme.set(newCfg.theme);
      // The SettingsModal already syncs the store before calling onSave,
      // but we re-apply here as a safety net in case config was loaded from
      // a persistent source with different values.
      if (newCfg.translation) {
        translation.fps = newCfg.translation.fps ?? 1.0;
        translation.showEnglishCaption = newCfg.translation.show_english_caption;
        if (newCfg.translation.subtitle_position) {
          translation.subtitlePosition = newCfg.translation.subtitle_position;
        }
        if (newCfg.translation.region) translation.setRegion(newCfg.translation.region);
      }
      ui.showToast('Settings saved', 'success');
      logger.info('settings saved', { deviceChanged });

      if (deviceChanged && newCfg.last_video_device_id) {
        await stream.acquire(newCfg.last_video_device_id, newCfg.last_audio_device_id);
        if (stream.status === 'error') {
          ui.showToast(stream.error?.message ?? 'Failed to switch device', 'error');
        } else {
          await devices.refresh();
        }
      } else if (deviceChanged && !newCfg.last_video_device_id) {
        stream.release();
      }

      settingsOpen = false;
      ui.popModal('settings');
    } catch (err) {
      logger.error('settings save failed', { err: String(err) });
      ui.showToast('Failed to save settings', 'error');
    }
  }

  async function rememberDeviceSelection(videoId: string, audioId: string | null) {
    try {
      const cfg = config ?? (await commands.loadConfig());
      const next: AppConfig = {
        ...cfg,
        last_video_device_id: videoId,
        last_audio_device_id: audioId,
      };
      await commands.saveConfig(next);
      config = next;
      logger.info('saved last-used device', { videoId, audioId });
    } catch (err) {
      logger.warn('failed to save last-used device', { err: String(err) });
    }
  }

  async function handleWelcomeGrant() {
    grantRequesting = true;
    try {
      const granted = await requestPermission();
      logger.info('welcome permission probe', { granted });
      await devices.refresh();
      await dismissWelcome();
    } finally {
      grantRequesting = false;
    }
  }

  async function handleWelcomeSkip() {
    await dismissWelcome();
  }

  async function dismissWelcome() {
    if (!config) return;
    try {
      const next: AppConfig = { ...config, welcome_dismissed: true };
      await commands.saveConfig(next);
      config = next;
      logger.info('welcome dismissed');
    } catch (err) {
      logger.warn('failed to persist welcome_dismissed', { err: String(err) });
      // Fail-open: show main UI anyway so the user isn't stuck.
      config = { ...config, welcome_dismissed: true };
    }
  }

  function reopenChoose() {
    handleChoose();
  }
</script>

<div class="shell" class:chrome-hidden={chromeHidden}>
  <div class="chrome chrome-top">
    <TitleBar deviceLabel={activeVideoLabel} status={statusLabel} />
  </div>
  <main class="main">
    {#if showWelcome}
      <WelcomeScreen
        onGrant={handleWelcomeGrant}
        onSkip={handleWelcomeSkip}
        requesting={grantRequesting}
      />
    {:else if stream.status === 'active'}
      <VideoView bind:videoEl />
    {:else if stream.status === 'acquiring'}
      <LoadingState />
    {:else if stream.status === 'error' && stream.error}
      <ErrorOverlay
        title={stream.error.kind === 'disconnected'
          ? 'Device disconnected'
          : 'Unable to start stream'}
        message={stream.error.message}
        primary={{ label: 'Choose device', onClick: reopenChoose }}
      />
    {:else}
      <EmptyState onChoose={handleChoose} />
    {/if}
  </main>
  <div class="chrome chrome-bottom">
    <ActionBar onFullscreen={toggleFullscreen} onSettings={handleSettings} />
  </div>
</div>

<DevicePicker open={pickerOpen} onConfirm={handlePickerConfirm} onClose={handlePickerClose} />

{#if config}
  <SettingsModal
    open={settingsOpen}
    {config}
    onSave={handleSettingsSave}
    onClose={handleSettingsClose}
    {videoEl}
  />
{/if}

{#if showModelDownload}
  <ModelDownloadModal />
{/if}

<Toast />

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

  .chrome {
    transition:
      opacity var(--bv-dur-med) var(--bv-ease),
      transform var(--bv-dur-med) var(--bv-ease);
  }

  .chrome-hidden .chrome-top {
    opacity: 0;
    transform: translateY(-100%);
    pointer-events: none;
  }
  .chrome-hidden .chrome-bottom {
    opacity: 0;
    transform: translateY(100%);
    pointer-events: none;
  }
</style>

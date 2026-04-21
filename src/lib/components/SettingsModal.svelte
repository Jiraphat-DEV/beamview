<script lang="ts">
  import { X } from 'lucide-svelte';
  import { onMount } from 'svelte';
  import { commands } from '$lib/ipc';
  import type { AppConfig, Theme } from '$lib/ipc';
  import { displayLabel } from '$lib/capture/devices';
  import { devices } from '$lib/stores/devices.svelte';
  import { ui } from '$lib/stores/ui.svelte';

  // Modal with Video / Audio / About tabs.
  //
  // Follows the "explicit save only" pattern (spec §1.4, §8.6): a
  // local copy of the editable fields is compared against `config`
  // to drive the dirty flag. Save fires `onSave(newCfg, deviceChanged)`
  // so the parent can re-acquire the stream + persist the config.
  // Cancel (or Esc via the native dialog) discards local edits.
  //
  // Volume + mute are live-applied through ui store rather than waiting
  // for save — those aren't persisted to config Phase 1 (session only)
  // and users expect the knob to respond immediately.

  interface Props {
    open: boolean;
    config: AppConfig;
    onSave: (cfg: AppConfig, deviceChanged: boolean) => void | Promise<void>;
    onClose: () => void;
  }

  let { open, config, onSave, onClose }: Props = $props();

  type Tab = 'video' | 'audio' | 'about';
  let activeTab = $state<Tab>('video');

  let formVideoId = $state<string | null>(null);
  let formAudioId = $state<string | null>(null);
  let formTheme = $state<Theme>('system');
  let saving = $state(false);
  let version = $state<string>('');

  let dialogEl = $state<HTMLDialogElement | null>(null);

  onMount(async () => {
    try {
      version = await commands.getAppVersion();
    } catch {
      version = '—';
    }
  });

  // Sync local form state with the latest config whenever the modal
  // opens. Using an $effect tied to `open` means reopening the modal
  // after an external save reflects the fresh baseline immediately.
  $effect(() => {
    const el = dialogEl;
    if (!el) return;
    if (open && !el.open) {
      formVideoId = config.last_video_device_id;
      formAudioId = config.last_audio_device_id;
      formTheme = config.theme;
      activeTab = 'video';
      el.showModal();
    }
    if (!open && el.open) el.close();
  });

  const deviceChanged = $derived(
    formVideoId !== config.last_video_device_id || formAudioId !== config.last_audio_device_id,
  );
  const dirty = $derived(deviceChanged || formTheme !== config.theme);

  async function handleSave() {
    saving = true;
    try {
      const newCfg: AppConfig = {
        ...config,
        last_video_device_id: formVideoId,
        last_audio_device_id: formAudioId,
        theme: formTheme,
      };
      await onSave(newCfg, deviceChanged);
    } finally {
      saving = false;
    }
  }

  function handleCancel() {
    onClose();
  }

  function handleDialogClose() {
    if (open) handleCancel();
  }
</script>

<dialog bind:this={dialogEl} class="modal" onclose={handleDialogClose}>
  <header class="header">
    <h2>Settings</h2>
    <button type="button" class="close" aria-label="Close" onclick={handleCancel}>
      <X size={16} strokeWidth={1.5} />
    </button>
  </header>

  <nav class="tabs" aria-label="Settings tabs">
    <button
      type="button"
      class="tab"
      class:active={activeTab === 'video'}
      onclick={() => (activeTab = 'video')}
    >
      Video
    </button>
    <button
      type="button"
      class="tab"
      class:active={activeTab === 'audio'}
      onclick={() => (activeTab = 'audio')}
    >
      Audio
    </button>
    <button
      type="button"
      class="tab"
      class:active={activeTab === 'about'}
      onclick={() => (activeTab = 'about')}
    >
      About
    </button>
  </nav>

  <div class="body">
    {#if activeTab === 'video'}
      <label class="field">
        <span class="label">Capture device</span>
        <select bind:value={formVideoId}>
          <option value={null}>— None —</option>
          {#each devices.video as d (d.deviceId)}
            <option value={d.deviceId}>{displayLabel(d)}</option>
          {/each}
        </select>
      </label>
      <p class="hint">Changing the device reconnects the stream after Save.</p>
    {:else if activeTab === 'audio'}
      <label class="field">
        <span class="label">Audio input</span>
        <select bind:value={formAudioId}>
          <option value={null}>Disabled</option>
          {#each devices.audio as d (d.deviceId)}
            <option value={d.deviceId}>{displayLabel(d)}</option>
          {/each}
        </select>
      </label>

      <div class="field">
        <span class="label">Volume</span>
        <div class="slider-row">
          <input
            type="range"
            min="0"
            max="100"
            value={Math.round(ui.volume * 100)}
            oninput={(e) => ui.setVolume(Number((e.currentTarget as HTMLInputElement).value) / 100)}
            disabled={ui.muted}
          />
          <span class="value bv-mono">{ui.muted ? 'muted' : `${Math.round(ui.volume * 100)}%`}</span
          >
        </div>
      </div>

      <label class="toggle">
        <input type="checkbox" checked={ui.muted} onchange={() => ui.toggleMute()} />
        <span>Mute audio</span>
      </label>
      <p class="hint">Volume and mute apply immediately and persist for the session only.</p>
    {:else}
      <section class="about">
        <p class="about-name">Beamview</p>
        <p class="about-tagline">Beam your game. See it instantly.</p>

        <dl class="meta">
          <dt>Version</dt>
          <dd class="bv-mono">{version}</dd>
          <dt>License</dt>
          <dd>MIT © 2026 Jiraphat</dd>
        </dl>

        <label class="field">
          <span class="label">Theme</span>
          <select bind:value={formTheme}>
            <option value="system">Follow system</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </label>
      </section>
    {/if}
  </div>

  <footer class="footer">
    <span class="dirty-hint">
      {#if dirty}Unsaved changes{/if}
    </span>
    <div class="spacer"></div>
    <button type="button" class="btn ghost" onclick={handleCancel} disabled={saving}>
      Cancel
    </button>
    <button type="button" class="btn accent" onclick={handleSave} disabled={!dirty || saving}>
      {saving ? 'Saving…' : 'Save'}
    </button>
  </footer>
</dialog>

<style>
  .modal {
    background: var(--bv-surface);
    color: var(--bv-text);
    border: 1px solid var(--bv-border);
    border-radius: 8px;
    padding: 0;
    min-width: 520px;
    max-width: 620px;
    font-family: var(--bv-font-body);
  }
  .modal::backdrop {
    background: rgba(0, 0, 0, 0.4);
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--bv-space-4) var(--bv-space-6);
    border-bottom: 1px solid var(--bv-border);
  }
  h2 {
    font-size: 18px;
    font-weight: 400;
    letter-spacing: 0.3px;
    margin: 0;
  }
  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 4px;
    color: var(--bv-text-muted);
  }
  .close:hover {
    background: color-mix(in srgb, var(--bv-text) 8%, transparent);
    color: var(--bv-text);
  }

  .tabs {
    display: flex;
    gap: var(--bv-space-1);
    padding: 0 var(--bv-space-6);
    border-bottom: 1px solid var(--bv-border);
  }
  .tab {
    padding: 10px 14px;
    font-size: 13px;
    color: var(--bv-text-muted);
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    transition: color var(--bv-dur-fast) var(--bv-ease);
  }
  .tab:hover {
    color: var(--bv-text);
  }
  .tab.active {
    color: var(--bv-text);
    border-bottom-color: var(--bv-accent);
  }

  .body {
    padding: var(--bv-space-6);
    min-height: 220px;
  }

  .field {
    display: block;
    margin-bottom: var(--bv-space-4);
  }
  .label {
    display: block;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--bv-text-subtle);
    margin-bottom: var(--bv-space-1);
  }
  select,
  input[type='range'] {
    width: 100%;
    padding: 8px 10px;
    background: var(--bv-bg);
    color: var(--bv-text);
    border: 1px solid var(--bv-border);
    border-radius: 4px;
    font-family: inherit;
    font-size: 13px;
  }
  input[type='range'] {
    padding: 0;
    accent-color: var(--bv-accent);
  }

  .slider-row {
    display: flex;
    align-items: center;
    gap: var(--bv-space-3);
  }
  .slider-row .value {
    font-size: 12px;
    color: var(--bv-text-muted);
    min-width: 48px;
    text-align: right;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: var(--bv-space-2);
    font-size: 13px;
    color: var(--bv-text);
    margin-bottom: var(--bv-space-2);
  }

  .hint {
    color: var(--bv-text-subtle);
    font-size: 11px;
    margin: 0;
  }

  .about-name {
    font-family: var(--bv-font-display);
    font-weight: 300;
    font-size: 24px;
    letter-spacing: 1px;
    margin: 0;
  }
  .about-tagline {
    color: var(--bv-text-muted);
    font-size: 13px;
    margin: var(--bv-space-1) 0 var(--bv-space-5);
  }
  .meta {
    display: grid;
    grid-template-columns: 90px 1fr;
    gap: 6px 12px;
    font-size: 13px;
    margin: 0 0 var(--bv-space-5);
  }
  .meta dt {
    color: var(--bv-text-subtle);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    padding-top: 1px;
  }
  .meta dd {
    margin: 0;
    color: var(--bv-text);
  }

  .footer {
    display: flex;
    align-items: center;
    gap: var(--bv-space-2);
    padding: var(--bv-space-4) var(--bv-space-6);
    border-top: 1px solid var(--bv-border);
  }
  .dirty-hint {
    font-size: 11px;
    color: var(--bv-accent);
    text-transform: uppercase;
    letter-spacing: 1px;
  }
  .spacer {
    flex: 1;
  }

  .btn {
    padding: 8px 16px;
    border: 1px solid var(--bv-border);
    border-radius: 4px;
    background: transparent;
    color: var(--bv-text);
    font-size: 13px;
    transition:
      background var(--bv-dur-fast) var(--bv-ease),
      color var(--bv-dur-fast) var(--bv-ease);
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .btn.ghost {
    color: var(--bv-text-muted);
  }
  .btn.ghost:hover:not(:disabled) {
    background: color-mix(in srgb, var(--bv-text) 6%, transparent);
    color: var(--bv-text);
  }
  .btn.accent {
    background: var(--bv-accent);
    border-color: var(--bv-accent);
    color: var(--bv-paper);
  }
  :root[data-theme='dark'] .btn.accent {
    color: var(--bv-ink-dark);
  }
  .btn.accent:hover:not(:disabled) {
    background: color-mix(in srgb, var(--bv-accent) 90%, black);
  }
</style>

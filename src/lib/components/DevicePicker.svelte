<script lang="ts">
  import { RefreshCcw } from 'lucide-svelte';
  import { displayLabel, requestPermission } from '$lib/capture/devices';
  import { devices } from '$lib/stores/devices.svelte';
  import { logger } from '$lib/logger';

  interface Props {
    open: boolean;
    onConfirm: (videoId: string, audioId: string | null) => void;
    onClose: () => void;
  }

  let { open, onConfirm, onClose }: Props = $props();

  let dialogEl = $state<HTMLDialogElement | null>(null);
  let requesting = $state(false);

  // Devices list is considered "missing permission" when there are no
  // video devices OR every returned device has no label (macOS hides
  // labels behind the permission prompt).
  const needsPermission = $derived(
    devices.ready && (devices.video.length === 0 || devices.video.every((d) => d.label === null)),
  );

  $effect(() => {
    const el = dialogEl;
    if (!el) return;
    if (open && !el.open) el.showModal();
    if (!open && el.open) el.close();
  });

  async function grantAccess() {
    requesting = true;
    try {
      const granted = await requestPermission();
      logger.info('permission probe result', { granted });
      await devices.refresh();
    } finally {
      requesting = false;
    }
  }

  async function refresh() {
    await devices.refresh();
  }

  function handleConfirm() {
    if (!devices.videoId) return;
    onConfirm(devices.videoId, devices.audioId);
  }

  function handleDialogClose() {
    // Fires for Esc, backdrop (via form method=dialog), or programmatic close.
    if (open) onClose();
  }

  function handleBackdropClick(event: MouseEvent) {
    if (event.target === dialogEl) onClose();
  }
</script>

<dialog
  bind:this={dialogEl}
  class="picker"
  onclose={handleDialogClose}
  onclick={handleBackdropClick}
>
  <header>
    <h2>Select capture device</h2>
  </header>

  {#if !devices.ready}
    <p class="muted">Looking for devices…</p>
  {:else if needsPermission}
    <p class="muted">Beamview needs camera and microphone permission to list your devices.</p>
    <button class="btn primary full" onclick={grantAccess} disabled={requesting}>
      {requesting ? 'Requesting access…' : 'Grant access'}
    </button>
  {:else}
    <label class="field">
      <span class="label">Video</span>
      <select bind:value={devices.videoId}>
        {#each devices.video as d (d.deviceId)}
          <option value={d.deviceId}>{displayLabel(d)}</option>
        {/each}
      </select>
    </label>

    <label class="field">
      <span class="label">Audio</span>
      <select bind:value={devices.audioId}>
        <option value={null}>Disabled</option>
        {#each devices.audio as d (d.deviceId)}
          <option value={d.deviceId}>{displayLabel(d)}</option>
        {/each}
      </select>
    </label>
  {/if}

  <footer>
    <button class="btn ghost" onclick={refresh} aria-label="Refresh devices">
      <RefreshCcw size={14} strokeWidth={1.5} /> Refresh
    </button>
    <div class="spacer"></div>
    <button class="btn" onclick={onClose}>Cancel</button>
    <button class="btn primary" onclick={handleConfirm} disabled={!devices.videoId}>
      Confirm
    </button>
  </footer>
</dialog>

<style>
  .picker {
    background: var(--bv-surface);
    color: var(--bv-text);
    border: 1px solid var(--bv-border);
    border-radius: 8px;
    padding: var(--bv-space-6);
    min-width: 420px;
    max-width: 540px;
    font-family: var(--bv-font-body);
  }
  .picker::backdrop {
    background: rgba(0, 0, 0, 0.4);
  }

  header {
    margin-bottom: var(--bv-space-4);
  }
  h2 {
    font-size: 18px;
    font-weight: 400;
    letter-spacing: 0.3px;
    margin: 0;
  }

  .muted {
    color: var(--bv-text-muted);
    margin: var(--bv-space-3) 0 var(--bv-space-4);
    font-size: 13px;
    line-height: 1.5;
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
  select {
    width: 100%;
    padding: 8px 10px;
    background: var(--bv-bg);
    color: var(--bv-text);
    border: 1px solid var(--bv-border);
    border-radius: 4px;
    font-family: inherit;
    font-size: 13px;
  }

  footer {
    display: flex;
    align-items: center;
    gap: var(--bv-space-2);
    margin-top: var(--bv-space-4);
  }
  .spacer {
    flex: 1;
  }

  .btn {
    padding: 8px 14px;
    border: 1px solid var(--bv-border);
    border-radius: 4px;
    background: transparent;
    color: var(--bv-text);
    font-size: 13px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    transition:
      background var(--bv-dur-fast) var(--bv-ease),
      border-color var(--bv-dur-fast) var(--bv-ease);
  }
  .btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--bv-text) 6%, transparent);
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .btn.primary {
    background: var(--bv-accent);
    border-color: var(--bv-accent);
    color: var(--bv-paper);
  }
  :root[data-theme='dark'] .btn.primary {
    color: var(--bv-ink-dark);
  }
  .btn.primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--bv-accent) 90%, black);
  }
  .btn.ghost {
    border-color: transparent;
    color: var(--bv-text-muted);
  }
  .btn.full {
    width: 100%;
    justify-content: center;
  }
</style>

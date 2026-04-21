<script lang="ts">
  // Thin app-level strip that sits below the native Tauri title bar.
  // Phase 1 shows the active device label on the left and a status
  // region (fps/latency) on the right. Both accept null so empty state
  // renders gracefully before a stream is acquired.
  //
  // Auto-hide behaviour (spec §5.4.1 "Title bar — auto-hide 2s") lands
  // in Milestone 7 polish.
  interface Props {
    deviceLabel?: string | null;
    status?: string | null;
  }

  let { deviceLabel = null, status = null }: Props = $props();
</script>

<header class="titlebar">
  <span class="device">
    {deviceLabel ?? 'No device selected'}
  </span>
  <span class="status bv-mono">{status ?? '—'}</span>
</header>

<style>
  .titlebar {
    height: var(--bv-titlebar-h);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 var(--bv-space-4);
    border-bottom: 1px solid var(--bv-border);
    background: var(--bv-surface);
    color: var(--bv-text);
    flex-shrink: 0;
    user-select: none;
  }

  .device {
    font-size: 13px;
    color: var(--bv-text-muted);
    letter-spacing: 0.3px;
  }

  .status {
    font-size: 11px;
    color: var(--bv-text-subtle);
  }
</style>

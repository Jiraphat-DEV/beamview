<script lang="ts">
  import { Maximize, Settings } from 'lucide-svelte';

  // Bottom action bar.
  //
  // Phase 1 only renders Fullscreen + Settings — Record + Screenshot are
  // Phase 2 features per spec §9. Callbacks are optional props so the
  // layout renders cleanly in Milestone 3 before the hotkey and settings
  // stores exist to drive them (Milestones 5 + 6).
  interface Props {
    onFullscreen?: () => void;
    onSettings?: () => void;
  }

  let { onFullscreen, onSettings }: Props = $props();
</script>

<footer class="actionbar">
  <!-- Phase 2 will put Record + Screenshot here. -->
  <div class="group"></div>

  <div class="group">
    <button type="button" class="icon-btn" aria-label="Fullscreen" onclick={onFullscreen}>
      <Maximize size={18} strokeWidth={1.5} />
    </button>
    <button type="button" class="icon-btn" aria-label="Settings" onclick={onSettings}>
      <Settings size={18} strokeWidth={1.5} />
    </button>
  </div>
</footer>

<style>
  .actionbar {
    height: var(--bv-actionbar-h);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 var(--bv-space-3);
    border-top: 1px solid var(--bv-border);
    background: var(--bv-surface);
    flex-shrink: 0;
  }

  .group {
    display: flex;
    gap: var(--bv-space-1);
  }

  .icon-btn {
    width: 32px;
    height: 32px;
    border-radius: 6px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--bv-text-muted);
    transition:
      background var(--bv-dur-fast) var(--bv-ease),
      color var(--bv-dur-fast) var(--bv-ease);
  }

  .icon-btn:hover {
    background: color-mix(in srgb, var(--bv-text) 8%, transparent);
    color: var(--bv-text);
  }

  .icon-btn:active {
    background: color-mix(in srgb, var(--bv-text) 12%, transparent);
  }
</style>

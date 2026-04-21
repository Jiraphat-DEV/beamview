<script lang="ts">
  // Shown when the video area has no active stream.
  //
  // Minimal Japanese: lots of Ma (whitespace), single Vermilion accent on
  // the primary action, no illustrations. Headline copy per spec §3.8
  // voice guide ("Select a device to begin.").
  //
  // The `onChoose` callback will be wired to the DevicePicker in
  // Milestone 4; for Milestone 3 it is simply optional so this component
  // is useful inside pnpm tauri dev as a visual smoke test.
  interface Props {
    onChoose?: () => void;
  }

  let { onChoose }: Props = $props();
</script>

<section class="empty" aria-live="polite">
  <p class="wordmark">beamview</p>
  <p class="message">Select a device to begin.</p>
  <button type="button" class="cta" onclick={onChoose}> Choose device </button>
</section>

<style>
  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--bv-space-4);
    color: var(--bv-text);
    background: var(--bv-bg);
    padding: var(--bv-space-12);
  }

  .wordmark {
    font-family: var(--bv-font-display);
    font-weight: 300;
    font-size: 28px;
    letter-spacing: 2px;
    color: var(--bv-text);
    margin: 0;
  }

  .message {
    font-size: 14px;
    color: var(--bv-text-muted);
    margin-bottom: var(--bv-space-4);
  }

  .cta {
    padding: 10px 20px;
    border: 1px solid var(--bv-accent);
    border-radius: 4px;
    color: var(--bv-accent);
    font-size: 13px;
    font-weight: 400;
    letter-spacing: 0.3px;
    transition:
      background var(--bv-dur-fast) var(--bv-ease),
      color var(--bv-dur-fast) var(--bv-ease);
  }

  .cta:hover {
    background: var(--bv-accent);
    color: var(--bv-paper);
  }

  :root[data-theme='dark'] .cta:hover {
    color: var(--bv-ink-dark);
  }
</style>

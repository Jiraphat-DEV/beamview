<script lang="ts">
  // Blocking error panel (spec §8.7 pattern C). Filled fullscreen by
  // its parent's flex layout; the main area swaps this in instead of
  // VideoView / EmptyState when stream.status === 'error'. Keep the
  // copy concise per brand voice §3.8.
  interface Action {
    label: string;
    primary?: boolean;
    onClick: () => void;
  }

  interface Props {
    title: string;
    message: string;
    primary?: Action;
    secondary?: Action;
  }

  let { title, message, primary, secondary }: Props = $props();
</script>

<section class="overlay" role="alert">
  <p class="title">{title}</p>
  <p class="message">{message}</p>
  {#if primary || secondary}
    <div class="actions">
      {#if secondary}
        <button type="button" class="btn ghost" onclick={secondary.onClick}>
          {secondary.label}
        </button>
      {/if}
      {#if primary}
        <button type="button" class="btn accent" onclick={primary.onClick}>
          {primary.label}
        </button>
      {/if}
    </div>
  {/if}
</section>

<style>
  .overlay {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--bv-space-3);
    background: var(--bv-bg);
    color: var(--bv-text);
    padding: var(--bv-space-12);
    text-align: center;
  }

  .title {
    font-family: var(--bv-font-display);
    font-size: 20px;
    font-weight: 300;
    letter-spacing: 0.5px;
    margin: 0;
    color: var(--bv-accent);
  }

  .message {
    color: var(--bv-text-muted);
    margin: 0;
    max-width: 420px;
    font-size: 13px;
    line-height: 1.5;
  }

  .actions {
    display: flex;
    gap: var(--bv-space-2);
    margin-top: var(--bv-space-2);
  }

  .btn {
    padding: 10px 20px;
    border: 1px solid var(--bv-border);
    border-radius: 4px;
    background: transparent;
    color: var(--bv-text);
    font-size: 13px;
    transition:
      background var(--bv-dur-fast) var(--bv-ease),
      color var(--bv-dur-fast) var(--bv-ease),
      border-color var(--bv-dur-fast) var(--bv-ease);
  }
  .btn.ghost {
    color: var(--bv-text-muted);
  }
  .btn.accent {
    border-color: var(--bv-accent);
    color: var(--bv-accent);
  }
  .btn.accent:hover {
    background: var(--bv-accent);
    color: var(--bv-paper);
  }
  :root[data-theme='dark'] .btn.accent:hover {
    color: var(--bv-ink-dark);
  }
  .btn.ghost:hover {
    background: color-mix(in srgb, var(--bv-text) 6%, transparent);
    color: var(--bv-text);
  }
</style>

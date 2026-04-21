<script lang="ts">
  // First-run welcome (spec §8.1). Minimal Japanese layout — wordmark,
  // tagline, permission explanation, single Vermilion primary CTA. We
  // intentionally avoid illustrations and multi-paragraph copy here so
  // the screen feels calm rather than "onboarding-y".
  //
  // The parent is responsible for:
  //   - calling requestPermission() when onGrant fires
  //   - flipping AppConfig.welcome_dismissed to true + persisting
  //   - re-enumerating devices so the picker has real labels
  interface Props {
    onGrant: () => void;
    onSkip?: () => void;
    requesting?: boolean;
  }

  let { onGrant, onSkip, requesting = false }: Props = $props();
</script>

<section class="welcome">
  <p class="wordmark">beamview</p>
  <p class="tagline">Beam your game. See it instantly.</p>

  <p class="explain">
    Beamview needs camera and microphone access to read your capture card. Your video and audio stay
    on this device — nothing is sent to the internet.
  </p>

  <div class="actions">
    {#if onSkip}
      <button type="button" class="btn ghost" onclick={onSkip} disabled={requesting}>
        Skip for now
      </button>
    {/if}
    <button type="button" class="btn accent" onclick={onGrant} disabled={requesting}>
      {requesting ? 'Requesting access…' : 'Grant access'}
    </button>
  </div>
</section>

<style>
  .welcome {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--bv-space-2);
    background: var(--bv-bg);
    color: var(--bv-text);
    padding: var(--bv-space-12);
    text-align: center;
  }

  .wordmark {
    font-family: var(--bv-font-display);
    font-weight: 300;
    font-size: 36px;
    letter-spacing: 3px;
    margin: 0;
  }

  .tagline {
    font-size: 13px;
    color: var(--bv-text-muted);
    letter-spacing: 0.5px;
    margin: 0 0 var(--bv-space-6);
  }

  .explain {
    max-width: 420px;
    font-size: 13px;
    line-height: 1.7;
    color: var(--bv-text-muted);
    margin: 0 0 var(--bv-space-6);
  }

  .actions {
    display: flex;
    gap: var(--bv-space-2);
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
    border-color: var(--bv-accent);
    color: var(--bv-accent);
  }
  .btn.accent:hover:not(:disabled) {
    background: var(--bv-accent);
    color: var(--bv-paper);
  }
  :root[data-theme='dark'] .btn.accent:hover:not(:disabled) {
    color: var(--bv-ink-dark);
  }
</style>

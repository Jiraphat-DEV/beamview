<script lang="ts">
  // Shown while stream.status === 'acquiring'. Minimal Japanese:
  // wordmark + single-line message + a subtle pulse. No spinner —
  // "seijaku" says we shouldn't spin anxiously while waiting.
  interface Props {
    message?: string;
  }

  let { message = 'Connecting to device…' }: Props = $props();
</script>

<section class="loading" aria-live="polite">
  <p class="wordmark">beamview</p>
  <p class="message">{message}</p>
  <div class="pulse" aria-hidden="true"></div>
</section>

<style>
  .loading {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--bv-space-3);
    background: var(--bv-bg);
    color: var(--bv-text);
    padding: var(--bv-space-12);
  }

  .wordmark {
    font-family: var(--bv-font-display);
    font-weight: 300;
    font-size: 28px;
    letter-spacing: 2px;
    margin: 0;
  }

  .message {
    font-size: 13px;
    color: var(--bv-text-muted);
    margin: 0;
  }

  .pulse {
    width: 40px;
    height: 2px;
    margin-top: var(--bv-space-3);
    background: var(--bv-accent);
    border-radius: 1px;
    animation: pulse 1.6s var(--bv-ease) infinite;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 0.25;
      transform: scaleX(0.4);
    }
    50% {
      opacity: 1;
      transform: scaleX(1);
    }
  }
</style>

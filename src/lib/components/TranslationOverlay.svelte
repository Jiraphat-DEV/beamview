<script lang="ts">
  // TranslationOverlay — M4
  //
  // Renders the translated Thai subtitle (and optionally the English caption)
  // as an absolutely-positioned overlay inside the VideoView shell.
  //
  // Styling:
  //  - Semi-opaque paper-colour backdrop for readability over bright footage.
  //  - Subtle text-shadow for extra contrast.
  //  - 150 ms opacity fade on text change.
  //  - 3-dot loading indicator while a tick is in flight.
  //  - Bottom-anchored at 8% from the video edge, horizontally centred, max 75%
  //    of the video width.

  import { translation } from '$lib/stores/translation.svelte';

  // Visible when translation is enabled AND there is Thai text (or loading).
  const visible = $derived(translation.enabled && (translation.th !== null || translation.loading));
</script>

{#if visible}
  <div class="overlay-wrap">
    <div class="subtitle-box" class:has-en={translation.showEnglishCaption && translation.en}>
      {#if translation.showEnglishCaption && translation.en}
        <p class="en-text">{translation.en}</p>
      {/if}

      <p class="th-text">
        {#if translation.loading && !translation.th}
          <span class="loading-dots" aria-label="กำลังแปล">
            <span></span><span></span><span></span>
          </span>
        {:else}
          {translation.th ?? ''}
          {#if translation.loading}
            <span class="loading-inline" aria-hidden="true">
              <span></span><span></span><span></span>
            </span>
          {/if}
        {/if}
      </p>
    </div>
  </div>
{/if}

<style>
  .overlay-wrap {
    position: absolute;
    inset: 0;
    pointer-events: none;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    padding-bottom: 8%;
  }

  .subtitle-box {
    max-width: 75%;
    background: rgba(250, 249, 246, 0.88);
    border-radius: 4px;
    padding: 8px 16px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    animation: fade-in 0.15s ease;
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .en-text {
    margin: 0;
    font-size: 13px;
    line-height: 1.4;
    color: rgba(26, 26, 26, 0.65);
    font-weight: 400;
    text-shadow: none;
    text-align: center;
  }

  .th-text {
    margin: 0;
    font-size: 19px;
    line-height: 1.45;
    font-weight: 500;
    color: #1a1a1a;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.55);
    text-align: center;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  /* 3-dot loading indicator (standalone when no text yet) */
  .loading-dots,
  .loading-inline {
    display: inline-flex;
    gap: 4px;
    align-items: center;
  }

  .loading-dots span,
  .loading-inline span {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: rgba(26, 26, 26, 0.55);
    animation: dot-bounce 1s ease-in-out infinite;
  }

  .loading-dots span:nth-child(2),
  .loading-inline span:nth-child(2) {
    animation-delay: 0.15s;
  }

  .loading-dots span:nth-child(3),
  .loading-inline span:nth-child(3) {
    animation-delay: 0.3s;
  }

  @keyframes dot-bounce {
    0%,
    80%,
    100% {
      transform: scale(0.8);
      opacity: 0.5;
    }
    40% {
      transform: scale(1);
      opacity: 1;
    }
  }
</style>

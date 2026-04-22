<script lang="ts">
  // TranslationOverlay — M4 + M5 polish
  //
  // Renders the translated Thai subtitle and (by default) the English source
  // as an absolutely-positioned overlay inside the VideoView shell.  The EN
  // caption is kept visually prominent because translation latency (~1–2 s)
  // means the English subtitle on-screen has usually already changed by the
  // time the Thai appears — pairing EN↔TH inside the overlay itself is the
  // only reliable way for the user to know which source line is being
  // translated.
  //
  // States:
  //  - enabled && th (no loading)                → normal: EN + TH both
  //                                                 crisp.
  //  - enabled && th && loading                  → stale: TH is dimmed and
  //                                                 a ⟳ hint is appended,
  //                                                 signalling a fresh TH
  //                                                 is on the way.
  //  - enabled && !th && loading (first tick)    → no TH yet: 3-dot spinner
  //                                                 in place of TH.

  import { translation } from '$lib/stores/translation.svelte';

  // Visible when translation is enabled AND there is Thai text (or loading).
  const visible = $derived(translation.enabled && (translation.th !== null || translation.loading));

  // True when the currently-shown TH is being replaced — use it to dim the
  // old text so the user knows the displayed translation is stale.
  const stale = $derived(translation.loading && translation.th !== null);
</script>

{#if visible}
  <div class="overlay-wrap">
    <div class="subtitle-box" class:has-en={translation.showEnglishCaption && translation.en}>
      {#if translation.showEnglishCaption && translation.en}
        <p class="en-text">{translation.en}</p>
      {/if}

      <p class="th-text" class:stale>
        {#if translation.loading && !translation.th}
          <span class="loading-dots" aria-label="กำลังแปล">
            <span></span><span></span><span></span>
          </span>
        {:else}
          <span class="th-phrase">{translation.th ?? ''}</span>
          {#if translation.loading}
            <span class="stale-hint" aria-label="กำลังแปลประโยคใหม่">⟳</span>
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
    max-width: 78%;
    background: rgba(250, 249, 246, 0.92);
    border-radius: 6px;
    padding: 10px 18px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    animation: fade-in 0.15s ease;
  }

  .subtitle-box.has-en {
    gap: 8px;
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  /* EN caption — promoted from 13 px / 0.65 to 16 px / 0.88.
     Italic keeps it visually distinct from the bold Thai line. */
  .en-text {
    margin: 0;
    font-size: 16px;
    line-height: 1.35;
    color: rgba(26, 26, 26, 0.88);
    font-weight: 400;
    font-style: italic;
    text-align: center;
    padding-bottom: 4px;
    border-bottom: 1px solid rgba(26, 26, 26, 0.18);
    width: 100%;
  }

  .th-text {
    margin: 0;
    font-size: 20px;
    line-height: 1.45;
    font-weight: 500;
    color: #1a1a1a;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
    text-align: center;
    display: flex;
    align-items: center;
    gap: 8px;
    transition: opacity 0.15s ease;
  }

  /* When a new translation is in flight but the previous TH is still
     on-screen, dim it so the user knows the shown text is stale. */
  .th-text.stale .th-phrase {
    opacity: 0.55;
  }

  .stale-hint {
    font-size: 15px;
    color: rgba(26, 26, 26, 0.55);
    animation: spin 1.2s linear infinite;
    display: inline-block;
  }

  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  /* 3-dot loading indicator (standalone when no text yet) */
  .loading-dots {
    display: inline-flex;
    gap: 4px;
    align-items: center;
  }

  .loading-dots span {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: rgba(26, 26, 26, 0.55);
    animation: dot-bounce 1s ease-in-out infinite;
  }

  .loading-dots span:nth-child(2) {
    animation-delay: 0.15s;
  }

  .loading-dots span:nth-child(3) {
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

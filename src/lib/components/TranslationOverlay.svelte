<script lang="ts">
  // TranslationOverlay — M4 + M5 polish
  //
  // Renders the translated Thai subtitle (and by default the English source)
  // in one of two layouts controlled by the `variant` prop:
  //  - `overlay`  absolutely positioned at the bottom of the video frame
  //               (original M4 behaviour; covers a small strip of game
  //               content).  VideoView mounts this variant inside the
  //               .video-stage element.
  //  - `panel`    a normal-flow block that lives BELOW the video in
  //               VideoView's flex column — never covers game content.
  //               New default per user feedback.
  //
  // The EN caption is kept visually prominent because translation latency
  // (~1–2 s) means the English subtitle on-screen has usually already
  // changed by the time the Thai appears — pairing EN↔TH inside the
  // overlay itself is the only reliable way for the user to know which
  // source line is being translated.

  import { translation } from '$lib/stores/translation.svelte';

  interface Props {
    variant?: 'overlay' | 'panel';
  }

  const { variant = 'overlay' }: Props = $props();

  // Visible when translation is enabled AND there is Thai text (or loading).
  const visible = $derived(translation.enabled && (translation.th !== null || translation.loading));

  // True when the currently-shown TH is being replaced — use it to dim the
  // old text so the user knows the displayed translation is stale.
  const stale = $derived(translation.loading && translation.th !== null);
</script>

{#if variant === 'panel'}
  <!-- Panel variant is ALWAYS in the DOM when mounted so the video area
       does not resize every time translation toggles on/off — but the
       inner content is only rendered when there's actually something to
       show. -->
  <div class="panel-wrap" class:empty={!visible}>
    {#if visible}
      <div class="subtitle-content panel">
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
    {:else}
      <p class="panel-placeholder">ยังไม่มีคำแปล — ปรับพื้นที่ subtitle หรือรออีกครู่</p>
    {/if}
  </div>
{:else if visible}
  <div class="overlay-wrap">
    <div
      class="subtitle-content overlay"
      class:has-en={translation.showEnglishCaption && translation.en}
    >
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
  /* ── Overlay variant — absolutely positioned over the video ───────── */
  .overlay-wrap {
    position: absolute;
    inset: 0;
    pointer-events: none;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    padding-bottom: 8%;
  }

  .subtitle-content.overlay {
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

  .subtitle-content.overlay.has-en {
    gap: 8px;
  }

  /* ── Panel variant — sits below the video, non-blocking ───────────── */
  .panel-wrap {
    flex: 0 0 auto;
    background: var(--bv-surface, #faf9f6);
    color: var(--bv-text, #1a1a1a);
    border-top: 1px solid var(--bv-divider, rgba(26, 26, 26, 0.12));
    padding: 12px 24px;
    min-height: 72px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .subtitle-content.panel {
    width: 100%;
    max-width: 960px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    animation: fade-in 0.15s ease;
  }

  .panel-placeholder {
    margin: 0;
    color: rgba(26, 26, 26, 0.45);
    font-size: 13px;
    font-style: italic;
  }

  /* Dark-theme-friendly panel tones */
  :global([data-theme='dark']) .panel-wrap {
    background: var(--bv-surface, #181817);
    color: var(--bv-text, #e8e6de);
    border-top-color: rgba(232, 230, 222, 0.14);
  }

  :global([data-theme='dark']) .en-text {
    color: rgba(232, 230, 222, 0.85);
    border-bottom-color: rgba(232, 230, 222, 0.18);
  }

  :global([data-theme='dark']) .th-text {
    color: #e8e6de;
    text-shadow: none;
  }

  :global([data-theme='dark']) .panel-placeholder {
    color: rgba(232, 230, 222, 0.45);
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  /* ── Shared text styles ───────────────────────────────────────────── */

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

  /* Panel variant — no shadow needed because there's no bright
     background behind the text. */
  .subtitle-content.panel .th-text {
    text-shadow: none;
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

<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { stream } from '$lib/stores/stream.svelte';
  import { translation } from '$lib/stores/translation.svelte';
  import { FrameSampler } from '$lib/services/frameSampler';
  import { logger } from '$lib/logger';
  import TranslationOverlay from './TranslationOverlay.svelte';

  // The `muted` attribute is deliberate — audio routes through the
  // Web Audio pipeline in $lib/audio/context. Without `muted`, macOS
  // would play it through both paths simultaneously.
  //
  // `disablepictureinpicture` + `disableremoteplayback` hide the
  // WebKit-native overlay controls that would otherwise appear on
  // Safari/WKWebView when the user right-clicks the video.

  interface Props {
    // Expose the underlying <video> element so parent components (e.g.
    // RegionSelector) can capture a freeze-frame against the live stream.
    videoEl?: HTMLVideoElement | null;
  }

  let { videoEl = $bindable(null) }: Props = $props();
  let sampler: FrameSampler | null = null;

  $effect(() => {
    if (videoEl) {
      videoEl.srcObject = stream.value;
    }
  });

  // M3: Manage the FrameSampler lifecycle via $effect.
  // Start the sampler when translation is enabled, a region is set, and the
  // model is ready.  Stop (and destroy) it in all other cases.
  $effect(() => {
    const shouldSample =
      translation.enabled &&
      translation.region !== null &&
      translation.modelStatus.type === 'ready' &&
      videoEl !== null;

    if (shouldSample && videoEl !== null) {
      if (sampler === null) {
        const el = videoEl; // capture for closure
        sampler = new FrameSampler({
          videoEl: el,
          getRegion: () => translation.region,
          fps: translation.fps,
          onResult: (result) => {
            // Update store so M4 can render the overlay.
            translation.en = result.en;
            translation.th = result.th;
            translation.lastLatencyMs = result.latency_ms;
          },
          onError: (err) => {
            logger.error('[VideoView] frame sampler error', { err: String(err) });
          },
        });
        sampler.start();
        logger.info('[VideoView] frame sampler started');
      }
    } else {
      if (sampler !== null) {
        sampler.stop();
        sampler = null;
        logger.info('[VideoView] frame sampler stopped');
      }
    }
  });

  onDestroy(() => {
    if (videoEl) {
      videoEl.srcObject = null;
    }
    if (sampler !== null) {
      sampler.stop();
      sampler = null;
    }
    // translation.destroy() has been moved to App.svelte's onDestroy (M4
    // carry-over C) so the progress listener lives for the app's full lifetime
    // and is not torn down when the stream drops and VideoView unmounts.
  });

  // M3: Expose a debug harness on window.__beamviewDebug (DEV only).
  // This allows end-to-end testing without any visible UI.
  //
  // Usage in DevTools:
  //   window.__beamviewDebug.downloadModel()
  //   window.__beamviewDebug.setRegion({ x: 0, y: 900, width: 1280, height: 120 })
  //   window.__beamviewDebug.enableTranslation()
  //   // watch the console for [translate] lines
  onMount(() => {
    // Sync the store with Rust's live model state — the Svelte singleton
    // resets to `not_installed` on every hot reload, but the Rust engine
    // survives and may already hold a loaded translator.
    translation.refreshModelStatus();

    if (import.meta.env.DEV) {
      // @ts-expect-error -- debug harness is intentionally untyped
      window.__beamviewDebug = {
        getTranslationStore: () => translation,
        setRegion: (r: { x: number; y: number; width: number; height: number }) =>
          translation.setRegion(r),
        enableTranslation: () => {
          translation.enabled = true;
        },
        disableTranslation: () => {
          translation.enabled = false;
        },
        downloadModel: () => translation.downloadModel(),
        refreshModelStatus: () => translation.refreshModelStatus(),
      };
      console.info('[beamview] debug harness available at window.__beamviewDebug');
    }
  });
</script>

<div class="video-shell">
  <video
    bind:this={videoEl}
    autoplay
    muted
    playsinline
    disablepictureinpicture
    disableremoteplayback
  ></video>
  <TranslationOverlay />
</div>

<style>
  .video-shell {
    flex: 1;
    min-height: 0;
    background: var(--bv-video-bg);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    position: relative;
  }

  video {
    width: 100%;
    height: 100%;
    object-fit: contain;
    background: var(--bv-video-bg);
  }
</style>

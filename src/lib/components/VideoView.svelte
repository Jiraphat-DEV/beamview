<script lang="ts">
  import { onDestroy } from 'svelte';
  import { stream } from '$lib/stores/stream.svelte';

  // The `muted` attribute is deliberate — audio routes through the
  // Web Audio pipeline in $lib/audio/context. Without `muted`, macOS
  // would play it through both paths simultaneously.
  //
  // `disablepictureinpicture` + `disableremoteplayback` hide the
  // WebKit-native overlay controls that would otherwise appear on
  // Safari/WKWebView when the user right-clicks the video.

  let videoEl: HTMLVideoElement | null = null;

  $effect(() => {
    if (videoEl) {
      videoEl.srcObject = stream.value;
    }
  });

  onDestroy(() => {
    if (videoEl) {
      videoEl.srcObject = null;
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
  }

  video {
    width: 100%;
    height: 100%;
    object-fit: contain;
    background: var(--bv-video-bg);
  }
</style>

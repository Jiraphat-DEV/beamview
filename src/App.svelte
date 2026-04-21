<script lang="ts">
  import { onMount } from 'svelte';
  import { commands } from '$lib/ipc';
  import { theme } from '$lib/stores/theme.svelte';
  import TitleBar from '$lib/components/TitleBar.svelte';
  import ActionBar from '$lib/components/ActionBar.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';

  onMount(async () => {
    await theme.init();
    try {
      const version = await commands.getAppVersion();
      console.info(`Beamview ${version}`);
    } catch (err) {
      console.warn('[app] failed to fetch app version', err);
    }
  });

  // Apply the resolved theme to <html>. Main.ts sets it synchronously
  // before mount to avoid FOUC; this effect keeps it in sync after the
  // store has loaded the saved pref and while the OS theme changes.
  $effect(() => {
    if (theme.ready) {
      document.documentElement.dataset.theme = theme.resolved;
    }
  });

  function handleFullscreen() {
    // Milestone 5 wires this to commands.toggleFullscreen()
    console.info('[app] fullscreen requested');
  }
  function handleSettings() {
    // Milestone 6 wires this to the SettingsModal
    console.info('[app] settings requested');
  }
  function handleChoose() {
    // Milestone 4 wires this to the DevicePicker
    console.info('[app] choose device requested');
  }
</script>

<div class="shell">
  <TitleBar />
  <main class="main">
    <EmptyState onChoose={handleChoose} />
  </main>
  <ActionBar onFullscreen={handleFullscreen} onSettings={handleSettings} />
</div>

<style>
  .shell {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bv-bg);
    color: var(--bv-text);
  }

  .main {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--bv-video-bg);
  }
</style>

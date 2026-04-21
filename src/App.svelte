<script lang="ts">
  // Root component — minimal placeholder for Milestone 1.
  // UI shell and real layout land in Milestone 3 (UI Shell).
</script>

<main class="app">
  <h1 class="brand">beamview</h1>
  <p class="tagline">Beam your game. See it instantly.</p>
</main>

<style>
  .app {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    font-family: 'Helvetica Neue', Inter, Arial, sans-serif;
    background: #faf9f6;
    color: #1a1a1a;
  }

  .brand {
    margin: 0;
    font-weight: 300;
    font-size: 32px;
    letter-spacing: 2px;
  }

  .tagline {
    margin: 0;
    font-weight: 400;
    font-size: 14px;
    color: #5f5e5a;
  }

  @media (prefers-color-scheme: dark) {
    .app {
      background: #0f0f0e;
      color: #e8e6de;
    }
    .tagline {
      color: #8a8984;
    }
  }
</style>

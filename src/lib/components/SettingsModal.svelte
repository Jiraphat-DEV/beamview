<script lang="ts">
  import { X } from 'lucide-svelte';
  import { onMount } from 'svelte';
  import { commands } from '$lib/ipc';
  import type { AppConfig, Theme, TranslationConfig } from '$lib/ipc';
  import { DEFAULT_TRANSLATION_CONFIG } from '$lib/ipc';
  import type { ModelInfo } from '$lib/ipc/commands';
  import { displayLabel } from '$lib/capture/devices';
  import { devices } from '$lib/stores/devices.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import { translation } from '$lib/stores/translation.svelte';
  import ModelDownloadModal from './ModelDownloadModal.svelte';
  import RegionSelector from './RegionSelector.svelte';

  // Modal with Video / Audio / About / Translation tabs.
  //
  // Follows the "explicit save only" pattern (spec §1.4, §8.6): a
  // local copy of the editable fields is compared against `config`
  // to drive the dirty flag. Save fires `onSave(newCfg, deviceChanged)`
  // so the parent can re-acquire the stream + persist the config.
  // Cancel (or Esc via the native dialog) discards local edits.
  //
  // Volume + mute are live-applied through ui store rather than waiting
  // for save — those aren't persisted to config Phase 1 (session only)
  // and users expect the knob to respond immediately.

  interface Props {
    open: boolean;
    config: AppConfig;
    onSave: (cfg: AppConfig, deviceChanged: boolean) => void | Promise<void>;
    onClose: () => void;
    /** The live <video> element from VideoView — needed by RegionSelector. */
    videoEl?: HTMLVideoElement | null;
  }

  let { open, config, onSave, onClose, videoEl = null }: Props = $props();

  type Tab = 'video' | 'audio' | 'about' | 'translation';
  let activeTab = $state<Tab>('video');

  let formVideoId = $state<string | null>(null);
  let formAudioId = $state<string | null>(null);
  let formTheme = $state<Theme>('system');
  // Translation form state — local until Save.
  let formTranslation = $state<TranslationConfig>({ ...DEFAULT_TRANSLATION_CONFIG });
  let saving = $state(false);
  let version = $state<string>('');
  let showDownloadModal = $state(false);
  let showRegionSelector = $state(false);
  /** Model ID being downloaded (for the model picker download button state). */
  let downloadingModelId = $state<string | null>(null);
  /** Model ID to download, passed into ModelDownloadModal. */
  let downloadModalModelId = $state<string | null>(null);

  let dialogEl = $state<HTMLDialogElement | null>(null);

  onMount(async () => {
    try {
      version = await commands.getAppVersion();
    } catch {
      version = '—';
    }
  });

  // Sync local form state with the latest config whenever the modal
  // opens. Using an $effect tied to `open` means reopening the modal
  // after an external save reflects the fresh baseline immediately.
  $effect(() => {
    const el = dialogEl;
    if (!el) return;
    if (open && !el.open) {
      formVideoId = config.last_video_device_id;
      formAudioId = config.last_audio_device_id;
      formTheme = config.theme;
      // Spread defaults first so fields added in later schema bumps
      // (e.g. `subtitle_position` after M5 user feedback) fall back to
      // their default when the existing on-disk config predates them.
      formTranslation = { ...DEFAULT_TRANSLATION_CONFIG, ...(config.translation ?? {}) };
      activeTab = 'video';
      el.showModal();
    }
    if (!open && el.open) el.close();
  });

  const deviceChanged = $derived(
    formVideoId !== config.last_video_device_id || formAudioId !== config.last_audio_device_id,
  );

  const translationChanged = $derived(
    formTranslation.enabled !== (config.translation?.enabled ?? false) ||
      formTranslation.fps !== (config.translation?.fps ?? 1.0) ||
      formTranslation.show_english_caption !==
        (config.translation?.show_english_caption ?? false) ||
      formTranslation.subtitle_position !==
        (config.translation?.subtitle_position ?? 'panel_below') ||
      formTranslation.active_model_id !==
        (config.translation?.active_model_id ?? 'nllb-200-distilled-600M') ||
      JSON.stringify(formTranslation.region) !== JSON.stringify(config.translation?.region ?? null),
  );

  const dirty = $derived(deviceChanged || formTheme !== config.theme || translationChanged);

  async function handleSave() {
    saving = true;
    try {
      const newCfg: AppConfig = {
        ...config,
        last_video_device_id: formVideoId,
        last_audio_device_id: formAudioId,
        theme: formTheme,
        translation: { ...formTranslation },
      };
      // Sync the translation store with the saved values so hotkey state
      // and config state stay in sync immediately.
      translation.enabled = formTranslation.enabled;
      translation.fps = formTranslation.fps;
      translation.showEnglishCaption = formTranslation.show_english_caption;
      translation.subtitlePosition = formTranslation.subtitle_position;
      if (formTranslation.region) {
        translation.setRegion(formTranslation.region);
      }
      // If the active model changed, hot-swap the translator in Rust.
      const prevModelId = config.translation?.active_model_id ?? 'nllb-200-distilled-600M';
      if (formTranslation.active_model_id !== prevModelId) {
        await translation.setActiveModel(formTranslation.active_model_id);
      }
      await onSave(newCfg, deviceChanged);
    } finally {
      saving = false;
    }
  }

  function handleCancel() {
    onClose();
  }

  function handleDialogClose() {
    if (open) handleCancel();
  }

  function openRegionSelector() {
    if (!videoEl) return;
    showRegionSelector = true;
    ui.pushModal('region-selector');
  }

  // Refresh the model catalogue when the translation tab becomes active.
  $effect(() => {
    if (open && activeTab === 'translation') {
      translation.refreshModelList();
    }
  });

  // Close the region selector when it pops itself from the modal stack.
  // It calls ui.popModal('region-selector'); we detect when the modal
  // disappears from the stack and reset our local flag.
  $effect(() => {
    if (showRegionSelector && !ui.modalStack.includes('region-selector')) {
      showRegionSelector = false;
      // Pick up the region that RegionSelector may have saved.
      if (translation.region) {
        formTranslation = { ...formTranslation, region: translation.region };
      }
    }
  });

  // Close the download modal when it pops itself from the modal stack.
  $effect(() => {
    if (showDownloadModal && !ui.modalStack.includes('model-download')) {
      showDownloadModal = false;
      downloadingModelId = null;
      downloadModalModelId = null;
    }
  });

  function regionPreviewText(cfg: TranslationConfig): string {
    const r = cfg.region;
    if (!r) return 'ยังไม่ได้เลือก';
    return `x:${r.x} y:${r.y}  ${r.width}×${r.height}`;
  }

  function formatBytes(b: number): string {
    if (b >= 1_000_000_000) return `${(b / 1_000_000_000).toFixed(1)} GB`;
    if (b >= 1_000_000) return `${Math.round(b / 1_000_000)} MB`;
    return `${Math.round(b / 1_000)} KB`;
  }

  function modelBadge(m: ModelInfo): string {
    if (m.is_active) return 'กำลังใช้';
    if (m.installed) return 'ติดตั้งแล้ว';
    return 'ยังไม่ติดตั้ง';
  }

  function modelBadgeClass(m: ModelInfo): string {
    if (m.is_active) return 'badge active';
    if (m.installed) return 'badge installed';
    return 'badge not-installed';
  }

  async function handleDownloadModel(m: ModelInfo) {
    downloadModalModelId = m.id;
    downloadingModelId = m.id;
    showDownloadModal = true;
    ui.pushModal('model-download');
  }

  async function handleUseModel(m: ModelInfo) {
    // Optimistically update the form — the actual hot-swap happens on Save.
    formTranslation = { ...formTranslation, active_model_id: m.id };
  }

  async function handleDeleteModel(m: ModelInfo) {
    try {
      await translation.deleteModel(m.id);
      ui.showToast(`ลบโมเดล ${m.display_name} แล้ว`, 'success');
    } catch (err) {
      ui.showToast(`ลบล้มเหลว: ${String(err)}`, 'error');
    }
  }
</script>

<dialog bind:this={dialogEl} class="modal" onclose={handleDialogClose}>
  <header class="header">
    <h2>Settings</h2>
    <button type="button" class="close" aria-label="Close" onclick={handleCancel}>
      <X size={16} strokeWidth={1.5} />
    </button>
  </header>

  <nav class="tabs" aria-label="Settings tabs">
    <button
      type="button"
      class="tab"
      class:active={activeTab === 'video'}
      onclick={() => (activeTab = 'video')}
    >
      Video
    </button>
    <button
      type="button"
      class="tab"
      class:active={activeTab === 'audio'}
      onclick={() => (activeTab = 'audio')}
    >
      Audio
    </button>
    <button
      type="button"
      class="tab"
      class:active={activeTab === 'translation'}
      onclick={() => (activeTab = 'translation')}
    >
      การแปล
    </button>
    <button
      type="button"
      class="tab"
      class:active={activeTab === 'about'}
      onclick={() => (activeTab = 'about')}
    >
      About
    </button>
  </nav>

  <div class="body">
    {#if activeTab === 'video'}
      <label class="field">
        <span class="label">Capture device</span>
        <select bind:value={formVideoId}>
          <option value={null}>— None —</option>
          {#each devices.video as d (d.deviceId)}
            <option value={d.deviceId}>{displayLabel(d)}</option>
          {/each}
        </select>
      </label>
      <p class="hint">Changing the device reconnects the stream after Save.</p>
    {:else if activeTab === 'audio'}
      <label class="field">
        <span class="label">Audio input</span>
        <select bind:value={formAudioId}>
          <option value={null}>Disabled</option>
          {#each devices.audio as d (d.deviceId)}
            <option value={d.deviceId}>{displayLabel(d)}</option>
          {/each}
        </select>
      </label>

      <div class="field">
        <span class="label">Volume</span>
        <div class="slider-row">
          <input
            type="range"
            min="0"
            max="100"
            value={Math.round(ui.volume * 100)}
            oninput={(e) => ui.setVolume(Number((e.currentTarget as HTMLInputElement).value) / 100)}
            disabled={ui.muted}
          />
          <span class="value bv-mono">{ui.muted ? 'muted' : `${Math.round(ui.volume * 100)}%`}</span
          >
        </div>
      </div>

      <label class="toggle">
        <input type="checkbox" checked={ui.muted} onchange={() => ui.toggleMute()} />
        <span>Mute audio</span>
      </label>
      <p class="hint">Volume and mute apply immediately and persist for the session only.</p>
    {:else if activeTab === 'translation'}
      <!-- Toggle: enable real-time translation -->
      <label class="toggle">
        <input
          type="checkbox"
          checked={formTranslation.enabled}
          onchange={(e) =>
            (formTranslation = {
              ...formTranslation,
              enabled: (e.currentTarget as HTMLInputElement).checked,
            })}
        />
        <span>เปิดการแปลแบบเรียลไทม์</span>
      </label>

      <!-- Region selector -->
      <div class="field tr-region-field">
        <span class="label">พื้นที่ subtitle</span>
        <div class="tr-region-row">
          <span class="bv-mono region-preview">{regionPreviewText(formTranslation)}</span>
          <button
            type="button"
            class="btn-sm"
            onclick={openRegionSelector}
            disabled={!videoEl}
            title={videoEl ? 'เลือกพื้นที่ subtitle บนภาพ' : 'เริ่มสตรีมก่อนเพื่อเลือกพื้นที่'}
          >
            เลือกพื้นที่ subtitle
          </button>
        </div>
        {#if !videoEl}
          <p class="hint">เริ่มสตรีมก่อนเพื่อเปิดตัวเลือกพื้นที่</p>
        {/if}
      </div>

      <!-- FPS slider -->
      <div class="field">
        <span class="label">ความถี่การวิเคราะห์</span>
        <div class="slider-row">
          <input
            type="range"
            min="0"
            max="2"
            step="1"
            value={formTranslation.fps === 0.5 ? 0 : formTranslation.fps === 1.0 ? 1 : 2}
            oninput={(e) => {
              const v = Number((e.currentTarget as HTMLInputElement).value);
              formTranslation = {
                ...formTranslation,
                fps: v === 0 ? 0.5 : v === 1 ? 1.0 : 2.0,
              };
            }}
          />
          <span class="value bv-mono">
            {formTranslation.fps === 0.5 ? '0.5' : formTranslation.fps === 1.0 ? '1' : '2'} fps
          </span>
        </div>
      </div>

      <!-- Show EN caption toggle -->
      <label class="toggle">
        <input
          type="checkbox"
          checked={formTranslation.show_english_caption}
          onchange={(e) =>
            (formTranslation = {
              ...formTranslation,
              show_english_caption: (e.currentTarget as HTMLInputElement).checked,
            })}
        />
        <span>แสดง EN caption เหนือ TH</span>
      </label>

      <!-- Subtitle position -->
      <div class="field">
        <span class="label">ตำแหน่งคำแปล</span>
        <div class="tr-position-group" role="radiogroup" aria-label="ตำแหน่งคำแปล">
          <label class="radio">
            <input
              type="radio"
              name="subtitle-position"
              value="panel_below"
              checked={formTranslation.subtitle_position === 'panel_below'}
              onchange={() =>
                (formTranslation = { ...formTranslation, subtitle_position: 'panel_below' })}
            />
            <span class="radio-label">
              <strong>แยก panel ใต้วิดีโอ</strong>
              <span class="hint">ไม่ทับเนื้อหาเกม (แนะนำ)</span>
            </span>
          </label>
          <label class="radio">
            <input
              type="radio"
              name="subtitle-position"
              value="overlay_bottom"
              checked={formTranslation.subtitle_position === 'overlay_bottom'}
              onchange={() =>
                (formTranslation = { ...formTranslation, subtitle_position: 'overlay_bottom' })}
            />
            <span class="radio-label">
              <strong>ทับวิดีโอด้านล่าง</strong>
              <span class="hint">กะทัดรัด แต่บังเนื้อหาเกมบางส่วน</span>
            </span>
          </label>
        </div>
      </div>

      <!-- Model picker -->
      <div class="field">
        <span class="label">โมเดลแปลภาษา</span>
        {#if translation.modelList.length === 0}
          <p class="hint">กำลังโหลดรายการโมเดล…</p>
        {:else}
          <div class="model-card-list">
            {#each translation.modelList as m (m.id)}
              <div class="model-card" class:selected={formTranslation.active_model_id === m.id}>
                <div class="model-card-top">
                  <div class="model-card-info">
                    <span class="model-name">{m.display_name}</span>
                    <span class={modelBadgeClass(m)}>{modelBadge(m)}</span>
                  </div>
                  <span class="model-size">~{formatBytes(m.size_bytes)}</span>
                </div>
                <p class="model-desc">{m.description}</p>
                <div class="model-card-actions">
                  {#if !m.installed}
                    <button
                      type="button"
                      class="btn-sm"
                      onclick={() => handleDownloadModel(m)}
                      disabled={downloadingModelId === m.id}
                    >
                      {downloadingModelId === m.id ? 'กำลังดาวน์โหลด…' : 'ดาวน์โหลด'}
                    </button>
                  {:else if !m.is_active && formTranslation.active_model_id !== m.id}
                    <button
                      type="button"
                      class="btn-sm accent"
                      onclick={() => handleUseModel(m)}
                      disabled={translation.switchingModel}
                    >
                      ใช้โมเดลนี้
                    </button>
                    <button
                      type="button"
                      class="btn-sm danger"
                      onclick={() => handleDeleteModel(m)}
                      disabled={translation.switchingModel}
                    >
                      ลบ
                    </button>
                  {:else if formTranslation.active_model_id === m.id && !m.is_active}
                    <!-- Will become active on Save -->
                    <span class="hint">จะเปิดใช้งานเมื่อกด Save</span>
                  {:else}
                    <!-- Active model: show delete disabled with tooltip -->
                    <button
                      type="button"
                      class="btn-sm danger"
                      disabled
                      title="ไม่สามารถลบโมเดลที่กำลังใช้งานได้"
                    >
                      ลบ
                    </button>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <p class="hint tr-about">
        โมเดลทำงานแบบออฟไลน์สมบูรณ์ ดาวน์โหลดเพียงครั้งเดียว ไม่มีการส่งข้อมูลสู่อินเทอร์เน็ต
      </p>
    {:else}
      <section class="about">
        <p class="about-name">Beamview</p>
        <p class="about-tagline">Beam your game. See it instantly.</p>

        <dl class="meta">
          <dt>Version</dt>
          <dd class="bv-mono">{version}</dd>
          <dt>License</dt>
          <dd>MIT © 2026 Jiraphat</dd>
        </dl>

        <label class="field">
          <span class="label">Theme</span>
          <select bind:value={formTheme}>
            <option value="system">Follow system</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </label>
      </section>
    {/if}
  </div>

  <footer class="footer">
    <span class="dirty-hint">
      {#if dirty}Unsaved changes{/if}
    </span>
    <div class="spacer"></div>
    <button type="button" class="btn ghost" onclick={handleCancel} disabled={saving}>
      Cancel
    </button>
    <button type="button" class="btn accent" onclick={handleSave} disabled={!dirty || saving}>
      {saving ? 'Saving…' : 'Save'}
    </button>
  </footer>
</dialog>

{#if showDownloadModal}
  <ModelDownloadModal
    modelId={downloadModalModelId}
    onDone={() => {
      downloadingModelId = null;
      downloadModalModelId = null;
      translation.refreshModelList();
    }}
  />
{/if}

{#if showRegionSelector && videoEl}
  <RegionSelector {videoEl} />
{/if}

<style>
  .modal {
    background: var(--bv-surface);
    color: var(--bv-text);
    border: 1px solid var(--bv-border);
    border-radius: 8px;
    padding: 0;
    min-width: 520px;
    max-width: 620px;
    font-family: var(--bv-font-body);
  }
  .modal::backdrop {
    background: rgba(0, 0, 0, 0.4);
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--bv-space-4) var(--bv-space-6);
    border-bottom: 1px solid var(--bv-border);
  }
  h2 {
    font-size: 18px;
    font-weight: 400;
    letter-spacing: 0.3px;
    margin: 0;
  }
  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 4px;
    color: var(--bv-text-muted);
  }
  .close:hover {
    background: color-mix(in srgb, var(--bv-text) 8%, transparent);
    color: var(--bv-text);
  }

  .tabs {
    display: flex;
    gap: var(--bv-space-1);
    padding: 0 var(--bv-space-6);
    border-bottom: 1px solid var(--bv-border);
  }
  .tab {
    padding: 10px 14px;
    font-size: 13px;
    color: var(--bv-text-muted);
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    transition: color var(--bv-dur-fast) var(--bv-ease);
  }
  .tab:hover {
    color: var(--bv-text);
  }
  .tab.active {
    color: var(--bv-text);
    border-bottom-color: var(--bv-accent);
  }

  .body {
    padding: var(--bv-space-6);
    min-height: 220px;
  }

  .field {
    display: block;
    margin-bottom: var(--bv-space-4);
  }
  .label {
    display: block;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--bv-text-subtle);
    margin-bottom: var(--bv-space-1);
  }
  select,
  input[type='range'] {
    width: 100%;
    padding: 8px 10px;
    background: var(--bv-bg);
    color: var(--bv-text);
    border: 1px solid var(--bv-border);
    border-radius: 4px;
    font-family: inherit;
    font-size: 13px;
  }
  input[type='range'] {
    padding: 0;
    accent-color: var(--bv-accent);
  }

  .slider-row {
    display: flex;
    align-items: center;
    gap: var(--bv-space-3);
  }
  .slider-row .value {
    font-size: 12px;
    color: var(--bv-text-muted);
    min-width: 48px;
    text-align: right;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: var(--bv-space-2);
    font-size: 13px;
    color: var(--bv-text);
    margin-bottom: var(--bv-space-2);
  }

  .tr-position-group {
    display: flex;
    flex-direction: column;
    gap: var(--bv-space-2);
  }

  .radio {
    display: flex;
    align-items: flex-start;
    gap: var(--bv-space-3);
    font-size: 13px;
    color: var(--bv-text);
    padding: var(--bv-space-2) var(--bv-space-3);
    border: 1px solid var(--bv-divider, rgba(26, 26, 26, 0.1));
    border-radius: 4px;
    cursor: pointer;
    transition: border-color 0.12s ease;
  }

  .radio:has(input:checked) {
    border-color: var(--bv-text);
  }

  .radio input[type='radio'] {
    margin-top: 2px;
  }

  .radio-label {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .radio-label strong {
    font-weight: 500;
  }

  .radio-label .hint {
    font-size: 11px;
    color: var(--bv-text-subtle);
  }

  .hint {
    color: var(--bv-text-subtle);
    font-size: 11px;
    margin: 0;
  }

  .about-name {
    font-family: var(--bv-font-display);
    font-weight: 300;
    font-size: 24px;
    letter-spacing: 1px;
    margin: 0;
  }
  .about-tagline {
    color: var(--bv-text-muted);
    font-size: 13px;
    margin: var(--bv-space-1) 0 var(--bv-space-5);
  }
  .meta {
    display: grid;
    grid-template-columns: 90px 1fr;
    gap: 6px 12px;
    font-size: 13px;
    margin: 0 0 var(--bv-space-5);
  }
  .meta dt {
    color: var(--bv-text-subtle);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    padding-top: 1px;
  }
  .meta dd {
    margin: 0;
    color: var(--bv-text);
  }

  .footer {
    display: flex;
    align-items: center;
    gap: var(--bv-space-2);
    padding: var(--bv-space-4) var(--bv-space-6);
    border-top: 1px solid var(--bv-border);
  }
  .dirty-hint {
    font-size: 11px;
    color: var(--bv-accent);
    text-transform: uppercase;
    letter-spacing: 1px;
  }
  .spacer {
    flex: 1;
  }

  .btn {
    padding: 8px 16px;
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
    background: var(--bv-accent);
    border-color: var(--bv-accent);
    color: var(--bv-paper);
  }
  :root[data-theme='dark'] .btn.accent {
    color: var(--bv-ink-dark);
  }
  .btn.accent:hover:not(:disabled) {
    background: color-mix(in srgb, var(--bv-accent) 90%, black);
  }

  /* ── Translation tab ────────────────────────────────────────────────────── */

  .tr-region-field,
  .tr-model-row {
    margin-bottom: var(--bv-space-4);
  }

  .tr-region-row,
  .tr-model-status-row {
    display: flex;
    align-items: center;
    gap: var(--bv-space-3);
    flex-wrap: wrap;
  }

  .region-preview {
    font-size: 12px;
    color: var(--bv-text-muted);
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .model-status-text {
    font-size: 12px;
    color: var(--bv-text-muted);
    flex: 1;
  }

  .btn-sm {
    padding: 5px 12px;
    border: 1px solid var(--bv-border);
    border-radius: 4px;
    background: transparent;
    color: var(--bv-text);
    font-size: 12px;
    white-space: nowrap;
    cursor: pointer;
    transition:
      background var(--bv-dur-fast) var(--bv-ease),
      color var(--bv-dur-fast) var(--bv-ease);
  }
  .btn-sm:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .btn-sm:hover:not(:disabled) {
    background: color-mix(in srgb, var(--bv-text) 6%, transparent);
  }

  .tr-about {
    margin-top: var(--bv-space-4);
    line-height: 1.6;
  }

  /* ── Model picker cards ──────────────────────────────────────────────────── */

  .model-card-list {
    display: flex;
    flex-direction: column;
    gap: var(--bv-space-2);
    margin-top: var(--bv-space-1);
  }

  .model-card {
    border: 1px solid var(--bv-divider, rgba(26, 26, 26, 0.12));
    border-radius: 6px;
    padding: var(--bv-space-3) var(--bv-space-4);
    background: var(--bv-bg);
    transition: border-color 0.12s ease;
  }

  .model-card.selected {
    border-color: var(--bv-accent);
  }

  .model-card-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--bv-space-2);
    margin-bottom: 4px;
  }

  .model-card-info {
    display: flex;
    align-items: center;
    gap: var(--bv-space-2);
  }

  .model-name {
    font-size: 13px;
    font-weight: 500;
    color: var(--bv-text);
  }

  .model-size {
    font-size: 11px;
    color: var(--bv-text-subtle);
    white-space: nowrap;
  }

  .model-desc {
    font-size: 11px;
    color: var(--bv-text-subtle);
    margin: 0 0 var(--bv-space-2);
    line-height: 1.5;
  }

  .model-card-actions {
    display: flex;
    align-items: center;
    gap: var(--bv-space-2);
    flex-wrap: wrap;
  }

  /* Status badges */
  .badge {
    display: inline-block;
    padding: 1px 7px;
    border-radius: 10px;
    font-size: 10px;
    font-weight: 500;
    letter-spacing: 0.3px;
    text-transform: uppercase;
  }

  .badge.active {
    background: color-mix(in srgb, var(--bv-accent) 15%, transparent);
    color: var(--bv-accent);
  }

  .badge.installed {
    background: color-mix(in srgb, var(--bv-text) 10%, transparent);
    color: var(--bv-text-muted);
  }

  .badge.not-installed {
    background: transparent;
    color: var(--bv-text-subtle);
    border: 1px solid var(--bv-divider, rgba(26, 26, 26, 0.12));
  }

  /* Accent and danger btn-sm variants */
  .btn-sm.accent {
    background: var(--bv-accent);
    border-color: var(--bv-accent);
    color: var(--bv-paper);
  }
  .btn-sm.accent:hover:not(:disabled) {
    background: color-mix(in srgb, var(--bv-accent) 88%, black);
  }
  :root[data-theme='dark'] .btn-sm.accent {
    color: var(--bv-ink-dark);
  }

  .btn-sm.danger {
    color: #c94f30;
    border-color: rgba(201, 79, 48, 0.35);
  }
  .btn-sm.danger:hover:not(:disabled) {
    background: rgba(201, 79, 48, 0.08);
  }
</style>

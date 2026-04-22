<script lang="ts">
  import { X } from 'lucide-svelte';
  import { onMount } from 'svelte';
  import { translation } from '$lib/stores/translation.svelte';
  import { ui } from '$lib/stores/ui.svelte';

  // ModelDownloadModal — updated M5.5
  //
  // Accepts an optional `modelId` prop.  When provided, it downloads that
  // specific catalogue model.  When omitted (legacy call from the old flow)
  // it downloads the active model.
  //
  // Rendered as a native `<dialog>` with `showModal()` — required to
  // stack ABOVE another native dialog (e.g. SettingsModal).  A plain
  // div can never cover a native-dialog top-layer.

  interface Props {
    /** Specific model to download. If null/undefined, downloads the active model. */
    modelId?: string | null;
    /** Called after a successful download (or on dismiss). */
    onDone?: () => void;
  }

  let { modelId = null, onDone }: Props = $props();

  const MODAL_ID = 'model-download';

  let downloading = $state(false);
  let dialogEl = $state<HTMLDialogElement | null>(null);

  // Promote to the browser top-layer as soon as the dialog renders —
  // otherwise another open `<dialog>` (SettingsModal) covers us.
  onMount(() => {
    // Defer to next microtask so the element is actually in the DOM.
    queueMicrotask(() => {
      if (dialogEl && !dialogEl.open) dialogEl.showModal();
    });
  });
  // Only react to Ready once the user has actually kicked off the
  // download.  Without this flag the modal would auto-close on mount
  // whenever the active model was already loaded (its `modelStatus` is
  // already 'ready'), so the user would see the success toast without
  // ever triggering a real download.
  let started = $state(false);

  // Status of the specific model this modal is downloading.  We
  // prefer the per-model `downloadProgress[modelId]` map so Ready
  // events for OTHER models (e.g. the already-loaded active one)
  // don't trigger our auto-close.
  const status = $derived(
    modelId ? translation.downloadProgress[modelId] : translation.modelStatus,
  );

  function formatBytes(b: number): string {
    if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
    if (b < 1024 * 1024 * 1024) return `${(b / (1024 * 1024)).toFixed(1)} MB`;
    return `${(b / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }

  async function handleDownload() {
    started = true;
    downloading = true;
    try {
      await translation.downloadModel(modelId ?? undefined);
    } finally {
      downloading = false;
    }
  }

  function handleCancel() {
    onDone?.();
    if (dialogEl?.open) dialogEl.close();
    ui.popModal(MODAL_ID);
  }

  // React to status transitions — but only after the user has clicked
  // Download (`started`), so a stale Ready status from a previously
  // loaded model doesn't auto-close the modal before anything
  // happens.
  $effect(() => {
    if (started && status?.type === 'ready') {
      ui.showToast('โมเดลพร้อมใช้งาน', 'success');
      onDone?.();
      if (dialogEl?.open) dialogEl.close();
      ui.popModal(MODAL_ID);
    }
  });

  const progressPercent = $derived(() => {
    const s = status;
    if (s?.type !== 'downloading') return 0;
    return s.total > 0 ? Math.round((s.bytes / s.total) * 100) : 0;
  });

  const progressLabel = $derived(() => {
    const s = status;
    if (s?.type !== 'downloading') return '';
    return `${formatBytes(s.bytes)} / ${formatBytes(s.total)}`;
  });
</script>

<dialog bind:this={dialogEl} class="modal" aria-labelledby="mdm-title" onclose={handleCancel}>
  <header class="header">
    <h2 id="mdm-title">ดาวน์โหลดโมเดลแปลภาษา</h2>
    <button type="button" class="close" aria-label="Close" onclick={handleCancel}>
      <X size={16} strokeWidth={1.5} />
    </button>
  </header>

  <div class="body">
    {#if status?.type === 'not_installed' || (status?.type !== 'downloading' && status?.type !== 'failed' && status?.type !== 'ready')}
      <p class="description">
        ฟีเจอร์แปลภาษาต้องใช้โมเดลซึ่งมีขนาดหลายร้อย MB ดาวน์โหลดเพียงครั้งเดียว
        และทำงานแบบออฟไลน์หลังจากนั้น ไม่มีการส่งข้อมูลออกสู่อินเทอร์เน็ต
      </p>
    {:else if status?.type === 'downloading'}
      <p class="description">กำลังดาวน์โหลดโมเดล กรุณารอสักครู่…</p>
      <div class="progress-wrap">
        <div class="progress-bar">
          <div class="progress-fill" style="width: {progressPercent()}%"></div>
        </div>
        <div class="progress-meta">
          <span class="bv-mono">{progressLabel()}</span>
          <span class="bv-mono">{progressPercent()}%</span>
        </div>
      </div>
    {:else if status?.type === 'failed'}
      <p class="description error-text">
        ดาวน์โหลดล้มเหลว: {status.message}
      </p>
      <p class="hint">กรุณาตรวจสอบการเชื่อมต่ออินเทอร์เน็ตแล้วลองอีกครั้ง</p>
    {/if}
  </div>

  <footer class="footer">
    <button type="button" class="btn ghost" onclick={handleCancel}> ยกเลิก </button>
    <div class="spacer"></div>
    {#if status?.type === 'failed'}
      <button type="button" class="btn accent" onclick={handleDownload} disabled={downloading}>
        ลองอีกครั้ง
      </button>
    {:else if status?.type !== 'downloading'}
      <button type="button" class="btn accent" onclick={handleDownload} disabled={downloading}>
        {downloading ? 'กำลังเตรียม…' : 'ดาวน์โหลดโมเดล'}
      </button>
    {/if}
  </footer>
</dialog>

<style>
  /* Native <dialog> styling — `::backdrop` replaces the old .backdrop div.
     The browser puts the dialog in the top layer when `showModal()` is
     called, so no z-index is needed even when stacked over another open
     `<dialog>` (e.g. SettingsModal). */
  .modal {
    background: var(--bv-surface);
    color: var(--bv-text);
    border: 1px solid var(--bv-border);
    border-radius: 8px;
    padding: 0;
    width: 480px;
    max-width: calc(100vw - 48px);
    font-family: var(--bv-font-body);
  }

  .modal::backdrop {
    background: rgba(0, 0, 0, 0.45);
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--bv-space-4) var(--bv-space-6);
    border-bottom: 1px solid var(--bv-border);
  }

  h2 {
    font-size: 16px;
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

  .body {
    padding: var(--bv-space-6);
    min-height: 100px;
  }

  .description {
    font-size: 13px;
    line-height: 1.55;
    color: var(--bv-text);
    margin: 0 0 var(--bv-space-4);
  }

  .error-text {
    color: #d85a30;
  }

  .hint {
    font-size: 11px;
    color: var(--bv-text-subtle);
    margin: 0;
  }

  .progress-wrap {
    display: flex;
    flex-direction: column;
    gap: var(--bv-space-2);
  }

  .progress-bar {
    width: 100%;
    height: 6px;
    background: color-mix(in srgb, var(--bv-text) 12%, transparent);
    border-radius: 3px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--bv-accent);
    border-radius: 3px;
    transition: width 300ms ease;
  }

  .progress-meta {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    color: var(--bv-text-muted);
  }

  .footer {
    display: flex;
    align-items: center;
    gap: var(--bv-space-2);
    padding: var(--bv-space-4) var(--bv-space-6);
    border-top: 1px solid var(--bv-border);
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
</style>

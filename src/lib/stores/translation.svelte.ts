//! Svelte 5 runes store for the offline EN→TH translation feature.
//!
//! Singleton exported as `translation`.  State is driven by:
//!   - IPC calls to the Rust `TranslationEngine` (via `ocrTranslate`,
//!     `getTranslationModelStatus`, `downloadTranslationModel`)
//!   - Tauri events on `model-download-progress` (emitted by model_store.rs)
//!
//! M3: only console logging — no overlay rendering.  M4 renders the overlay
//! and the model-download modal using the state exposed here.

import { listen } from '@tauri-apps/api/event';
import {
  deleteTranslationModel,
  downloadTranslationModel,
  getTranslationModelStatus,
  listTranslationModels,
  ocrTranslate,
  setActiveTranslationModel,
  type ModelInfo,
  type ModelStatus,
  type OcrTranslateResult,
  type Region,
} from '$lib/ipc/commands';
import { logger } from '$lib/logger';

class TranslationStore {
  /** Whether the 1-fps sampling loop is enabled. */
  enabled = $state(false);

  /** Active region within the video frame (null = disabled). */
  region: Region | null = $state(null);

  /** Current status of the offline translation model. */
  modelStatus: ModelStatus = $state({ type: 'not_installed' });

  /** Catalogue of all known models (populated by `refreshModelList`). */
  modelList: ModelInfo[] = $state([]);

  /** Last recognised English text (null until first successful frame). */
  en: string | null = $state(null);

  /** Last translated Thai text. */
  th: string | null = $state(null);

  /** True while an `ocrTranslate` call is in flight. */
  loading = $state(false);

  /** Round-trip latency of the last completed call (ms). */
  lastLatencyMs: number | null = $state(null);

  /** Last error message from `tick`, `downloadModel`, or model picker ops. */
  lastError: string | null = $state(null);

  /** Frames per second for the sampler (0.5 | 1.0 | 2.0). Default 1.0. */
  fps = $state(1.0);

  /** Whether to show the English caption above the Thai overlay.
   *
   * Default ON — helps the user pair EN↔TH visually despite the ~1–2 s
   * translation lag (by the time TH appears, the on-video EN has usually
   * changed, so the overlay needs to carry its own source-of-truth EN). */
  showEnglishCaption = $state(true);

  /** Where the translated subtitle renders.  `panel_below` is the default
   *  — a separate panel under the video that does NOT cover game content.
   *  `overlay_bottom` preserves the original M4 overlay-on-video layout
   *  for users who prefer the compact look.
   *  Mirrors `SubtitlePosition` in src-tauri/src/config.rs. */
  subtitlePosition: 'panel_below' | 'overlay_bottom' = $state('panel_below');

  /** True while a set-active-model operation is in flight. */
  switchingModel = $state(false);

  /** Per-model download status, keyed by model ID.  Populated by the
   *  `model-download-progress` event listener.  The ModelDownloadModal
   *  watches `downloadProgress[modelId]` so it can correlate Ready events
   *  with the specific model being downloaded (not the currently-active
   *  model whose status is tracked separately in `modelStatus`). */
  downloadProgress: Record<string, ModelStatus> = $state({});

  // ── Private ───────────────────────────────────────────────────────────────

  /** True when a tick call is still in flight — used to drop overlapping ticks. */
  #tickInFlight = false;

  /**
   * Unlisten callback for the `model-download-progress` event.
   * Registered once by `_initProgressListener` and cleaned up on teardown.
   */
  #unlistenProgress: (() => void) | null = null;

  /** Whether the progress listener has been registered. */
  #progressListenerRegistered = false;

  // ── Public methods ────────────────────────────────────────────────────────

  /** Flip the `enabled` flag.  The frame sampler reads this flag each tick. */
  toggle(): void {
    this.enabled = !this.enabled;
    logger.info('[translation] toggled', { enabled: this.enabled });
  }

  /** Update the capture region.  Pass `null` to disable sampling. */
  setRegion(r: Region | null): void {
    this.region = r;
    logger.info('[translation] region set', { region: r });
  }

  /** Fetch the current model status from Rust and update `modelStatus`.
   *
   * Call this at app startup so the store reflects the live backend state
   * after a hot reload — otherwise the store defaults to `not_installed`
   * even when the Rust engine already holds a loaded translator.
   */
  async refreshModelStatus(): Promise<void> {
    try {
      this.modelStatus = await getTranslationModelStatus();
    } catch (err) {
      logger.warn('[translation] refreshModelStatus failed', { err: String(err) });
    }
  }

  /**
   * Start a first-run model download.
   *
   * Pass `modelId` to download a specific catalogue model.
   * When omitted, downloads the currently active model (legacy behaviour).
   */
  async downloadModel(modelId?: string): Promise<void> {
    await this._initProgressListener();
    this.lastError = null;
    try {
      await downloadTranslationModel(modelId);
    } catch (err) {
      const msg = String(err);
      this.lastError = msg;
      this.modelStatus = { type: 'failed', message: msg };
      logger.error('[translation] downloadModel failed', { err: msg });
    }
  }

  /**
   * Fetch the model catalogue from Rust and update `modelList`.
   *
   * Call this on Settings tab open or after any download/delete/switch so
   * the UI always reflects the current on-disk state.
   */
  async refreshModelList(): Promise<void> {
    try {
      this.modelList = await listTranslationModels();
    } catch (err) {
      logger.warn('[translation] refreshModelList failed', { err: String(err) });
    }
  }

  /**
   * Delete a model's files from disk.
   *
   * Refreshes `modelList` on success.  Rejects (and sets `lastError`) when
   * the model is currently active.
   */
  async deleteModel(modelId: string): Promise<void> {
    this.lastError = null;
    try {
      await deleteTranslationModel(modelId);
      await this.refreshModelList();
      logger.info('[translation] model deleted', { modelId });
    } catch (err) {
      const msg = String(err);
      this.lastError = msg;
      logger.error('[translation] deleteModel failed', { modelId, err: msg });
      throw err;
    }
  }

  /**
   * Switch the active translation model.
   *
   * Unloads the current translator and loads the new one.  Refreshes
   * `modelList` and `modelStatus` on completion.  Sets `switchingModel` while
   * in flight so the UI can show a busy indicator.
   */
  async setActiveModel(modelId: string): Promise<void> {
    this.lastError = null;
    this.switchingModel = true;
    try {
      await setActiveTranslationModel(modelId);
      // Update modelStatus to reflect the newly loaded model.
      this.modelStatus = { type: 'ready' };
      await this.refreshModelList();
      logger.info('[translation] active model switched', { modelId });
    } catch (err) {
      const msg = String(err);
      this.lastError = msg;
      logger.error('[translation] setActiveModel failed', { modelId, err: msg });
      throw err;
    } finally {
      this.switchingModel = false;
    }
  }

  /**
   * Invoke one OCR-translate cycle for the given JPEG bytes and region.
   *
   * Idempotent with respect to in-flight calls: if a previous tick is still
   * running the new one is dropped with a warning.
   */
  async tick(jpeg: Uint8Array, region: Region): Promise<void> {
    if (this.#tickInFlight) {
      logger.warn('[translation] tick dropped — previous call still in flight');
      return;
    }

    this.#tickInFlight = true;
    this.loading = true;
    this.lastError = null;

    try {
      const result: OcrTranslateResult | null = await ocrTranslate(jpeg, region);

      if (result !== null) {
        this.en = result.en;
        this.th = result.th;
        this.lastLatencyMs = result.latency_ms;

        console.log(
          '[translate]',
          result.latency_ms + 'ms',
          result.cache_hit ? '(cache)' : '',
          result.duplicate ? '(dup)' : '',
          'EN:',
          result.en,
          'TH:',
          result.th,
        );
      }
      // If null: near-duplicate or no text — keep existing en/th visible.
    } catch (err) {
      const msg = String(err);
      this.lastError = msg;
      logger.error('[translation] tick failed', { err: msg });
    } finally {
      this.loading = false;
      this.#tickInFlight = false;
    }
  }

  /**
   * Tear down the progress event listener.  Call this when the app is
   * unmounted (e.g. in an `onDestroy` hook in the root component).
   */
  destroy(): void {
    if (this.#unlistenProgress) {
      this.#unlistenProgress();
      this.#unlistenProgress = null;
      this.#progressListenerRegistered = false;
    }
  }

  // ── Private ───────────────────────────────────────────────────────────────

  /**
   * Register the `model-download-progress` Tauri event listener once.
   * Subsequent calls are no-ops.
   */
  private async _initProgressListener(): Promise<void> {
    if (this.#progressListenerRegistered) return;

    // The Rust side emits two payload shapes on `model-download-progress`:
    //  - `ModelStatus` (flat): used by `ensure_ready` for the ACTIVE model.
    //  - `{ model_id, status }` (nested): used by `download_model` for any
    //    non-active model (the new multi-model picker flow).
    // Parse both, always update `downloadProgress[id]` keyed by model_id,
    // and update `modelStatus` too when the event pertains to the currently
    // active model.
    type NestedPayload = { model_id: string; status: ModelStatus };
    type Payload = ModelStatus | NestedPayload;

    try {
      const unlisten = await listen<Payload>('model-download-progress', (event) => {
        const p = event.payload;
        if (p && typeof p === 'object' && 'model_id' in p && 'status' in p) {
          const nested = p as NestedPayload;
          this.downloadProgress = { ...this.downloadProgress, [nested.model_id]: nested.status };
          // Also reflect into the singleton `modelStatus` when this event
          // targets the active model (active_model_id was passed from
          // config at startup and lives in Rust; here we track it via the
          // `is_active` flag on `modelList`).
          const active = this.modelList.find((m) => m.is_active);
          if (active && nested.model_id === active.id) {
            this.modelStatus = nested.status;
          }
        } else {
          // Flat ModelStatus — assume it targets the active model.
          const flat = p as ModelStatus;
          this.modelStatus = flat;
          const active = this.modelList.find((m) => m.is_active);
          if (active) {
            this.downloadProgress = { ...this.downloadProgress, [active.id]: flat };
          }
        }
      });
      this.#unlistenProgress = unlisten;
      this.#progressListenerRegistered = true;
    } catch (err) {
      logger.error('[translation] failed to register progress listener', { err: String(err) });
    }
  }
}

export const translation = new TranslationStore();

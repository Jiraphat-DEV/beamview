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
  downloadTranslationModel,
  getTranslationModelStatus,
  ocrTranslate,
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

  /** Last recognised English text (null until first successful frame). */
  en: string | null = $state(null);

  /** Last translated Thai text. */
  th: string | null = $state(null);

  /** True while an `ocrTranslate` call is in flight. */
  loading = $state(false);

  /** Round-trip latency of the last completed call (ms). */
  lastLatencyMs: number | null = $state(null);

  /** Last error message from `tick` or `downloadModel`. */
  lastError: string | null = $state(null);

  /** Frames per second for the sampler (0.5 | 1.0 | 2.0). Default 1.0. */
  fps = $state(1.0);

  /** Whether to show the English caption above the Thai overlay.
   *
   * Default ON — helps the user pair EN↔TH visually despite the ~1–2 s
   * translation lag (by the time TH appears, the on-video EN has usually
   * changed, so the overlay needs to carry its own source-of-truth EN). */
  showEnglishCaption = $state(true);

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
   * Registers a Tauri event listener (once) to keep `modelStatus` in sync
   * with `model-download-progress` events emitted by the Rust downloader.
   * The listener is reused on subsequent calls and torn down on app shutdown.
   */
  async downloadModel(): Promise<void> {
    await this._initProgressListener();
    this.lastError = null;
    try {
      await downloadTranslationModel();
    } catch (err) {
      const msg = String(err);
      this.lastError = msg;
      this.modelStatus = { type: 'failed', message: msg };
      logger.error('[translation] downloadModel failed', { err: msg });
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

    try {
      const unlisten = await listen<ModelStatus>('model-download-progress', (event) => {
        this.modelStatus = event.payload;
      });
      this.#unlistenProgress = unlisten;
      this.#progressListenerRegistered = true;
    } catch (err) {
      logger.error('[translation] failed to register progress listener', { err: String(err) });
    }
  }
}

export const translation = new TranslationStore();

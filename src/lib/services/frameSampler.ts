/**
 * FrameSampler — owns the 1-fps OCR-translate loop.
 *
 * Not a Svelte component.  Create one, call `start()`, and call `stop()` to
 * clean up.  The M4 VideoView component will manage the lifecycle via a Svelte
 * `$effect`.
 *
 * Loop implementation:
 * - Uses `requestVideoFrameCallback` when available (WebKit, Chrome ≥ 83).
 * - Falls back to `setInterval` otherwise.
 * - Drop-frame policy: if a prior IPC call is still pending, the new tick is
 *   skipped entirely (no queuing).
 */

import type { OcrTranslateResult, Region } from '$lib/ipc/commands';
import { ocrTranslate } from '$lib/ipc/commands';

export interface FrameSamplerOptions {
  /** The `<video>` element to sample frames from. */
  videoEl: HTMLVideoElement;
  /**
   * Called fresh each tick so region updates take effect without restarting
   * the sampler.  Return `null` to skip the tick.
   */
  getRegion: () => Region | null;
  /** Frames per second.  Default 1. */
  fps?: number;
  /** Called with a successful OCR-translate result. */
  onResult: (r: OcrTranslateResult) => void;
  /** Called when an error occurs in a tick. */
  onError: (e: unknown) => void;
}

export class FrameSampler {
  private readonly videoEl: HTMLVideoElement;
  private readonly getRegion: () => Region | null;
  private readonly fps: number;
  private readonly onResult: (r: OcrTranslateResult) => void;
  private readonly onError: (e: unknown) => void;

  /** One hidden canvas reused across all ticks. */
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;

  private running = false;
  private ipcInFlight = false;

  /** Handle returned by `requestVideoFrameCallback`, if in use. */
  private rvfcHandle: number | null = null;
  /** Handle returned by `setInterval`, if in use. */
  private intervalHandle: ReturnType<typeof setInterval> | null = null;

  constructor(opts: FrameSamplerOptions) {
    this.videoEl = opts.videoEl;
    this.getRegion = opts.getRegion;
    this.fps = opts.fps ?? 1;
    this.onResult = opts.onResult;
    this.onError = opts.onError;

    // Allocate a hidden canvas (not attached to the DOM).
    this.canvas = document.createElement('canvas');
    const ctx = this.canvas.getContext('2d');
    if (!ctx) throw new Error('FrameSampler: could not get 2D canvas context');
    this.ctx = ctx;
  }

  start(): void {
    if (this.running) return;
    this.running = true;

    if (typeof this.videoEl.requestVideoFrameCallback === 'function') {
      console.info('[FrameSampler] using requestVideoFrameCallback');
      this.scheduleRvfc();
    } else {
      const intervalMs = Math.round(1000 / this.fps);
      console.info(
        `[FrameSampler] requestVideoFrameCallback not available — using setInterval(${intervalMs}ms)`,
      );
      this.intervalHandle = setInterval(() => {
        void this.tick();
      }, intervalMs);
    }
  }

  stop(): void {
    this.running = false;

    if (this.rvfcHandle !== null) {
      this.videoEl.cancelVideoFrameCallback(this.rvfcHandle);
      this.rvfcHandle = null;
    }

    if (this.intervalHandle !== null) {
      clearInterval(this.intervalHandle);
      this.intervalHandle = null;
    }
  }

  // ── Private ─────────────────────────────────────────────────────────────────

  private scheduleRvfc(): void {
    if (!this.running) return;

    // Schedule a tick, then re-schedule after the desired frame interval.
    // `requestVideoFrameCallback` fires on every decoded video frame, which
    // at 30 fps is 30× per second.  We throttle via a timestamp comparison.
    let lastTickTime = 0;
    const intervalMs = Math.round(1000 / this.fps);

    const rvfcCallback = (now: DOMHighResTimeStamp): void => {
      if (!this.running) return;

      if (now - lastTickTime >= intervalMs) {
        lastTickTime = now;
        void this.tick();
      }

      // Re-register for the next frame.
      this.rvfcHandle = this.videoEl.requestVideoFrameCallback(rvfcCallback);
    };

    this.rvfcHandle = this.videoEl.requestVideoFrameCallback(rvfcCallback);
  }

  private async tick(): Promise<void> {
    // Drop-frame: skip if a previous IPC call is still in flight.
    if (this.ipcInFlight) return;

    const region = this.getRegion();
    if (!region) return;

    // Snap the region from the video element onto the canvas.
    this.canvas.width = region.width;
    this.canvas.height = region.height;
    this.ctx.drawImage(
      this.videoEl,
      region.x,
      region.y,
      region.width,
      region.height,
      0,
      0,
      region.width,
      region.height,
    );

    // Encode to JPEG via canvas.toBlob.
    let jpegBytes: Uint8Array;
    try {
      jpegBytes = await canvasToJpeg(this.canvas, 0.85);
    } catch (err) {
      this.onError(err);
      return;
    }

    // Fire IPC call.
    this.ipcInFlight = true;
    try {
      const result = await ocrTranslate(jpegBytes, region);

      // If the sampler was stopped while the call was in flight, ignore.
      if (!this.running) return;

      if (result !== null) {
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
        this.onResult(result);
      }
    } catch (err) {
      if (this.running) this.onError(err);
    } finally {
      this.ipcInFlight = false;
    }
  }
}

// ── Private helpers ──────────────────────────────────────────────────────────

/** Encode a canvas to a JPEG Uint8Array at the given quality. */
function canvasToJpeg(canvas: HTMLCanvasElement, quality: number): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    canvas.toBlob(
      (blob) => {
        if (!blob) {
          reject(new Error('canvas.toBlob produced null'));
          return;
        }
        blob
          .arrayBuffer()
          .then((buf) => resolve(new Uint8Array(buf)))
          .catch(reject);
      },
      'image/jpeg',
      quality,
    );
  });
}

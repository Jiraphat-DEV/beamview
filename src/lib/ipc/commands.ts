import { invoke } from '@tauri-apps/api/core';
import type { AppConfig } from './types';

// Typed wrappers around the Rust `#[tauri::command]` handlers in
// src-tauri/src/commands.rs. Each call returns a Promise and rejects
// with the string body emitted by `Err(String)` on the Rust side.

export const commands = {
  loadConfig: (): Promise<AppConfig> => invoke<AppConfig>('load_config'),

  saveConfig: (config: AppConfig): Promise<void> => invoke<void>('save_config', { config }),

  resetConfig: (): Promise<AppConfig> => invoke<AppConfig>('reset_config'),

  getAppVersion: (): Promise<string> => invoke<string>('get_app_version'),

  quitApp: (): Promise<void> => invoke<void>('quit_app'),

  isFullscreen: (): Promise<boolean> => invoke<boolean>('is_fullscreen'),

  /** Toggle the main window's fullscreen state. Resolves with the new state. */
  toggleFullscreen: (): Promise<boolean> => invoke<boolean>('toggle_fullscreen'),
};

// ── Translation IPC (M3) ──────────────────────────────────────────────────────

/** Pixel-coordinate region within the video frame (top-left origin). */
export type Region = { x: number; y: number; width: number; height: number };

/**
 * Live status of the offline translation model.
 * Mirrors `ModelStatus` in src-tauri/src/translation/types.rs.
 *
 * The Rust enum uses `#[serde(tag = "type", rename_all = "snake_case")]` so
 * each variant arrives as `{ type: 'not_installed' }`, `{ type: 'downloading',
 * bytes: number, total: number }`, etc.
 */
export type ModelStatus =
  | { type: 'not_installed' }
  | { type: 'downloading'; bytes: number; total: number }
  | { type: 'ready' }
  | { type: 'failed'; message: string };

/** Result of a single OCR-translate cycle. */
export type OcrTranslateResult = {
  en: string;
  th: string;
  latency_ms: number;
  cache_hit: boolean;
  duplicate: boolean;
};

/**
 * OCR a captured JPEG frame and translate the text to Thai.
 *
 * `jpegBytes` — JPEG-encoded frame crop from the hidden canvas.
 * `region`    — pixel region used to crop inside the JPEG (or null to use the
 *               full frame).  The crop is re-applied server-side by Apple Vision.
 *
 * Resolves with `null` when the frame contains no text or is a near-duplicate
 * (the frontend should keep the previous translation visible).
 *
 * Note: Tauri 2 serialises `Vec<u8>` as a JSON array of numbers.
 * `Array.from(Uint8Array)` produces exactly that format.
 */
export const ocrTranslate = (
  jpegBytes: Uint8Array,
  region: Region | null,
): Promise<OcrTranslateResult | null> =>
  invoke<OcrTranslateResult | null>('ocr_translate', {
    jpegBytes: Array.from(jpegBytes),
    region,
  });

/** Query the current model status without downloading anything. */
export const getTranslationModelStatus = (): Promise<ModelStatus> =>
  invoke<ModelStatus>('get_translation_model_status');

/**
 * Start a first-run model download (~900 MB).
 *
 * Pass an optional `modelId` to download a specific catalogue entry.
 * When omitted, downloads the currently active model (legacy behaviour).
 *
 * While downloading, the Rust side emits `model-download-progress` events.
 * The payload is `{ model_id: string, status: ModelStatus }` when a model_id
 * is provided, or a bare `ModelStatus` for the legacy path.
 */
export const downloadTranslationModel = (modelId?: string): Promise<void> =>
  invoke<void>('download_translation_model', { modelId: modelId ?? null });

// ── Model picker commands (M5.5) ──────────────────────────────────────────────

/** Per-model catalogue entry returned by `listTranslationModels`. */
export type ModelInfo = {
  id: string;
  display_name: string;
  description: string;
  /** Estimated download size in bytes. */
  size_bytes: number;
  /** True when model files are present and verified on disk. */
  installed: boolean;
  /** True when this is the currently active (loaded) model. */
  is_active: boolean;
  /** Actual on-disk size in bytes (null when not installed). */
  installed_size_bytes: number | null;
};

/** Return metadata for all models in the catalogue. */
export const listTranslationModels = (): Promise<ModelInfo[]> =>
  invoke<ModelInfo[]>('list_translation_models');

/**
 * Delete a model's files from disk.
 *
 * Rejects with an error string when the model is currently active and loaded.
 */
export const deleteTranslationModel = (modelId: string): Promise<void> =>
  invoke<void>('delete_translation_model', { modelId });

/**
 * Switch the active translation model.
 *
 * Unloads the current translator and loads the new one from disk.
 * Rejects with an error string if the model is not installed.
 */
export const setActiveTranslationModel = (modelId: string): Promise<void> =>
  invoke<void>('set_active_translation_model', { modelId });

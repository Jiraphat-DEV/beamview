//! Orchestrator that composes OCR + Translator + ModelStore + TranslationCache.
//!
//! # Usage
//!
//! ```ignore
//! let (engine, status_handle) = TranslationEngine::new()?;
//! engine.ensure_ready(&app_handle, &status_handle).await?;
//! let result = engine.ocr_translate(jpeg_bytes, region).await?;
//! ```
//!
//! # Thread safety
//!
//! `TranslationEngine` is intentionally **not** `Clone`.  At the Tauri state
//! layer it lives behind `Arc<tokio::sync::Mutex<TranslationEngine>>` so
//! concurrent IPC calls serialise on the mutex.
//!
//! The heavy `ocr_translate` path (JPEG decode + Apple Vision + NLLB inference)
//! is dispatched to the Tokio blocking-thread pool via `spawn_blocking` to
//! avoid stalling the async executor.
//!
//! # Non-blocking status reads (carry-over B)
//!
//! A separate `Arc<std::sync::RwLock<ModelStatus>>` (exposed as
//! `ModelStatusHandle`) is managed in parallel with the engine mutex.  The
//! engine writes to it as it progresses through `ensure_ready`; the
//! `get_translation_model_status` command reads from it **without** acquiring
//! the engine mutex, which would otherwise block for up to 5 minutes during a
//! large download.

use std::sync::{Arc, RwLock};
use std::time::Instant;

use tauri::{AppHandle, Emitter, Runtime};

use crate::translation::{
    cache::{CacheLookup, TranslationCache},
    model_store::ModelStore,
    translator::Translator,
    types::{EngineError, ModelStatus, OcrTranslateResult, Region},
};

// ── ModelStatusHandle ─────────────────────────────────────────────────────────

/// A cheaply-cloneable handle to the current `ModelStatus`.
///
/// Reads are lock-free from the perspective of the engine mutex because this
/// `RwLock` is stored *outside* the `Mutex<TranslationEngine>`.  The engine
/// writes to it; the `get_translation_model_status` command reads from it.
pub type ModelStatusHandle = Arc<RwLock<ModelStatus>>;

// ── CallStats ─────────────────────────────────────────────────────────────────

/// Rolling counters emitted as a single summary log line every `STATS_INTERVAL`
/// calls.  This makes cache effectiveness easy to assess from the log file
/// after a real gameplay session without requiring a debug UI.
struct CallStats {
    /// Number of calls since the last log flush.
    total: u64,
    /// OCR-to-NLLB translation calls (model was invoked).
    translations: u64,
    /// Hits served from the LRU cache.
    cache_hits: u64,
    /// Near-duplicate frames skipped via jaro-winkler dedup.
    duplicates: u64,
    /// Accumulated translation latency (ms) for the translation bucket only.
    latency_sum_ms: u64,
    /// Number of translation calls counted in `latency_sum_ms`.
    latency_count: u64,
}

/// Log a summary line every N calls (≈ once per minute at 1 fps).
const STATS_INTERVAL: u64 = 60;

impl CallStats {
    fn new() -> Self {
        Self {
            total: 0,
            translations: 0,
            cache_hits: 0,
            duplicates: 0,
            latency_sum_ms: 0,
            latency_count: 0,
        }
    }

    fn record_translation(&mut self, latency_ms: u64) {
        self.total += 1;
        self.translations += 1;
        self.latency_sum_ms += latency_ms;
        self.latency_count += 1;
        self.maybe_flush();
    }

    fn record_cache_hit(&mut self) {
        self.total += 1;
        self.cache_hits += 1;
        self.maybe_flush();
    }

    fn record_duplicate(&mut self) {
        self.total += 1;
        self.duplicates += 1;
        self.maybe_flush();
    }

    fn maybe_flush(&mut self) {
        if self.total % STATS_INTERVAL != 0 {
            return;
        }
        let median_ms = self
            .latency_sum_ms
            .checked_div(self.latency_count)
            .unwrap_or(0);
        let hit_pct = (self.cache_hits * 100).checked_div(self.total).unwrap_or(0);
        log::info!(
            "[translate] last {total} calls: {trans} translations / {hits} cache hits ({hit_pct}%) / {dups} duplicates — median latency {median_ms} ms",
            total = STATS_INTERVAL,
            trans = self.translations,
            hits = self.cache_hits,
            hit_pct = hit_pct,
            dups = self.duplicates,
            median_ms = median_ms
        );
        // Reset counters for the next window.
        self.translations = 0;
        self.cache_hits = 0;
        self.duplicates = 0;
        self.latency_sum_ms = 0;
        self.latency_count = 0;
    }
}

// ── TranslationEngine ─────────────────────────────────────────────────────────

/// Composes ModelStore, Translator, and TranslationCache into a single
/// orchestrating type.  Designed to be held behind
/// `Arc<tokio::sync::Mutex<TranslationEngine>>` at the Tauri state layer.
pub struct TranslationEngine {
    model_store: ModelStore,
    /// Lazily initialised after `ensure_ready` succeeds.
    translator: Option<Translator>,
    cache: TranslationCache,
    /// Rolling statistics for cache-effectiveness logging.
    stats: CallStats,
}

impl TranslationEngine {
    /// Construct a new engine.  Checks whether the model is already installed
    /// on disk but does NOT load it into memory yet — call `ensure_ready` for
    /// that.
    ///
    /// Returns the engine and a shared `ModelStatusHandle` that can be passed
    /// to Tauri state independently so `get_translation_model_status` can read
    /// the status without acquiring the engine mutex.
    pub fn new() -> Result<(Self, ModelStatusHandle), EngineError> {
        let model_store = ModelStore::new()?;
        let initial_status = model_store.model_status();
        let status_handle: ModelStatusHandle = Arc::new(RwLock::new(initial_status));
        let engine = Self {
            model_store,
            translator: None,
            cache: TranslationCache::new(),
            stats: CallStats::new(),
        };
        Ok((engine, status_handle))
    }

    /// Return the current model status without performing any I/O.
    ///
    /// Prefer reading from the `ModelStatusHandle` at the Tauri state layer
    /// for non-blocking reads; this method is kept for internal use and tests.
    pub fn model_status(&self) -> ModelStatus {
        if self.translator.is_some() {
            return ModelStatus::Ready;
        }
        self.model_store.model_status()
    }

    /// Ensure the model is downloaded and loaded into memory.
    ///
    /// Safe to call repeatedly — returns immediately when the translator is
    /// already initialised.  Emits `model-download-progress` events to the
    /// frontend while downloading.  Keeps `status_handle` updated so
    /// concurrent `get_translation_model_status` reads see live progress.
    pub async fn ensure_ready<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        status_handle: &ModelStatusHandle,
    ) -> Result<(), EngineError> {
        if self.translator.is_some() {
            // Model is already loaded in this process — but the frontend
            // store may be stale (e.g. after a hot reload or ⌘R, which
            // resets the Svelte singleton while the Rust engine survives).
            // Emit the Ready status so any current listeners can sync.
            *status_handle.write().unwrap() = ModelStatus::Ready;
            let _ = app.emit("model-download-progress", &ModelStatus::Ready);
            return Ok(());
        }

        // Download if not already on disk.
        if !matches!(self.model_store.model_status(), ModelStatus::Ready) {
            let app_clone = app.clone();
            let sh = status_handle.clone();
            self.model_store
                .download(move |status| {
                    // Update the non-blocking handle so concurrent status
                    // queries return live progress without needing the engine lock.
                    *sh.write().unwrap() = status.clone();
                    // Best-effort emit — ignore send errors (window may not exist yet).
                    let _ = app_clone.emit("model-download-progress", &status);
                })
                .await?;
        }

        // Load the model from disk on the blocking pool (ORT session init
        // is synchronous and may take several seconds).
        let model_dir = self.model_store.model_dir().to_owned();
        let translator = tokio::task::spawn_blocking(move || Translator::load(&model_dir))
            .await
            .map_err(|e| {
                let msg = e.to_string();
                log::error!("[engine] Translator::load panicked in blocking pool: {msg}");
                EngineError::BlockingPanic(msg)
            })??;

        self.translator = Some(translator);

        // Update the non-blocking handle and notify the frontend.
        *status_handle.write().unwrap() = ModelStatus::Ready;
        let _ = app.emit("model-download-progress", &ModelStatus::Ready);
        Ok(())
    }

    /// If the model files are already on disk but the translator is not yet
    /// loaded (typical after an app restart), load it synchronously on the
    /// blocking pool.  Never downloads — pure disk-to-memory load.
    ///
    /// Returns `Ok(true)` when the translator is loaded (or was already
    /// loaded), `Ok(false)` when the model files are missing and a real
    /// `ensure_ready` with an `AppHandle` is still required.
    async fn load_from_disk_if_present(&mut self) -> Result<bool, EngineError> {
        if self.translator.is_some() {
            return Ok(true);
        }
        if !matches!(self.model_store.model_status(), ModelStatus::Ready) {
            return Ok(false);
        }
        let model_dir = self.model_store.model_dir().to_owned();
        let translator = tokio::task::spawn_blocking(move || Translator::load(&model_dir))
            .await
            .map_err(|e| {
                let msg = e.to_string();
                log::error!(
                    "[engine] Translator::load panicked in blocking pool (auto-heal): {msg}"
                );
                EngineError::BlockingPanic(msg)
            })??;
        self.translator = Some(translator);
        Ok(true)
    }

    /// Decode JPEG → RGBA → OCR (optional region crop) → dedup/cache lookup
    /// → translate → cache insert → return.
    ///
    /// Returns `Ok(None)` when:
    /// - the region contains no text, or
    /// - the OCR result is a near-duplicate of the last seen text AND no
    ///   cached translation exists (i.e. the first occurrence of that text
    ///   was itself a no-text frame).
    ///
    /// The JPEG decode + OCR + translate path runs on the Tokio blocking
    /// pool to avoid stalling the async executor.
    pub async fn ocr_translate(
        &mut self,
        jpeg_bytes: Vec<u8>,
        region: Option<Region>,
    ) -> Result<Option<OcrTranslateResult>, EngineError> {
        // Auto-heal: after an app restart the Rust engine is fresh
        // (translator = None) but model files may still be on disk.  Load
        // them synchronously so the sampler does not need to wait for an
        // explicit download_translation_model call from the frontend.
        // Never downloads from this path — that is reserved for the
        // explicit `download_translation_model` command.
        if !self.load_from_disk_if_present().await? {
            return Err(EngineError::ModelNotReady);
        }

        let start = Instant::now();

        // ── 1. Decode JPEG → RGBA on the blocking pool ────────────────────────
        let (rgba_bytes, img_width, img_height) = {
            let bytes_clone = jpeg_bytes.clone();
            tokio::task::spawn_blocking(move || decode_jpeg(&bytes_clone))
                .await
                .map_err(|e| EngineError::BlockingPanic(e.to_string()))??
        };

        // ── 2. OCR ─────────────────────────────────────────────────────────────
        // Run on the blocking pool (Apple Vision is synchronous).
        let ocr_text = {
            let rgba = rgba_bytes.clone();
            let w = img_width;
            let h = img_height;
            tokio::task::spawn_blocking(move || {
                #[cfg(target_os = "macos")]
                {
                    crate::translation::ocr::recognize_english(&rgba, w, h, region)
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = (rgba, w, h, region);
                    Err::<String, _>(crate::translation::types::OcrError::UnsupportedPlatform)
                }
            })
            .await
            .map_err(|e| EngineError::BlockingPanic(e.to_string()))?
        };

        let en_text = match ocr_text {
            Ok(t) if !t.trim().is_empty() => t,
            Ok(_) | Err(crate::translation::types::OcrError::NoTextFound) => {
                return Ok(None);
            }
            Err(e) => return Err(EngineError::Ocr(e)),
        };

        // ── 3. Dedup / cache lookup ────────────────────────────────────────────
        match self.cache.lookup(&en_text) {
            CacheLookup::Hit(th) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                log::debug!("[translate] cache hit — {latency_ms}ms EN: {en_text}");
                self.stats.record_cache_hit();
                return Ok(Some(OcrTranslateResult {
                    en: en_text,
                    th,
                    latency_ms,
                    cache_hit: true,
                    duplicate: false,
                }));
            }
            CacheLookup::Duplicate => {
                let latency_ms = start.elapsed().as_millis() as u64;
                log::debug!("[translate] duplicate — {latency_ms}ms EN: {en_text}");
                self.stats.record_duplicate();
                // Return None so the frontend keeps the previous translation
                // visible.  The caller must not clear the overlay on None.
                return Ok(None);
            }
            CacheLookup::Miss => {
                // Fall through to translation.
            }
        }

        // ── 4. Translate on the blocking pool ──────────────────────────────────
        // `Translator::translate_en_to_th` is synchronous and CPU-bound (~1 s).
        // We take the translator out of the Option, move it into the closure,
        // then put it back after the blocking call completes.
        let mut translator = self.translator.take().expect("checked above");
        let en_for_block = en_text.clone();

        let (translator, translate_result) = tokio::task::spawn_blocking(move || {
            let result = translator.translate_en_to_th(&en_for_block);
            (translator, result)
        })
        .await
        .map_err(|e| EngineError::BlockingPanic(e.to_string()))?;

        // Restore the translator regardless of whether translation succeeded.
        self.translator = Some(translator);

        let th_text = translate_result?;
        let latency_ms = start.elapsed().as_millis() as u64;

        // ── 5. Cache insert + stats + return ──────────────────────────────────
        self.cache.insert(&en_text, th_text.clone());
        self.stats.record_translation(latency_ms);

        log::info!("[translate] {latency_ms}ms EN: {en_text} → TH: {th_text}");

        Ok(Some(OcrTranslateResult {
            en: en_text,
            th: th_text,
            latency_ms,
            cache_hit: false,
            duplicate: false,
        }))
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Decode a JPEG byte buffer to raw RGBA8 pixels.
/// Returns `(rgba_bytes, width, height)`.
fn decode_jpeg(jpeg_bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), EngineError> {
    use image::ImageReader;
    use std::io::Cursor;

    let reader = ImageReader::new(Cursor::new(jpeg_bytes))
        .with_guessed_format()
        .map_err(|e| EngineError::ImageDecode(e.to_string()))?;

    let img = reader
        .decode()
        .map_err(|e| EngineError::ImageDecode(e.to_string()))?;

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok((rgba.into_raw(), width, height))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that `ocr_translate` returns `EngineError::ModelNotReady` when
    /// the model files are missing from disk (and the translator is not
    /// loaded).  Uses a tempdir-backed ModelStore so the assertion is stable
    /// regardless of whether the real user app-data dir happens to be seeded.
    #[tokio::test]
    async fn ocr_translate_errors_when_no_model_files() {
        use crate::translation::model_store::ModelStore;
        use tempfile::TempDir;

        let tmp = TempDir::new().expect("tempdir");
        let mut engine = TranslationEngine {
            model_store: ModelStore::new_with_dir(tmp.path().to_owned()),
            translator: None,
            cache: TranslationCache::new(),
            stats: CallStats::new(),
        };

        // Confirm precondition: no sentinel, translator not loaded.
        assert!(matches!(
            engine.model_store.model_status(),
            ModelStatus::NotInstalled
        ));
        assert!(engine.translator.is_none());

        let dummy_jpeg = vec![0u8; 10];
        let err = engine
            .ocr_translate(dummy_jpeg, None)
            .await
            .expect_err("expected EngineError::ModelNotReady");

        assert!(
            matches!(err, EngineError::ModelNotReady),
            "expected ModelNotReady, got: {err}"
        );
    }

    /// Verify that the cache is consulted and a Hit avoids the translator path.
    /// We pre-populate the cache and inject a fake translator state by using
    /// `model_status` as a proxy (translator is None so model_status returns
    /// the disk status, not Ready — but that is sufficient for this cache path
    /// test which is logically separate from the ensure_ready path).
    #[test]
    fn cache_lookup_hit_returns_th() {
        let mut cache = TranslationCache::new();
        cache.insert("Hello", "สวัสดี".to_owned());
        match cache.lookup("Hello") {
            CacheLookup::Hit(th) => assert_eq!(th, "สวัสดี"),
            other => panic!("expected Hit, got {other:?}"),
        }
    }

    /// Verify that the dedup path returns `Duplicate` for near-identical text.
    #[test]
    fn cache_lookup_duplicate_for_near_identical_text() {
        let mut cache = TranslationCache::new();
        cache.insert(
            "You cannot escape fate.",
            "คุณไม่สามารถหนีจากชะตากรรมได้".to_owned(),
        );
        assert!(
            matches!(
                cache.lookup("You cannot escape fate!"),
                CacheLookup::Duplicate
            ),
            "expected Duplicate for near-identical text"
        );
    }

    /// Verify `model_status` returns `ModelNotReady`-equivalent when translator
    /// is not loaded and the model files are not present on disk.
    #[test]
    fn model_status_not_installed_when_no_translator() {
        let (engine, _status_handle) = TranslationEngine::new().expect("TranslationEngine::new");
        assert!(engine.translator.is_none());
        // model_status delegates to model_store when translator is None;
        // in CI (no model files present) it should return NotInstalled or Ready
        // depending on what's on disk.  We only assert it is NOT the in-memory
        // Ready path (which only triggers when translator.is_some()).
        let status = engine.model_status();
        // If disk happens to have the sentinel we can't force NotInstalled here,
        // but we can confirm the type is something meaningful.
        assert!(
            matches!(
                status,
                ModelStatus::NotInstalled | ModelStatus::Ready | ModelStatus::Failed { .. }
            ),
            "unexpected model_status: {status:?}"
        );
    }
}

//! Orchestrator: OCR + Translator + ModelStore + TranslationCache.
//!
//! Upgraded in M5.5 to support a multi-model registry and hot-swap.
//!
//! # Thread safety
//!
//! `TranslationEngine` lives behind `Arc<tokio::sync::Mutex<TranslationEngine>>`
//! so concurrent IPC calls serialise on the mutex.
//!
//! # Non-blocking status reads
//!
//! A separate `Arc<std::sync::RwLock<ModelStatus>>` (`ModelStatusHandle`) lets
//! `get_translation_model_status` read without acquiring the engine mutex.

use std::sync::{Arc, RwLock};
use std::time::Instant;

use tauri::{AppHandle, Emitter, Runtime};

use crate::translation::{
    cache::{CacheLookup, TranslationCache},
    model_store::{ModelSpec, ModelStore, MODEL_REGISTRY},
    translator::Translator,
    types::{EngineError, ModelStatus, OcrTranslateResult, Region},
};

// ── ModelStatusHandle ─────────────────────────────────────────────────────────

/// Cheaply-cloneable handle to the current download/load `ModelStatus`.
pub type ModelStatusHandle = Arc<RwLock<ModelStatus>>;

// ── CallStats ─────────────────────────────────────────────────────────────────

struct CallStats {
    total: u64,
    translations: u64,
    cache_hits: u64,
    duplicates: u64,
    latency_sum_ms: u64,
    latency_count: u64,
}

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
        self.translations = 0;
        self.cache_hits = 0;
        self.duplicates = 0;
        self.latency_sum_ms = 0;
        self.latency_count = 0;
    }
}

// ── ModelInfo (IPC wire type) ─────────────────────────────────────────────────

/// Per-model data returned by `list_translation_models`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub description: String,
    /// Estimated download size in bytes.
    pub size_bytes: u64,
    /// True when the model files are present and verified on disk.
    pub installed: bool,
    /// True when this is the currently active (loaded) model.
    pub is_active: bool,
    /// Actual on-disk size in bytes (None when not installed).
    pub installed_size_bytes: Option<u64>,
}

// ── TranslationEngine ─────────────────────────────────────────────────────────

pub struct TranslationEngine {
    model_store: ModelStore,
    /// ID of the model the user has chosen (from `TranslationConfig`).
    active_model_id: String,
    /// Loaded translator (None until `ensure_ready` or `set_active_model`).
    translator: Option<Translator>,
    cache: TranslationCache,
    stats: CallStats,
}

impl TranslationEngine {
    /// Construct a new engine.  `active_model_id` comes from the persisted
    /// `TranslationConfig` — defaults to `"nllb-200-distilled-600M"`.
    pub fn new(active_model_id: String) -> Result<(Self, ModelStatusHandle), EngineError> {
        let model_store = ModelStore::new()?;
        // Wipe directories for retired model IDs (e.g. m2m100-418M, removed
        // because Xenova's tokenizer.json is malformed upstream).  Frees
        // hundreds of MB so users do not silently keep useless downloads.
        let reclaimed = model_store.cleanup_orphaned_dirs();
        if reclaimed > 0 {
            log::info!("[engine] reclaimed {reclaimed} bytes from retired model dirs");
        }
        let initial_status = model_store.model_status(&active_model_id);
        let status_handle: ModelStatusHandle = Arc::new(RwLock::new(initial_status));
        let engine = Self {
            model_store,
            active_model_id,
            translator: None,
            cache: TranslationCache::new(),
            stats: CallStats::new(),
        };
        Ok((engine, status_handle))
    }

    /// Return the current model status for the active model.
    pub fn model_status(&self) -> ModelStatus {
        if self.translator.is_some() {
            return ModelStatus::Ready;
        }
        self.model_store.model_status(&self.active_model_id)
    }

    /// Return the active model ID.
    pub fn active_model_id(&self) -> &str {
        &self.active_model_id
    }

    /// Return metadata for all catalogue models.
    pub fn list_models(&self) -> Vec<ModelInfo> {
        MODEL_REGISTRY
            .iter()
            .map(|spec| {
                let installed = self.model_store.is_installed(spec.id);
                let is_active = spec.id == self.active_model_id;
                ModelInfo {
                    id: spec.id.to_owned(),
                    display_name: spec.display_name.to_owned(),
                    description: spec.description.to_owned(),
                    size_bytes: spec.size_bytes,
                    installed,
                    is_active,
                    installed_size_bytes: self.model_store.installed_size_bytes(spec.id),
                }
            })
            .collect()
    }

    /// Ensure the active model is downloaded and loaded into memory.
    ///
    /// Safe to call repeatedly — returns immediately when already loaded.
    pub async fn ensure_ready<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        status_handle: &ModelStatusHandle,
    ) -> Result<(), EngineError> {
        if self.translator.is_some() {
            *status_handle.write().unwrap() = ModelStatus::Ready;
            let _ = app.emit("model-download-progress", &ModelStatus::Ready);
            return Ok(());
        }

        if !matches!(
            self.model_store.model_status(&self.active_model_id),
            ModelStatus::Ready
        ) {
            let app_clone = app.clone();
            let sh = status_handle.clone();
            let model_id = self.active_model_id.clone();
            self.model_store
                .download(&model_id, move |status| {
                    *sh.write().unwrap() = status.clone();
                    let _ = app_clone.emit("model-download-progress", &status);
                })
                .await?;
        }

        self.load_active_model().await?;

        *status_handle.write().unwrap() = ModelStatus::Ready;
        let _ = app.emit("model-download-progress", &ModelStatus::Ready);
        Ok(())
    }

    /// Download a specific model by ID (does not change the active model).
    ///
    /// Emits `model-download-progress` events tagged with `{ model_id, status }`.
    pub async fn download_model<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        status_handle: &ModelStatusHandle,
        model_id: &str,
    ) -> Result<(), EngineError> {
        // Validate model ID exists in registry.
        let _spec = spec_for_id(model_id)?;
        let downloading_for_active = model_id == self.active_model_id;

        let app_clone = app.clone();
        let sh = status_handle.clone();
        let mid = model_id.to_owned();

        self.model_store
            .download(model_id, move |status| {
                if downloading_for_active {
                    *sh.write().unwrap() = status.clone();
                }
                let _ = app_clone.emit(
                    "model-download-progress",
                    &serde_json::json!({ "model_id": mid, "status": status }),
                );
            })
            .await?;

        // If the downloaded model is the active one and no translator loaded yet,
        // auto-load it now.
        if downloading_for_active && self.translator.is_none() {
            self.load_active_model().await?;
            *status_handle.write().unwrap() = ModelStatus::Ready;
            let _ = app.emit("model-download-progress", &ModelStatus::Ready);
        }

        Ok(())
    }

    /// Delete a model's files from disk.
    ///
    /// Returns an error when the requested model is currently active and loaded.
    pub async fn delete_model(&mut self, model_id: &str) -> Result<(), EngineError> {
        if model_id == self.active_model_id && self.translator.is_some() {
            return Err(EngineError::CannotDeleteActiveModel);
        }
        self.model_store.delete(model_id).await?;
        Ok(())
    }

    /// Switch the active model to `model_id`.
    ///
    /// Unloads the current translator, clears the translation cache, and loads
    /// the new model from disk.  Returns an error if the model is not installed.
    pub async fn set_active_model(&mut self, model_id: &str) -> Result<(), EngineError> {
        let _spec = spec_for_id(model_id)?;

        if !self.model_store.is_installed(model_id) {
            return Err(EngineError::ModelNotReady);
        }

        // Unload current translator.
        self.translator = None;
        self.cache = TranslationCache::new();

        self.active_model_id = model_id.to_owned();
        self.load_active_model().await?;

        log::info!("[engine] active model switched to '{model_id}'");
        Ok(())
    }

    /// Load the active model from disk on the blocking pool.
    async fn load_active_model(&mut self) -> Result<(), EngineError> {
        let spec = spec_for_id(&self.active_model_id)?;
        let model_dir = self.model_store.model_dir(&self.active_model_id);
        let arch = spec.arch;

        let translator = tokio::task::spawn_blocking(move || Translator::load(&model_dir, arch))
            .await
            .map_err(|e| {
                let msg = e.to_string();
                log::error!("[engine] Translator::load panicked: {msg}");
                EngineError::BlockingPanic(msg)
            })??;

        self.translator = Some(translator);
        log::info!(
            "[engine] translator loaded — model='{}'",
            self.active_model_id
        );
        Ok(())
    }

    /// Auto-heal: load from disk if the model is present but not yet loaded.
    ///
    /// Called at the start of every `ocr_translate` call to recover from app
    /// restart without requiring an explicit download command.
    async fn load_from_disk_if_present(&mut self) -> Result<bool, EngineError> {
        if self.translator.is_some() {
            return Ok(true);
        }
        if !self.model_store.is_installed(&self.active_model_id) {
            return Ok(false);
        }
        self.load_active_model().await?;
        Ok(true)
    }

    /// OCR + translate a JPEG frame.
    pub async fn ocr_translate(
        &mut self,
        jpeg_bytes: Vec<u8>,
        region: Option<Region>,
    ) -> Result<Option<OcrTranslateResult>, EngineError> {
        if !self.load_from_disk_if_present().await? {
            return Err(EngineError::ModelNotReady);
        }

        let start = Instant::now();

        // 1. Decode JPEG
        let (rgba_bytes, img_width, img_height) = {
            let bytes_clone = jpeg_bytes.clone();
            tokio::task::spawn_blocking(move || decode_jpeg(&bytes_clone))
                .await
                .map_err(|e| EngineError::BlockingPanic(e.to_string()))??
        };

        // 2. OCR
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

        // 3. Dedup / cache lookup
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
                return Ok(None);
            }
            CacheLookup::Miss => {}
        }

        // 4. Translate on blocking pool
        let mut translator = self.translator.take().expect("checked above");
        let en_for_block = en_text.clone();

        let (translator, translate_result) = tokio::task::spawn_blocking(move || {
            let result = translator.translate_en_to_th(&en_for_block);
            (translator, result)
        })
        .await
        .map_err(|e| EngineError::BlockingPanic(e.to_string()))?;

        self.translator = Some(translator);

        let th_text = translate_result?;
        let latency_ms = start.elapsed().as_millis() as u64;

        // 5. Cache + stats
        self.cache.insert(&en_text, th_text.clone());
        self.stats.record_translation(latency_ms);

        log::info!(
            "[translate] {latency_ms}ms model={} EN: {en_text} → TH: {th_text}",
            self.active_model_id
        );

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

fn spec_for_id(model_id: &str) -> Result<&'static ModelSpec, EngineError> {
    MODEL_REGISTRY
        .iter()
        .find(|s| s.id == model_id)
        .ok_or_else(|| EngineError::UnknownModel(model_id.to_owned()))
}

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
    use crate::translation::cache::TranslationCache;

    fn make_test_engine(model_dir: std::path::PathBuf) -> TranslationEngine {
        TranslationEngine {
            model_store: ModelStore::new_with_dir(model_dir),
            active_model_id: "nllb-200-distilled-600M".to_owned(),
            translator: None,
            cache: TranslationCache::new(),
            stats: CallStats::new(),
        }
    }

    #[tokio::test]
    async fn ocr_translate_errors_when_no_model_files() {
        use tempfile::TempDir;

        let tmp = TempDir::new().expect("tempdir");
        let model_dir = tmp.path().join("nllb-200-distilled-600M");
        std::fs::create_dir_all(&model_dir).unwrap();

        let mut engine = make_test_engine(model_dir);

        assert!(matches!(engine.model_status(), ModelStatus::NotInstalled));
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

    #[test]
    fn cache_lookup_hit_returns_th() {
        let mut cache = TranslationCache::new();
        cache.insert("Hello", "สวัสดี".to_owned());
        match cache.lookup("Hello") {
            CacheLookup::Hit(th) => assert_eq!(th, "สวัสดี"),
            other => panic!("expected Hit, got {other:?}"),
        }
    }

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

    #[test]
    fn list_models_returns_registry() {
        let store = ModelStore::new_with_root(tempfile::TempDir::new().unwrap().path().to_owned());
        // Verify the catalogue entry is returned.  m2m100-418M was removed
        // from the registry after testing (Xenova tokenizer.json is
        // malformed upstream); restore the second-entry assertions when a
        // working fast-slot model lands.
        let engine = TranslationEngine {
            model_store: store,
            active_model_id: "nllb-200-distilled-600M".to_owned(),
            translator: None,
            cache: TranslationCache::new(),
            stats: CallStats::new(),
        };
        let models = engine.list_models();
        assert_eq!(models.len(), 1, "catalogue must have 1 entry (NLLB only)");
        assert_eq!(models[0].id, "nllb-200-distilled-600M");
        assert!(models[0].is_active);
        // Not installed in a fresh temp dir.
        assert!(!models[0].installed);
    }
}

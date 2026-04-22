//! Multi-model registry — download, verify, and delete offline translation models.
//!
//! Refactored from M5 single-model downloader to a catalogue-based registry.
//! The registry is a `&'static [ModelSpec]` constant; users download selectively.
//!
//! Existing users of `nllb-200-distilled-600M` are unaffected: the registry
//! entry for that ID resolves to the same path layout that M5 already wrote.
//!
//! # Models in the catalogue
//!
//! | Slot       | ID                        | Download | Architecture |
//! |------------|---------------------------|----------|--------------|
//! | "balanced" | nllb-200-distilled-600M   | ~912 MB  | Nllb         |
//! | "fast"     | m2m100-418M               | ~642 MB  | M2M100       |
//!
//! # Download layout
//!
//! ```text
//! ~/Library/Application Support/com.beamview.Beamview/models/
//!   nllb-200-distilled-600M/
//!     encoder_model_quantized.onnx
//!     decoder_model_merged_quantized.onnx
//!     tokenizer.json
//!     config.json
//!     .ready
//!   m2m100-418M/
//!     encoder_model_quantized.onnx
//!     decoder_model_merged_quantized.onnx
//!     tokenizer.json
//!     tokenizer_config.json
//!     sentencepiece.bpe.model
//!     config.json
//!     .ready
//! ```
//!
//! # SHA-256 pins
//!
//! All hashes were sourced from the HuggingFace LFS API on 2026-04-22 and
//! verified against `huggingface.co/api/models/{repo}?blobs=true`.  Re-pin if
//! the upstream revision changes.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use reqwest::Client;
use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::translation::types::{ModelStatus, ModelStoreError};

/// How long to wait between the first attempt and the single automatic retry.
const RETRY_BACKOFF_SECS: u64 = 2;

/// Sentinel file written after all files are verified.
const READY_SENTINEL: &str = ".ready";

// ── Model architecture ────────────────────────────────────────────────────────

/// Which decoding architecture a model uses.
///
/// This controls how `Translator` loads the model files, which language-forcing
/// token IDs it inserts, and how it constructs the encoder input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelArch {
    /// NLLB-200 family (Meta AI).  Source language is prepended by the
    /// tokenizer's TemplateProcessing post-processor (`eng_Latn` = 256047).
    /// Target language is forced via `forced_bos_token_id` (`tha_Thai` = 256175).
    Nllb,
    /// M2M-100 family (Meta AI).  Source language prefix `__en__` (= 128022)
    /// must be prepended manually to the encoder input_ids.  Target language
    /// is forced via `forced_bos_token_id` (`__th__` = 128090).
    M2M100,
}

// ── File spec ─────────────────────────────────────────────────────────────────

/// A single file to download for a model.
#[derive(Clone, Copy, Debug)]
pub struct FileSpec {
    /// HTTPS URL on HuggingFace.
    pub url: &'static str,
    /// Expected lowercase hex SHA-256 (from HF LFS API `?blobs=true`).
    pub sha256: &'static str,
    /// Filename to write inside the model directory.
    pub filename: &'static str,
}

// ── Model spec ────────────────────────────────────────────────────────────────

/// Describes one model in the catalogue.
#[derive(Clone, Copy, Debug)]
pub struct ModelSpec {
    /// Stable identifier — used as the on-disk directory name and in IPC.
    pub id: &'static str,
    /// Human-readable name shown in the UI.
    pub display_name: &'static str,
    /// Short description for the UI.
    pub description: &'static str,
    /// Estimated total download size in bytes (sum of all `files`).
    pub size_bytes: u64,
    /// Decoding architecture.
    pub arch: ModelArch,
    /// Ordered list of files to download (processed sequentially).
    pub files: &'static [FileSpec],
}

// ── Catalogue constants ───────────────────────────────────────────────────────
//
// Hash source: `curl -s https://huggingface.co/api/models/{repo}?blobs=true`
// Date:        2026-04-22

// ── NLLB-200-distilled-600M (balanced slot) ───────────────────────────────────

const NLLB_600M_FILES: &[FileSpec] = &[
    FileSpec {
        url: "https://huggingface.co/Xenova/nllb-200-distilled-600M/resolve/main/config.json",
        sha256: "52f035acb54ac80e5ef7fe78d6967f8ddf8e8799f078d1a92a2c8168d8ff4a20",
        filename: "config.json",
    },
    FileSpec {
        url: "https://huggingface.co/Xenova/nllb-200-distilled-600M/resolve/main/tokenizer.json",
        sha256: "8ac789ad7dabea44d41537822d48c516ba358374c51813e2cba78c006e150c94",
        filename: "tokenizer.json",
    },
    FileSpec {
        url: "https://huggingface.co/Xenova/nllb-200-distilled-600M/resolve/main/onnx/encoder_model_quantized.onnx",
        sha256: "5cde664eacba07a62f198857ec6c06e09572b1ebb77c8137f1fa99ac604a3a28",
        filename: "encoder_model_quantized.onnx",
    },
    FileSpec {
        url: "https://huggingface.co/Xenova/nllb-200-distilled-600M/resolve/main/onnx/decoder_model_merged_quantized.onnx",
        sha256: "dd66608c2a4194e78f95548fa0e64f24302303698c5b09fa8e1f9e16ec00676b",
        filename: "decoder_model_merged_quantized.onnx",
    },
];

// ── m2m100-418M (fast slot) ───────────────────────────────────────────────────

const M2M100_418M_FILES: &[FileSpec] = &[
    FileSpec {
        url: "https://huggingface.co/Xenova/m2m100_418M/resolve/main/config.json",
        sha256: "1dbdf77ddc7809acd4c54ccf0eab46f840b40174afb1b6f6de8787244e832938",
        filename: "config.json",
    },
    FileSpec {
        url: "https://huggingface.co/Xenova/m2m100_418M/resolve/main/tokenizer.json",
        sha256: "03d9e111731c2d71f39a2c2a88499743e4c251385d07f0384b4349a23ba54363",
        filename: "tokenizer.json",
    },
    FileSpec {
        url: "https://huggingface.co/Xenova/m2m100_418M/resolve/main/tokenizer_config.json",
        sha256: "bacfd4b9da25a61e01f17abe660465f616c9a1a3f5e23ab9ad3326c3788f2d9f",
        filename: "tokenizer_config.json",
    },
    FileSpec {
        url: "https://huggingface.co/Xenova/m2m100_418M/resolve/main/sentencepiece.bpe.model",
        sha256: "d8f7c76ed2a5e0822be39f0a4f95a55eb19c78f4593ce609e2edbc2aea4d380a",
        filename: "sentencepiece.bpe.model",
    },
    FileSpec {
        url: "https://huggingface.co/Xenova/m2m100_418M/resolve/main/onnx/encoder_model_quantized.onnx",
        sha256: "13a94e354a9140764eb81102d77d3ec6952d796e6f113c651eeb3c3443da0386",
        filename: "encoder_model_quantized.onnx",
    },
    FileSpec {
        url: "https://huggingface.co/Xenova/m2m100_418M/resolve/main/onnx/decoder_model_merged_quantized.onnx",
        sha256: "007654bcabb6cea6fd3bde34ce933137b431330b3755781145d7b6906270b45a",
        filename: "decoder_model_merged_quantized.onnx",
    },
];

/// The complete model catalogue.
///
/// First entry is the default model (used when `active_model_id` is absent from
/// the config file, preserving back-compat with configs written before model
/// picker existed).
pub static MODEL_REGISTRY: &[ModelSpec] = &[
    ModelSpec {
        id: "nllb-200-distilled-600M",
        display_name: "NLLB-200 Balanced (600M int8)",
        description: "คุณภาพการแปลสูง ขนาดดาวน์โหลด ~912 MB ต้องการพื้นที่ ~912 MB",
        size_bytes: 419_120_483 + 475_505_771 + 17_331_224 + 873,
        arch: ModelArch::Nllb,
        files: NLLB_600M_FILES,
    },
    ModelSpec {
        id: "m2m100-418M",
        display_name: "M2M-100 Fast (418M int8)",
        description: "แปลเร็วกว่า NLLB เล็กน้อย คุณภาพใกล้เคียงกัน ขนาดดาวน์โหลด ~632 MB",
        size_bytes: 287_856_370 + 344_128_178 + 7_988_527 + 2_423_393 + 1_813 + 908,
        arch: ModelArch::M2M100,
        files: M2M100_418M_FILES,
    },
];

// ── ModelStore ────────────────────────────────────────────────────────────────

/// Manages the on-disk ONNX model files for all catalogue entries.
///
/// One `ModelStore` is constructed at app startup.  It resolves the base
/// data directory once and derives per-model subdirectories from the catalogue.
pub struct ModelStore {
    data_root: PathBuf,
}

impl ModelStore {
    /// Resolve the platform app-data directory.
    ///
    /// macOS: `~/Library/Application Support/com.beamview.Beamview/models/`
    pub fn new() -> Result<Self, ModelStoreError> {
        let proj = directories::ProjectDirs::from("com", "beamview", "Beamview")
            .ok_or(ModelStoreError::NoAppDataDir)?;
        let data_root = proj.data_dir().join("models");
        Ok(Self { data_root })
    }

    /// Construct a ModelStore pointing at a custom base directory.
    /// Intended for tests.
    #[cfg(test)]
    pub fn new_with_root(data_root: PathBuf) -> Self {
        Self { data_root }
    }

    /// Legacy single-model constructor kept for back-compat with existing tests
    /// that inject a full model dir.
    #[cfg(test)]
    pub fn new_with_dir(model_dir: PathBuf) -> Self {
        // Interpret model_dir as `<data_root>/<model_id>/` —
        // strip one path component to get data_root.
        let data_root = model_dir
            .parent()
            .map(|p| p.to_owned())
            .unwrap_or(model_dir);
        Self { data_root }
    }

    /// Return the complete catalogue.
    pub fn list() -> &'static [ModelSpec] {
        MODEL_REGISTRY
    }

    /// Resolve the directory for a given `model_id`.
    pub fn model_dir(&self, model_id: &str) -> PathBuf {
        self.data_root.join(model_id)
    }

    /// True when the `.ready` sentinel exists for `model_id`.
    pub fn is_installed(&self, model_id: &str) -> bool {
        self.model_dir(model_id).join(READY_SENTINEL).exists()
    }

    /// Returns all `ModelSpec`s whose `.ready` sentinel is present on disk.
    pub fn installed_models(&self) -> Vec<&'static ModelSpec> {
        MODEL_REGISTRY
            .iter()
            .filter(|s| self.is_installed(s.id))
            .collect()
    }

    /// Convenience: `ModelStatus` for one model (used by the engine).
    pub fn model_status(&self, model_id: &str) -> ModelStatus {
        if self.is_installed(model_id) {
            ModelStatus::Ready
        } else {
            ModelStatus::NotInstalled
        }
    }

    /// Approximate on-disk size (sum of all file sizes) for an installed model.
    ///
    /// Returns `None` when the model is not installed or the directory is
    /// inaccessible.
    pub fn installed_size_bytes(&self, model_id: &str) -> Option<u64> {
        if !self.is_installed(model_id) {
            return None;
        }
        let dir = self.model_dir(model_id);
        let total = std::fs::read_dir(&dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter_map(|e| e.metadata().ok())
            .filter(|m| m.is_file())
            .map(|m| m.len())
            .sum();
        Some(total)
    }

    /// Download all files for `model_id`, verify SHA-256, and write `.ready`.
    ///
    /// `on_progress` receives a `ModelStatus::Downloading` after each chunk.
    /// The final call is `ModelStatus::Ready` on success.
    ///
    /// The download is idempotent: already-verified files are skipped (so a
    /// partial re-download after a previous failure resumes cleanly).
    pub async fn download<F>(&self, model_id: &str, on_progress: F) -> Result<(), ModelStoreError>
    where
        F: Fn(ModelStatus),
    {
        let spec = MODEL_REGISTRY
            .iter()
            .find(|s| s.id == model_id)
            .ok_or_else(|| ModelStoreError::UnknownModel(model_id.to_owned()))?;

        let model_dir = self.model_dir(model_id);
        fs::create_dir_all(&model_dir).await?;

        let client = Client::builder()
            .user_agent("beamview/0.2.0")
            .build()
            .map_err(|e| ModelStoreError::Http(e.to_string()))?;

        let mut downloaded_total: u64 = 0;
        let total_estimate = spec.size_bytes;

        for file in spec.files {
            let dest = model_dir.join(file.filename);

            if dest.exists() && verify_sha256(&dest, file.sha256).await.unwrap_or(false) {
                downloaded_total += dest.metadata().map(|m| m.len()).unwrap_or(0);
                on_progress(ModelStatus::Downloading {
                    bytes: downloaded_total,
                    total: total_estimate,
                });
                continue;
            }

            let partial = model_dir.join(format!("{}.partial", file.filename));

            let download_result = download_file(
                &client,
                file.url,
                &partial,
                downloaded_total,
                total_estimate,
                &on_progress,
            )
            .await;

            let file_bytes = match download_result {
                Ok(n) => n,
                Err(first_err) => {
                    log::warn!(
                        "[model_store] download of {} failed ({}), retrying in {}s…",
                        file.filename,
                        first_err,
                        RETRY_BACKOFF_SECS
                    );
                    let _ = fs::remove_file(&partial).await;
                    tokio::time::sleep(std::time::Duration::from_secs(RETRY_BACKOFF_SECS)).await;
                    download_file(
                        &client,
                        file.url,
                        &partial,
                        downloaded_total,
                        total_estimate,
                        &on_progress,
                    )
                    .await?
                }
            };
            downloaded_total += file_bytes;

            let actual_hex = sha256_hex(&partial).await?;
            if actual_hex != file.sha256 {
                let _ = fs::remove_file(&partial).await;
                return Err(ModelStoreError::Sha256Mismatch {
                    file: file.filename.to_owned(),
                    expected: file.sha256.to_owned(),
                    actual: actual_hex,
                });
            }

            fs::rename(&partial, &dest).await?;
        }

        fs::write(model_dir.join(READY_SENTINEL), b"ok").await?;
        on_progress(ModelStatus::Ready);
        Ok(())
    }

    /// Delete all files for `model_id` from disk.
    ///
    /// Returns `Err(ModelStoreError::CannotDeleteActiveModel)` when called with
    /// the currently-active model (callers should check before deleting).
    /// Here we just do the I/O — the engine layer enforces the "active model"
    /// constraint before calling this.
    pub async fn delete(&self, model_id: &str) -> Result<(), ModelStoreError> {
        let model_dir = self.model_dir(model_id);
        if model_dir.exists() {
            fs::remove_dir_all(&model_dir).await?;
            log::info!("[model_store] deleted model '{model_id}'");
        }
        Ok(())
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

async fn download_file<F>(
    client: &Client,
    url: &str,
    dest: &Path,
    already_downloaded: u64,
    total: u64,
    on_progress: &F,
) -> Result<u64, ModelStoreError>
where
    F: Fn(ModelStatus),
{
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| ModelStoreError::Http(e.to_string()))?;

    if !response.status().is_success() {
        return Err(ModelStoreError::Http(format!(
            "HTTP {} for {}",
            response.status(),
            url
        )));
    }

    let mut file = fs::File::create(dest).await.map_err(ModelStoreError::Io)?;
    let mut stream = response.bytes_stream();
    let mut written: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ModelStoreError::Http(e.to_string()))?;
        file.write_all(&chunk).await.map_err(ModelStoreError::Io)?;
        written += chunk.len() as u64;
        on_progress(ModelStatus::Downloading {
            bytes: already_downloaded + written,
            total,
        });
    }

    file.flush().await.map_err(ModelStoreError::Io)?;
    Ok(written)
}

async fn verify_sha256(path: &Path, expected: &str) -> Result<bool, ModelStoreError> {
    let actual = sha256_hex(path).await?;
    Ok(actual == expected)
}

async fn sha256_hex(path: &Path) -> Result<String, ModelStoreError> {
    let bytes = fs::read(path).await.map_err(ModelStoreError::Io)?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok(format!("{:x}", h.finalize()))
}

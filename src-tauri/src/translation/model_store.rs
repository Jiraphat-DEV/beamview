//! First-run model download and integrity verification.
//!
//! Downloads the Xenova/nllb-200-distilled-600M ONNX model files (int8
//! quantized) to the platform application-data directory and verifies each
//! file with a pinned SHA-256 hash before atomically renaming them into place.
//!
//! Model: Xenova/nllb-200-distilled-600M (community ONNX export of
//!        facebook/nllb-200-distilled-600M by the Transformers.js team)
//! Variant: int8 quantized (encoder 419 MB + decoder-merged 476 MB + tokenizer)
//! Total first-run download: ~912 MB  (under the 1 GB hard cap)
//! Runtime: ort 2.x (ONNX Runtime), CPU provider
//!
//! Note: the `ort` crate ships a prebuilt ORT shared library via its
//! `download-binaries` feature (enabled in Cargo.toml), adding ~20–30 MB to
//! the macOS app bundle. This is the documented trade-off for zero-install
//! deployment.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use reqwest::Client;
use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::translation::types::{ModelStatus, ModelStoreError};

// ── Pinned download constants ─────────────────────────────────────────────────
// All URLs point to the Xenova/nllb-200-distilled-600M repo on HuggingFace.
// Hashes were computed with `curl -L <url> | shasum -a 256` on 2026-04-21.
// Re-pin if you update the model revision.

/// int8 quantized encoder (419 MB).
const ENCODER_URL: &str = "https://huggingface.co/Xenova/nllb-200-distilled-600M/resolve/main/onnx/encoder_model_quantized.onnx";
const ENCODER_SHA256: &str = "5cde664eacba07a62f198857ec6c06e09572b1ebb77c8137f1fa99ac604a3a28";

/// int8 quantized merged decoder with past-KV (476 MB).
/// "Merged" means the same file handles both the first (no-cache) and
/// subsequent (with-cache) decode steps via a `use_cache_branch` bool input.
const DECODER_URL: &str = "https://huggingface.co/Xenova/nllb-200-distilled-600M/resolve/main/onnx/decoder_model_merged_quantized.onnx";
const DECODER_SHA256: &str = "dd66608c2a4194e78f95548fa0e64f24302303698c5b09fa8e1f9e16ec00676b";

/// tokenizer.json — Xenova ships a tokenizers-rs compatible JSON (17.3 MB).
const TOKENIZER_URL: &str =
    "https://huggingface.co/Xenova/nllb-200-distilled-600M/resolve/main/tokenizer.json";
const TOKENIZER_SHA256: &str = "8ac789ad7dabea44d41537822d48c516ba358374c51813e2cba78c006e150c94";

/// config.json — model architecture metadata (873 bytes).
const CONFIG_URL: &str =
    "https://huggingface.co/Xenova/nllb-200-distilled-600M/resolve/main/config.json";
const CONFIG_SHA256: &str = "52f035acb54ac80e5ef7fe78d6967f8ddf8e8799f078d1a92a2c8168d8ff4a20";

/// Sentinel file written after all files are verified — its presence makes
/// `model_status()` return `Ready` without re-hashing ~900 MB on every launch.
const READY_SENTINEL: &str = ".ready";

/// Estimated total download size in bytes (encoder + decoder + tokenizer +
/// config), used for progress reporting.  Update if you change the manifest.
const TOTAL_BYTES_ESTIMATE: u64 = 419_120_483  // encoder_model_quantized.onnx
    + 476_018_397 // decoder_model_merged_quantized.onnx
    + 17_331_224  // tokenizer.json
    + 873; // config.json

// ── Download manifest ─────────────────────────────────────────────────────────

struct FileSpec {
    url: &'static str,
    sha256: &'static str,
    filename: &'static str,
}

fn download_manifest() -> [FileSpec; 4] {
    [
        FileSpec {
            url: CONFIG_URL,
            sha256: CONFIG_SHA256,
            filename: "config.json",
        },
        FileSpec {
            url: TOKENIZER_URL,
            sha256: TOKENIZER_SHA256,
            filename: "tokenizer.json",
        },
        FileSpec {
            url: ENCODER_URL,
            sha256: ENCODER_SHA256,
            filename: "encoder_model_quantized.onnx",
        },
        FileSpec {
            url: DECODER_URL,
            sha256: DECODER_SHA256,
            filename: "decoder_model_merged_quantized.onnx",
        },
    ]
}

// ── ModelStore ────────────────────────────────────────────────────────────────

/// Manages the on-disk NLLB-200 ONNX model files for the offline EN→TH
/// translator.
pub struct ModelStore {
    model_dir: PathBuf,
}

impl ModelStore {
    /// Resolves `~/Library/Application Support/com.beamview.Beamview/models/nllb-200-distilled-600M/`
    /// (or the platform-equivalent via `directories::ProjectDirs`).
    pub fn new() -> Result<Self, ModelStoreError> {
        let proj = directories::ProjectDirs::from("com", "beamview", "Beamview")
            .ok_or(ModelStoreError::NoAppDataDir)?;
        let model_dir = proj
            .data_dir()
            .join("models")
            .join("nllb-200-distilled-600M");
        Ok(Self { model_dir })
    }

    /// Absolute path to the model directory — used by `Translator::load`.
    pub fn model_dir(&self) -> &Path {
        &self.model_dir
    }

    /// Returns the current model status without downloading anything.
    ///
    /// Uses the `.ready` sentinel file as a cheap proxy for "all files are
    /// present and verified" so we do not re-hash ~900 MB on every launch.
    pub fn model_status(&self) -> ModelStatus {
        if self.model_dir.join(READY_SENTINEL).exists() {
            ModelStatus::Ready
        } else {
            ModelStatus::NotInstalled
        }
    }

    /// Download all model files, verify their SHA-256 hashes, and write the
    /// `.ready` sentinel on success.
    ///
    /// `on_progress` is called after each chunk arrives; the caller can use
    /// it to drive a UI progress bar (wired up in M4 via `ModelDownloadModal`).
    pub async fn download<F>(&self, on_progress: F) -> Result<(), ModelStoreError>
    where
        F: Fn(ModelStatus),
    {
        fs::create_dir_all(&self.model_dir).await?;

        let client = Client::builder()
            .user_agent("beamview/0.2.0")
            .build()
            .map_err(|e| ModelStoreError::Http(e.to_string()))?;

        let manifest = download_manifest();
        let mut downloaded_total: u64 = 0;

        for spec in &manifest {
            let dest = self.model_dir.join(spec.filename);

            // Skip already-verified files (e.g. partial re-download after failure).
            if dest.exists() && verify_sha256(&dest, spec.sha256).await.unwrap_or(false) {
                // Estimate bytes for progress even for cached files.
                downloaded_total += dest.metadata().map(|m| m.len()).unwrap_or(0);
                on_progress(ModelStatus::Downloading {
                    bytes: downloaded_total,
                    total: TOTAL_BYTES_ESTIMATE,
                });
                continue;
            }

            // Download to a `.partial` sibling, then atomic rename.
            let partial = self.model_dir.join(format!("{}.partial", spec.filename));
            downloaded_total += download_file(
                &client,
                spec.url,
                &partial,
                downloaded_total,
                TOTAL_BYTES_ESTIMATE,
                &on_progress,
            )
            .await?;

            // Verify hash before committing.
            let actual_hex = sha256_hex(&partial).await?;
            if actual_hex != spec.sha256 {
                // Remove the bad partial file so a retry starts fresh.
                let _ = fs::remove_file(&partial).await;
                return Err(ModelStoreError::Sha256Mismatch {
                    file: spec.filename.to_owned(),
                    expected: spec.sha256.to_owned(),
                    actual: actual_hex,
                });
            }

            // Atomic rename into place.
            fs::rename(&partial, &dest).await?;
        }

        // Write ready sentinel.
        fs::write(self.model_dir.join(READY_SENTINEL), b"ok").await?;
        on_progress(ModelStatus::Ready);
        Ok(())
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Stream-download `url` into `dest`, calling `on_progress` after each chunk.
/// Returns the number of bytes written.
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

/// Verify `path` against the expected lowercase hex SHA-256.
async fn verify_sha256(path: &Path, expected: &str) -> Result<bool, ModelStoreError> {
    let actual = sha256_hex(path).await?;
    Ok(actual == expected)
}

/// Compute the lowercase hex SHA-256 of a file.
async fn sha256_hex(path: &Path) -> Result<String, ModelStoreError> {
    let bytes = fs::read(path).await.map_err(ModelStoreError::Io)?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok(format!("{:x}", h.finalize()))
}

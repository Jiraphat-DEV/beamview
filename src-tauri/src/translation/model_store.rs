//! First-run model download and integrity verification.
//!
//! Downloads the Helsinki-NLP/opus-mt-en-mul MarianMT model files to the
//! platform application-data directory and verifies each file with a pinned
//! SHA-256 hash before atomically renaming them into place.
//!
//! Pinned model: Helsinki-NLP/opus-mt-en-mul  (MarianMT, ~310 MB fp32)
//! Safetensors from HF PR #2 (sha256 below).
//! Tokenizer JSON files are pre-converted from the SentencePiece sources and
//! hosted in this repository under `src-tauri/assets/translation/`.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use reqwest::Client;
use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::translation::types::{ModelStatus, ModelStoreError};

// ── Pinned download constants ─────────────────────────────────────────────────

/// Safetensors weights — Helsinki-NLP/opus-mt-en-mul, HF PR #2.
const MODEL_URL: &str =
    "https://huggingface.co/Helsinki-NLP/opus-mt-en-mul/resolve/refs%2Fpr%2F2/model.safetensors";
const MODEL_SHA256: &str = "dd4c874ecad8853d94415c21937c7e35ea09cc22f178b63821294ed010121436";

/// Model config — from HF main branch.
const CONFIG_URL: &str =
    "https://huggingface.co/Helsinki-NLP/opus-mt-en-mul/resolve/main/config.json";
const CONFIG_SHA256: &str = "31d6ee3edd0ae559448c63d145b96a21dbb4ceb4c160287eacafd5fca2572b37";

/// Encoder (source) tokenizer — pre-converted from source.spm + vocab.json.
/// Hosted in the beamview repository; served via GitHub raw content.
const TOKENIZER_SRC_URL: &str = "https://raw.githubusercontent.com/Jiraphat-DEV/beamview/main/src-tauri/assets/translation/tokenizer_source.json";
const TOKENIZER_SRC_SHA256: &str =
    "153f1aa39458a682597eb93548a181b75e7985d08e7add0d6b5d269d19e69806";

/// Decoder (target) tokenizer — pre-converted from target.spm + vocab.json.
const TOKENIZER_TGT_URL: &str = "https://raw.githubusercontent.com/Jiraphat-DEV/beamview/main/src-tauri/assets/translation/tokenizer_target.json";
const TOKENIZER_TGT_SHA256: &str =
    "27bed644ecb4e1270a56de993d8858ab525f0309f01db15b4165abc808aa1de6";

/// Sentinel file written after all files are verified — its presence makes
/// `model_status()` return `Ready` without re-hashing 310 MB on every launch.
const READY_SENTINEL: &str = ".ready";

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
            url: TOKENIZER_SRC_URL,
            sha256: TOKENIZER_SRC_SHA256,
            filename: "tokenizer_source.json",
        },
        FileSpec {
            url: TOKENIZER_TGT_URL,
            sha256: TOKENIZER_TGT_SHA256,
            filename: "tokenizer_target.json",
        },
        FileSpec {
            url: MODEL_URL,
            sha256: MODEL_SHA256,
            filename: "model.safetensors",
        },
    ]
}

// ── ModelStore ────────────────────────────────────────────────────────────────

/// Manages the on-disk model files for the offline EN→TH translator.
pub struct ModelStore {
    model_dir: PathBuf,
}

impl ModelStore {
    /// Resolves `~/Library/Application Support/com.beamview.app/models/opus-mt-en-mul/`
    /// (or the platform-equivalent via `directories::ProjectDirs`).
    pub fn new() -> Result<Self, ModelStoreError> {
        let proj = directories::ProjectDirs::from("com", "beamview", "Beamview")
            .ok_or(ModelStoreError::NoAppDataDir)?;
        let model_dir = proj.data_dir().join("models").join("opus-mt-en-mul");
        Ok(Self { model_dir })
    }

    /// Absolute path to the model directory — used by `Translator::load`.
    pub fn model_dir(&self) -> &Path {
        &self.model_dir
    }

    /// Returns the current model status without downloading anything.
    ///
    /// Uses the `.ready` sentinel file as a cheap proxy for "all files are
    /// present and verified" so we do not re-hash 310 MB on every launch.
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

        // Accumulate total bytes for cross-file progress reporting.
        let total_size: u64 = 310_234_136 + 2_048 + 3_995_852 + 3_995_410;
        let mut downloaded_total: u64 = 0;

        for spec in &manifest {
            let dest = self.model_dir.join(spec.filename);

            // Skip already-verified files (e.g. partial re-download after failure).
            if dest.exists() && verify_sha256(&dest, spec.sha256).await.unwrap_or(false) {
                // Estimate bytes for progress even for cached files.
                downloaded_total += dest.metadata().map(|m| m.len()).unwrap_or(0);
                on_progress(ModelStatus::Downloading {
                    bytes: downloaded_total,
                    total: total_size,
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
                total_size,
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

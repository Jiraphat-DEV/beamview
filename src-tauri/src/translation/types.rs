use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A rectangular region within a frame (pixel coordinates, top-left origin).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Result returned by `TranslationEngine::ocr_translate`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrTranslateResult {
    /// Recognised English text.
    pub en: String,
    /// Translated Thai text.
    pub th: String,
    /// Wall-clock milliseconds from entry to return.
    pub latency_ms: u64,
    /// True when the Thai string was served from the LRU cache.
    pub cache_hit: bool,
    /// True when OCR re-translation was skipped via jaro-winkler dedup.
    pub duplicate: bool,
}

/// Top-level errors produced by `TranslationEngine`.
#[derive(Debug, Error)]
pub enum EngineError {
    #[error("translation model is not ready — call download_translation_model first")]
    ModelNotReady,

    #[error("failed to initialise model store: {0}")]
    ModelStore(#[from] ModelStoreError),

    #[error("OCR failed: {0}")]
    Ocr(#[from] OcrError),

    #[error("translation failed: {0}")]
    Translate(#[from] TranslateError),

    #[error("JPEG decode failed: {0}")]
    ImageDecode(String),

    #[error("blocking task panicked: {0}")]
    BlockingPanic(String),
}

/// Errors that can be returned by the OCR module.
#[derive(Debug, Error)]
pub enum OcrError {
    #[error("invalid image: {0}")]
    InvalidImage(String),

    #[error("Vision framework error: {0}")]
    VisionFramework(String),

    #[error("no text found in the image")]
    NoTextFound,

    #[error("OCR is not supported on this platform")]
    UnsupportedPlatform,
}

// ── Translation types (added in M2) ──────────────────────────────────────────

/// Errors returned by the translation inference engine.
#[derive(Debug, Error)]
pub enum TranslateError {
    #[error("translation model is not ready — call ModelStore::download first")]
    ModelNotReady,

    #[error("inference failed: {0}")]
    InferenceFailed(String),

    #[error("tokenizer error: {0}")]
    Tokenizer(String),

    #[error("device initialisation failed: {0}")]
    DeviceInitFailed(String),
}

/// Errors returned by the model-download / integrity layer.
#[derive(Debug, Error)]
pub enum ModelStoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("SHA-256 mismatch for {file}: expected {expected}, got {actual}")]
    Sha256Mismatch {
        file: String,
        expected: String,
        actual: String,
    },

    #[error("HTTP download error: {0}")]
    Http(String),

    #[error("could not determine application data directory")]
    NoAppDataDir,

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Live status of the offline translation model.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelStatus {
    /// Model files have not been downloaded yet.
    NotInstalled,
    /// A download is in progress.
    Downloading { bytes: u64, total: u64 },
    /// All model files are present and verified.
    Ready,
    /// The last download or verify attempt failed.
    Failed { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ModelStatus::Failed` must serialise as `{ "type": "failed", "message": "..." }`
    /// — not the tuple-variant shape `{ "type": "failed", "0": "..." }` that
    /// existed in M3.  This test pins that contract so we don't regress.
    #[test]
    fn model_status_failed_json_shape() {
        let status = ModelStatus::Failed {
            message: "network timeout".to_owned(),
        };
        let json = serde_json::to_string(&status).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["type"], "failed", "type tag must be 'failed'");
        assert_eq!(
            v["message"], "network timeout",
            "field must be 'message', not '0'"
        );
        assert!(
            v.get("0").is_none(),
            "old tuple-variant '0' key must not exist"
        );
    }

    /// Round-trip all variants to confirm serde symmetry.
    #[test]
    fn model_status_all_variants_round_trip() {
        let cases: Vec<ModelStatus> = vec![
            ModelStatus::NotInstalled,
            ModelStatus::Downloading {
                bytes: 1024,
                total: 2048,
            },
            ModelStatus::Ready,
            ModelStatus::Failed {
                message: "oops".to_owned(),
            },
        ];
        for case in cases {
            let json = serde_json::to_string(&case).unwrap();
            let back: ModelStatus = serde_json::from_str(&json).unwrap();
            // Can't use PartialEq without deriving it; check type tag instead.
            let orig_tag = serde_json::to_value(&case).unwrap()["type"].clone();
            let back_tag = serde_json::to_value(&back).unwrap()["type"].clone();
            assert_eq!(orig_tag, back_tag);
        }
    }
}

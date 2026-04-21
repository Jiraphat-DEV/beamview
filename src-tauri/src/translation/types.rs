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

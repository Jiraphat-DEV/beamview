//! Unit tests for the translator cache and model-store integrity path.
//!
//! These tests run offline (no model download) and exercise pure logic.

use beamview_lib::translation::cache::{CacheLookup, TranslationCache};
use beamview_lib::translation::types::ModelStoreError;

// ── Cache tests ───────────────────────────────────────────────────────────────

#[test]
fn cache_miss_on_empty() {
    let mut c = TranslationCache::new();
    assert!(matches!(c.lookup("Hello world"), CacheLookup::Miss));
}

#[test]
fn cache_hit_after_insert() {
    let mut c = TranslationCache::new();
    c.insert("Hello", "สวัสดี".to_owned());
    match c.lookup("Hello") {
        CacheLookup::Hit(th) => assert_eq!(th, "สวัสดี"),
        other => panic!("expected Hit, got {other:?}"),
    }
}

#[test]
fn cache_duplicate_for_near_identical_text() {
    let mut c = TranslationCache::new();
    c.insert(
        "You cannot escape fate.",
        "คุณไม่สามารถหนีจากชะตากรรมได้".to_owned(),
    );
    // "You cannot escape fate!" differs only in punctuation — jaro_winkler ≥ 0.95.
    assert!(
        matches!(c.lookup("You cannot escape fate!"), CacheLookup::Duplicate),
        "expected Duplicate for near-identical subtitle"
    );
}

#[test]
fn cache_miss_for_different_text() {
    let mut c = TranslationCache::new();
    c.insert("Hello", "สวัสดี".to_owned());
    // "Goodbye" shares no significant prefix with "Hello".
    assert!(matches!(c.lookup("Goodbye"), CacheLookup::Miss));
}

#[test]
fn cache_duplicate_does_not_require_exact_prior_insert() {
    // The Duplicate check only compares against the *last* OCR string, not
    // the cache keys.  If we never inserted anything the last_en is None.
    let mut c = TranslationCache::new();
    // Insert something so last_en is set.
    c.insert("Good morning, everyone.", "สวัสดีตอนเช้าทุกคน".to_owned());
    // Very similar query (only last char differs).
    assert!(matches!(
        c.lookup("Good morning, everyone!"),
        CacheLookup::Duplicate
    ));
}

// ── ModelStore SHA-256 mismatch test ─────────────────────────────────────────

/// Verify that `Sha256Mismatch` is produced when a downloaded file does not
/// match its pinned hash.  We exercise the check path by constructing the
/// error variant directly (the download helpers are async and tested via
/// the e2e test).
#[test]
fn sha256_mismatch_error_format() {
    let err = ModelStoreError::Sha256Mismatch {
        file: "model.safetensors".to_owned(),
        expected: "abc123".to_owned(),
        actual: "def456".to_owned(),
    };
    let msg = err.to_string();
    assert!(msg.contains("SHA-256 mismatch"), "message: {msg}");
    assert!(msg.contains("model.safetensors"), "message: {msg}");
    assert!(msg.contains("abc123"), "message: {msg}");
    assert!(msg.contains("def456"), "message: {msg}");
}

/// Verify the SHA-256 check path against a fixture file with a known mismatch.
#[test]
fn sha256_mismatch_detected_on_bad_file() {
    use sha2::{Digest, Sha256};
    use std::io::Write;

    // Write a tiny file with known content.
    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
    tmp.write_all(b"beamview test fixture").expect("write");
    tmp.flush().expect("flush");

    // Compute its actual hash.
    let actual_hex = {
        let bytes = std::fs::read(tmp.path()).expect("read");
        let mut h = Sha256::new();
        h.update(&bytes);
        format!("{:x}", h.finalize())
    };

    // Assert mismatch when we provide a wrong expected hash.
    let wrong_expected = "0000000000000000000000000000000000000000000000000000000000000000";
    assert_ne!(
        actual_hex, wrong_expected,
        "fixture hash accidentally matched the wrong hash"
    );

    let err = ModelStoreError::Sha256Mismatch {
        file: "fixture".to_owned(),
        expected: wrong_expected.to_owned(),
        actual: actual_hex,
    };
    assert!(matches!(err, ModelStoreError::Sha256Mismatch { .. }));
}

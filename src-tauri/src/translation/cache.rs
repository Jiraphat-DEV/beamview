//! LRU translation cache + last-text duplicate detection.
//!
//! The cache is intentionally a plain struct with no internal locking.  The
//! orchestrator (M3) will wrap it in a `tokio::sync::Mutex`; keeping the
//! mutex outside this module means the struct stays `Send + Sync`-neutral
//! and easier to unit-test.

use lru::LruCache;
use sha2::{Digest, Sha256};
use std::num::NonZeroUsize;

use crate::translation::types::TranslateError;

/// Outcome of a cache lookup.
#[derive(Debug)]
pub enum CacheLookup {
    /// A previously computed translation was found.
    Hit(String),
    /// The new text is suspiciously similar to the last text (jaro-winkler ≥
    /// 0.95) — the caller can safely reuse the last returned translation
    /// without hitting the model.
    Duplicate,
    /// No match; the model must be invoked.
    Miss,
}

/// In-memory LRU translation cache.
///
/// Capacity: 1 000 entries, keyed by `sha256(en_text)` → Thai string.
/// Thread safety: none — the caller is expected to hold a mutex externally.
pub struct TranslationCache {
    last_en: Option<String>,
    lru: LruCache<[u8; 32], String>,
}

impl TranslationCache {
    const CAPACITY: usize = 1_000;

    pub fn new() -> Self {
        Self {
            last_en: None,
            lru: LruCache::new(NonZeroUsize::new(Self::CAPACITY).expect("capacity > 0")),
        }
    }

    /// Hash helper — produces a stable 32-byte key from an English string.
    fn key(en: &str) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(en.as_bytes());
        h.finalize().into()
    }

    /// Look up `en` in the cache.
    ///
    /// Returns:
    /// - `Hit(th)` if an exact translation exists.
    /// - `Duplicate` if the text is ≥ 95 % similar to the last seen text (the
    ///   caller should reuse the previous translation).
    /// - `Miss` otherwise.
    pub fn lookup(&mut self, en: &str) -> CacheLookup {
        // 1. Exact LRU hit.
        let k = Self::key(en);
        if let Some(th) = self.lru.get(&k) {
            return CacheLookup::Hit(th.clone());
        }

        // 2. Near-duplicate check against the last OCR string.
        if let Some(ref last) = self.last_en {
            let sim = strsim::jaro_winkler(last.as_str(), en);
            if sim >= 0.95 {
                return CacheLookup::Duplicate;
            }
        }

        CacheLookup::Miss
    }

    /// Store a completed translation in the LRU cache and update `last_en`.
    pub fn insert(&mut self, en: &str, th: String) {
        let k = Self::key(en);
        self.lru.put(k, th);
        self.last_en = Some(en.to_owned());
    }
}

impl Default for TranslationCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Dummy conversion so `TranslateError` can be created without `anyhow`.
/// (Only used in the orchestrator when the cache short-circuits.)
impl From<std::convert::Infallible> for TranslateError {
    fn from(v: std::convert::Infallible) -> Self {
        match v {}
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miss_on_empty_cache() {
        let mut c = TranslationCache::new();
        assert!(matches!(c.lookup("Hello world"), CacheLookup::Miss));
    }

    #[test]
    fn hit_after_insert() {
        let mut c = TranslationCache::new();
        c.insert("Hello", "สวัสดี".to_owned());
        assert!(matches!(c.lookup("Hello"), CacheLookup::Hit(_)));
        if let CacheLookup::Hit(th) = c.lookup("Hello") {
            assert_eq!(th, "สวัสดี");
        }
    }

    #[test]
    fn duplicate_for_near_identical_text() {
        let mut c = TranslationCache::new();
        c.insert(
            "You cannot escape fate.",
            "คุณไม่สามารถหนีจากชะตากรรมได้".to_owned(),
        );
        // Very similar text — jaro_winkler should be well above 0.95.
        assert!(matches!(
            c.lookup("You cannot escape fate!"),
            CacheLookup::Duplicate
        ));
    }

    #[test]
    fn miss_for_different_text() {
        let mut c = TranslationCache::new();
        c.insert("Hello", "สวัสดี".to_owned());
        assert!(matches!(c.lookup("Goodbye"), CacheLookup::Miss));
    }

    #[test]
    fn lru_capacity_evicts_oldest() {
        let mut c = TranslationCache::new();
        // Insert CAPACITY + 1 items; the first one should be evicted.
        for i in 0..=TranslationCache::CAPACITY {
            c.insert(&format!("text_{i}"), format!("th_{i}"));
        }
        // text_0 is the oldest and should have been evicted.
        // (The last_en near-dup check would return Duplicate, so first break the
        // similarity by querying something clearly different from text_1000.)
        let _ = c.lookup("completely_different_phrase_xyz");
        assert!(matches!(c.lookup("text_0"), CacheLookup::Miss));
    }
}

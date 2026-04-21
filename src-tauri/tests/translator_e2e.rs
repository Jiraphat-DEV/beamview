//! End-to-end integration test for the NLLB-200 offline translator.
//!
//! This test is `#[ignore]`d by default because it requires the model files
//! (~900 MB) to be either downloaded or pre-staged locally.
//!
//! # Running
//!
//! Download + test (will take several minutes on a cold start):
//! ```sh
//! cargo test --manifest-path src-tauri/Cargo.toml --release \
//!     -- --ignored translator_e2e
//! ```
//!
//! Using pre-downloaded files (skip the network):
//! ```sh
//! BEAMVIEW_LOCAL_ASSETS=/path/to/dir \
//! cargo test --manifest-path src-tauri/Cargo.toml --release \
//!     -- --ignored translator_e2e
//! ```
//! The directory pointed to by `BEAMVIEW_LOCAL_ASSETS` must contain:
//!   - `encoder_model_quantized.onnx`
//!   - `decoder_model_merged_quantized.onnx`
//!   - `tokenizer.json`
//!
//! # Quality expectations
//!
//! Results are printed verbatim for human review.  The test only asserts that
//! each output is non-empty and contains at least one Thai Unicode codepoint
//! (U+0E00–U+0E7F).  Translation quality judgement is done by the user.

use std::path::PathBuf;
use std::time::Instant;

use beamview_lib::translation::{
    model_store::ModelStore, translator::Translator, types::ModelStatus,
};

/// The five fixture sentences shared with the MarianMT quality-comparison run.
const FIXTURES: &[&str] = &[
    "You cannot escape fate.",
    "The power of the crystal is fading.",
    "I will fight to protect everyone I love.",
    "This battle will determine the future…",
    "We must not give up hope.",
];

/// P95 latency warning threshold (milliseconds).  We warn — not fail — because
/// cold-start session init can inflate the first measurement.
const P95_WARN_MS: u128 = 2000;

#[test]
#[ignore = "requires model files — run with --ignored"]
fn translator_e2e() {
    // ── 1. Resolve model directory ─────────────────────────────────────────────
    let model_dir = prepare_model_dir();

    // ── 2. Load translator ────────────────────────────────────────────────────
    println!("\n[e2e] Loading Translator from {:?} …", model_dir);
    let load_start = Instant::now();
    let mut translator =
        Translator::load(&model_dir).expect("Translator::load failed — check model files");
    println!(
        "[e2e] Translator loaded in {} ms",
        load_start.elapsed().as_millis()
    );

    // ── 3. Translate fixtures and collect latencies ────────────────────────────
    let mut latencies: Vec<u128> = Vec::with_capacity(FIXTURES.len());

    println!("\n[e2e] --- Translation results ---");
    for &sentence in FIXTURES {
        let t0 = Instant::now();
        let thai = translator
            .translate_en_to_th(sentence)
            .unwrap_or_else(|e| panic!("translate_en_to_th failed for {:?}: {e}", sentence));
        let elapsed = t0.elapsed().as_millis();
        latencies.push(elapsed);

        println!("  EN : {sentence}");
        println!("  TH : {thai}");
        println!("  ({elapsed} ms)");
        println!();

        // Assert non-empty output.
        assert!(
            !thai.is_empty(),
            "translation of {sentence:?} produced empty string"
        );

        // Assert at least one Thai Unicode codepoint (U+0E00 – U+0E7F).
        let has_thai = thai.chars().any(|c| ('\u{0E00}'..='\u{0E7F}').contains(&c));
        assert!(
            has_thai,
            "translation of {sentence:?} contains no Thai codepoints: {thai:?}"
        );
    }

    // ── 4. Latency summary ────────────────────────────────────────────────────
    latencies.sort_unstable();
    let p50 = percentile(&latencies, 50);
    let p95 = percentile(&latencies, 95);
    let max = latencies.last().copied().unwrap_or(0);

    println!("[e2e] --- Latency summary ---");
    println!("  p50: {p50} ms");
    println!("  p95: {p95} ms");
    println!("  max: {max} ms");

    if p95 > P95_WARN_MS {
        println!(
            "[e2e] WARNING: p95 ({p95} ms) exceeds {P95_WARN_MS} ms threshold — \
             consider --release mode or check CPU load"
        );
    }
}

// ── Helper functions ──────────────────────────────────────────────────────────

/// Resolve or prepare the model directory.
///
/// If `BEAMVIEW_LOCAL_ASSETS` points at a directory containing the model
/// files, they are copied into a temporary directory (to mimic the real model
/// dir layout).  Otherwise the `ModelStore` downloads them.
fn prepare_model_dir() -> PathBuf {
    if let Ok(local) = std::env::var("BEAMVIEW_LOCAL_ASSETS") {
        let local_path = PathBuf::from(&local);
        // Check that the required files exist in the local dir.
        let required = [
            "encoder_model_quantized.onnx",
            "decoder_model_merged_quantized.onnx",
            "tokenizer.json",
        ];
        let all_present = required.iter().all(|f| local_path.join(f).exists());
        if all_present {
            println!("[e2e] Using pre-staged model files from {local}");
            return local_path;
        }
        println!(
            "[e2e] BEAMVIEW_LOCAL_ASSETS={local} set but not all model files found — \
             falling back to ModelStore download"
        );
    }

    // Download via ModelStore.
    let store = ModelStore::new().expect("ModelStore::new failed");
    let model_dir = store.model_dir().to_owned();

    if !matches!(store.model_status(), ModelStatus::Ready) {
        println!("[e2e] Model not installed — downloading (~900 MB) …");
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(store.download(|status| {
            if let ModelStatus::Downloading { bytes, total } = status {
                let pct = bytes * 100 / total.max(1);
                print!("\r[e2e] Download progress: {pct}% ({bytes}/{total} bytes)   ");
            }
        }))
        .expect("model download failed");
        println!("\n[e2e] Download complete.");
    } else {
        println!("[e2e] Model already installed at {:?}", model_dir);
    }

    model_dir
}

/// Compute the p-th percentile of a *sorted* slice.
fn percentile(sorted: &[u128], p: usize) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = (sorted.len() * p).saturating_sub(1) / 100;
    sorted[idx.min(sorted.len() - 1)]
}

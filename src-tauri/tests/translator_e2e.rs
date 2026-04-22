//! End-to-end integration test for the offline EN→TH translator.
//!
//! Supports both NLLB-200-distilled-600M (balanced) and m2m100-418M (fast).
//! This test is `#[ignore]`d by default because it requires model files.
//!
//! # Running with pre-staged files
//!
//! ```sh
//! # NLLB-200-distilled-600M (default):
//! BEAMVIEW_LOCAL_ASSETS=/path/to/nllb-dir \
//! cargo test --manifest-path src-tauri/Cargo.toml --release \
//!     -- --ignored translator_e2e
//!
//! # m2m100-418M (fast):
//! BEAMVIEW_LOCAL_ASSETS=/path/to/m2m100-dir BEAMVIEW_TEST_MODEL=m2m100-418M \
//! cargo test --manifest-path src-tauri/Cargo.toml --release \
//!     -- --ignored translator_e2e
//! ```
//!
//! # Quality expectations
//!
//! Results are printed verbatim for human review.  The test asserts that each
//! output is non-empty and contains at least one Thai Unicode codepoint
//! (U+0E00–U+0E7F).  Translation quality judgement is done by the user.

use std::path::PathBuf;
use std::time::Instant;

use beamview_lib::translation::{
    model_store::{ModelArch, ModelStore, MODEL_REGISTRY},
    translator::Translator,
    types::ModelStatus,
};

/// The five fixture sentences used across milestone benchmarks.
const FIXTURES: &[&str] = &[
    "You cannot escape fate.",
    "The power of the crystal is fading.",
    "I will fight to protect everyone I love.",
    "This battle will determine the future…",
    "We must not give up hope.",
];

/// P95 latency warning threshold (milliseconds).
const P95_WARN_MS: u128 = 2000;

#[test]
#[ignore = "requires model files — run with --ignored"]
fn translator_e2e() {
    // ── 1. Determine which model to test ──────────────────────────────────────
    let model_id = std::env::var("BEAMVIEW_TEST_MODEL")
        .unwrap_or_else(|_| "nllb-200-distilled-600M".to_owned());

    let spec = MODEL_REGISTRY
        .iter()
        .find(|s| s.id == model_id)
        .unwrap_or_else(|| panic!("Unknown model id: {model_id}"));

    println!("\n[e2e] Testing model: {} ({})", spec.display_name, spec.id);

    // ── 2. Resolve model directory ─────────────────────────────────────────────
    let model_dir = prepare_model_dir(&model_id, spec.arch);

    // ── 3. Load translator ────────────────────────────────────────────────────
    println!("[e2e] Loading Translator from {:?} …", model_dir);
    let load_start = Instant::now();
    let mut translator = Translator::load(&model_dir, spec.arch)
        .unwrap_or_else(|e| panic!("Translator::load failed — check model files: {e}"));
    println!(
        "[e2e] Translator loaded in {} ms",
        load_start.elapsed().as_millis()
    );

    // ── 4. Translate fixtures and collect latencies ────────────────────────────
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

        assert!(
            !thai.is_empty(),
            "translation of {sentence:?} produced empty string"
        );

        let has_thai = thai.chars().any(|c| ('\u{0E00}'..='\u{0E7F}').contains(&c));
        assert!(
            has_thai,
            "translation of {sentence:?} contains no Thai codepoints: {thai:?}"
        );
    }

    // ── 5. Latency summary ────────────────────────────────────────────────────
    latencies.sort_unstable();
    let p50 = percentile(&latencies, 50);
    let p95 = percentile(&latencies, 95);
    let max = latencies.last().copied().unwrap_or(0);

    println!("[e2e] --- Latency summary ({}) ---", spec.id);
    println!("  p50: {p50} ms");
    println!("  p95: {p95} ms");
    println!("  max: {max} ms");

    if p95 > P95_WARN_MS {
        println!(
            "[e2e] WARNING: p95 ({p95} ms) exceeds {P95_WARN_MS} ms — \
             consider --release mode or check CPU load"
        );
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn prepare_model_dir(model_id: &str, _arch: ModelArch) -> PathBuf {
    if let Ok(local) = std::env::var("BEAMVIEW_LOCAL_ASSETS") {
        let local_path = PathBuf::from(&local);
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
            "[e2e] BEAMVIEW_LOCAL_ASSETS={local} set but not all model files found — downloading"
        );
    }

    let store = ModelStore::new().expect("ModelStore::new failed");

    if !store.is_installed(model_id) {
        println!("[e2e] Model '{model_id}' not installed — downloading …");
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(store.download(model_id, |status| {
            if let ModelStatus::Downloading { bytes, total } = status {
                let pct = bytes * 100 / total.max(1);
                print!("\r[e2e] Download: {pct}% ({bytes}/{total} bytes)   ");
            }
        }))
        .expect("model download failed");
        println!("\n[e2e] Download complete.");
    } else {
        println!("[e2e] Model '{model_id}' already installed.");
    }

    store.model_dir(model_id)
}

fn percentile(sorted: &[u128], p: usize) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = (sorted.len() * p).saturating_sub(1) / 100;
    sorted[idx.min(sorted.len() - 1)]
}

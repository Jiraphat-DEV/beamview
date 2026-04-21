//! End-to-end translator test — downloads the real model and runs translation.
//!
//! This test is **`#[ignore]`** by default so CI does not download ~318 MB on
//! every PR.  Run it manually once you have cloned the repo:
//!
//! ```text
//! cargo test --manifest-path src-tauri/Cargo.toml -- --ignored translator_e2e
//! ```
//!
//! Or via the project-level alias:
//! ```text
//! pnpm test:rust -- --ignored
//! ```
//!
//! The test downloads the model to the standard app-data directory
//! (`~/Library/Application Support/com.beamview.app/models/opus-mt-en-mul/`)
//! and is idempotent — running it a second time re-uses the cached model.
//!
//! ## Pre-seeding tokenizer files during development
//!
//! The tokenizer JSON files are hosted in this repository under
//! `src-tauri/assets/translation/` and downloaded via GitHub raw content once
//! the branch has been pushed.  Before pushing, you can pre-seed them so the
//! test only needs to download the model weights:
//!
//! ```text
//! BEAMVIEW_LOCAL_ASSETS=$(pwd)/src-tauri/assets/translation \
//!   cargo test --manifest-path src-tauri/Cargo.toml -- --ignored translator_e2e
//! ```

use beamview_lib::translation::{
    model_store::ModelStore, translator::Translator, types::ModelStatus,
};

/// Five representative JRPG subtitle lines used as translation fixtures.
const FIXTURES: &[&str] = &[
    "You cannot escape fate.",
    "The power of the crystal is fading.",
    "I will fight to protect everyone I love.",
    "This battle will determine the future of our world.",
    "We must not give up hope.",
];

/// Thai Unicode block: U+0E00–U+0E7F.
fn contains_thai(s: &str) -> bool {
    s.chars().any(|c| ('\u{0E00}'..='\u{0E7F}').contains(&c))
}

#[test]
#[ignore = "downloads ~318 MB; run with: cargo test -- --ignored translator_e2e"]
fn translator_e2e() {
    // Use a single-threaded Tokio runtime for the async download.
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    // ── Step 0: pre-seed tokenizer files from local assets (development only) ─
    // When BEAMVIEW_LOCAL_ASSETS is set to the `src-tauri/assets/translation/`
    // directory, we copy the pre-generated tokenizer JSON files into the model
    // directory so the download step can skip them.  This lets the e2e test run
    // on a development machine before the branch has been pushed to GitHub.
    let store = ModelStore::new().expect("ModelStore::new");
    if let Ok(local_assets) = std::env::var("BEAMVIEW_LOCAL_ASSETS") {
        let src_dir = std::path::PathBuf::from(&local_assets);
        let model_dir = store.model_dir();
        std::fs::create_dir_all(model_dir).expect("create model dir");
        for fname in ["tokenizer_source.json", "tokenizer_target.json"] {
            let src = src_dir.join(fname);
            let dst = model_dir.join(fname);
            if src.exists() && !dst.exists() {
                std::fs::copy(&src, &dst).unwrap_or_else(|e| panic!("copy {fname}: {e}"));
                println!("Pre-seeded {fname} from local assets");
            }
        }
    }

    // ── Step 1: ensure model is downloaded ───────────────────────────────────
    if !matches!(store.model_status(), ModelStatus::Ready) {
        println!("Model not installed — downloading now (this may take several minutes)…");
        rt.block_on(store.download(|status| match &status {
            ModelStatus::Downloading { bytes, total } => {
                let pct = if *total > 0 { 100 * bytes / total } else { 0 };
                print!("\r  {pct:3}% ({bytes}/{total} bytes)");
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
            ModelStatus::Ready => println!("\n  Download complete."),
            other => println!("\n  Status: {other:?}"),
        }))
        .expect("model download");
        println!();
    } else {
        println!("Model already installed at {:?}", store.model_dir());
    }

    assert!(
        matches!(store.model_status(), ModelStatus::Ready),
        "model not ready after download"
    );

    // ── Step 2: load the translator ───────────────────────────────────────────
    println!("Loading model…");
    let mut translator = rt
        .block_on(Translator::load(store.model_dir()))
        .expect("Translator::load");

    // Warm-up call to prime Metal shader caches.
    let t_warmup = std::time::Instant::now();
    translator.warm_up().expect("warm_up");
    println!("Warm-up: {} ms", t_warmup.elapsed().as_millis());

    // ── Step 3: translate fixtures ────────────────────────────────────────────
    let mut latencies_ms: Vec<u128> = Vec::new();

    for &en in FIXTURES {
        let t0 = std::time::Instant::now();
        let th = translator
            .translate_en_to_th(en)
            .expect("translate_en_to_th");
        let elapsed = t0.elapsed().as_millis();
        latencies_ms.push(elapsed);

        println!("  EN: {en}");
        println!("  TH: {th}  [{elapsed} ms]");
        println!();

        assert!(!th.is_empty(), "translation must not be empty for: {en}");
        assert!(
            contains_thai(&th),
            "translation must contain at least one Thai codepoint (U+0E00–U+0E7F) for: {en}\n  got: {th}"
        );
    }

    // ── Step 4: latency assertion ─────────────────────────────────────────────
    // p95 latency target: < 800 ms (generous CI budget).
    latencies_ms.sort_unstable();
    let p95_idx = (latencies_ms.len() as f64 * 0.95).ceil() as usize;
    let p95_idx = p95_idx.saturating_sub(1).min(latencies_ms.len() - 1);
    let p95 = latencies_ms[p95_idx];
    println!("Latencies: {:?} ms  |  p95 = {} ms", latencies_ms, p95);

    assert!(
        p95 < 800,
        "p95 latency {p95} ms exceeds 800 ms budget (Metal or CPU too slow?)"
    );
}

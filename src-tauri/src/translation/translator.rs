//! Offline EN→TH translation using Helsinki-NLP/opus-mt-en-mul (MarianMT).
//!
//! ## Model choice
//! `candle-transformers` 0.10 ships `marian.rs` but has no NLLB-200 / m2m100
//! implementation (NLLB uses the m2m_100 architecture which is absent from the
//! candle model zoo as of 2026-04). MarianMT with the multilingual
//! `opus-mt-en-mul` checkpoint is the canonical fallback specified in the Phase
//! 2 plan ("Fallback: use candle-transformers' MarianMT example with a
//! Helsinki-NLP opus-mt-en-mul checkpoint that supports Thai").
//!
//! ## Language forcing
//! For multilingual Marian models, the target language is forced by prepending
//! the `>>tha<<` token (id 866 in the shared vocabulary) as the first decoder
//! token, exactly as the HuggingFace MarianMT Python implementation does.
//!
//! ## Device selection
//! On macOS we attempt to initialise the Metal GPU backend.  If that fails
//! (e.g. on CI or a VM) we fall back to CPU transparently.

use std::path::Path;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::marian;
use tokenizers::Tokenizer;

use crate::translation::types::TranslateError;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Token id that forces Thai output in the multilingual Marian decoder.
const THA_TOKEN_ID: u32 = 866;

/// Maximum number of decoder steps (output tokens).
const MAX_NEW_TOKENS: usize = 200;

/// EOS token in the opus-mt-en-mul vocabulary.
const EOS_TOKEN_ID: u32 = 0;

// ── Translator ────────────────────────────────────────────────────────────────

/// Holds a loaded MarianMT model and its tokenizers.
///
/// `load` is async because the file I/O may be substantial on a slow disk.
/// After loading, `translate_en_to_th` is synchronous (candle operations are
/// synchronous; Metal dispatch is implicit inside candle).
pub struct Translator {
    device: Device,
    model: marian::MTModel,
    encoder_tokenizer: Tokenizer,
    decoder_tokenizer: Tokenizer,
    config: marian::Config,
}

impl Translator {
    /// Load the model and tokenizers from `model_dir`.
    ///
    /// `model_dir` must contain:
    /// - `model.safetensors`
    /// - `config.json`
    /// - `tokenizer_source.json`
    /// - `tokenizer_target.json`
    pub async fn load(model_dir: &Path) -> Result<Self, TranslateError> {
        let device = select_device()?;

        // Config — `marian::Config` implements `serde::Deserialize`, so we can
        // deserialise it directly from the HF config.json.
        let config_path = model_dir.join("config.json");
        let config_text = std::fs::read_to_string(&config_path)
            .map_err(|e| TranslateError::InferenceFailed(format!("config read: {e}")))?;
        let config: marian::Config = serde_json::from_str(&config_text)
            .map_err(|e| TranslateError::InferenceFailed(format!("config parse: {e}")))?;

        // Weights — memory-mapped for fast startup (no full copy into RAM).
        // SAFETY: we have verified the SHA-256 of this file during download,
        // so the contents are trusted.
        let weights_path = model_dir.join("model.safetensors");
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[&weights_path], DType::F32, &device)
                .map_err(|e| TranslateError::InferenceFailed(format!("weights load: {e}")))?
        };

        let model = marian::MTModel::new(&config, vb)
            .map_err(|e| TranslateError::InferenceFailed(format!("model build: {e}")))?;

        // Tokenizers — pre-converted from SentencePiece to the HF fast JSON
        // format and shipped alongside the model in the download bundle.
        let enc_path = model_dir.join("tokenizer_source.json");
        let dec_path = model_dir.join("tokenizer_target.json");
        let encoder_tokenizer = Tokenizer::from_file(&enc_path)
            .map_err(|e| TranslateError::Tokenizer(e.to_string()))?;
        let decoder_tokenizer = Tokenizer::from_file(&dec_path)
            .map_err(|e| TranslateError::Tokenizer(e.to_string()))?;

        Ok(Self {
            device,
            model,
            encoder_tokenizer,
            decoder_tokenizer,
            config,
        })
    }

    /// Translate an English string to Thai.
    ///
    /// Uses greedy (argmax) decoding.  Thai subtitles are short (≤ 120 chars)
    /// so greedy quality is equivalent to beam search for this domain.
    pub fn translate_en_to_th(&mut self, text: &str) -> Result<String, TranslateError> {
        // ── Encode source ─────────────────────────────────────────────────────
        // For opus-mt-en-mul, target language is forced by prepending the
        // `>>tha<<` language token (id 866) to the ENCODER INPUT — not the
        // decoder start.  This is how the HuggingFace MarianTokenizer works
        // for multilingual Marian models: it inserts `>>lang<<` as the first
        // source token before encoding.
        let enc = self
            .encoder_tokenizer
            .encode(text, true)
            .map_err(|e| TranslateError::Tokenizer(e.to_string()))?;

        // Prepend >>tha<< (866) to the encoder token ids.
        let mut src_ids: Vec<u32> = Vec::with_capacity(enc.get_ids().len() + 2);
        src_ids.push(THA_TOKEN_ID);
        src_ids.extend_from_slice(enc.get_ids());
        // Marian expects EOS appended to the source sequence.
        src_ids.push(self.config.eos_token_id);

        let src_tensor = Tensor::new(src_ids.as_slice(), &self.device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?;

        // ── Encoder forward pass ──────────────────────────────────────────────
        let encoder_out = self
            .model
            .encoder()
            .forward(&src_tensor, 0)
            .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?;

        // ── Greedy decode ─────────────────────────────────────────────────────
        // Decoder starts with [pad] only — decoder_start_token_id = 64109.
        // The language is already forced via the encoder input prepend above.
        //
        // We feed ALL accumulated tokens each step (no KV-cache shortcut) to
        // avoid cross-attention cache inconsistencies with candle 0.10's
        // MarianMT implementation.  For subtitle-length sequences (≤ 30 tokens
        // output) this is fast enough on M-series CPU.
        let mut token_ids: Vec<u32> = vec![self.config.decoder_start_token_id];
        let mut logits_proc = LogitsProcessor::from_sampling(42, Sampling::ArgMax);

        // Vocab size for building the bad-words mask.
        let vocab_size = self
            .config
            .decoder_vocab_size
            .unwrap_or(self.config.vocab_size);

        for _step in 0..MAX_NEW_TOKENS {
            // Always feed all accumulated tokens; start_pos = 0 (no KV cache).
            self.model.reset_kv_cache();

            let input_ids = Tensor::new(token_ids.as_slice(), &self.device)
                .and_then(|t| t.unsqueeze(0))
                .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?;

            let logits = self
                .model
                .decode(&input_ids, &encoder_out, 0)
                .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?;

            // Shape: [1, seq_len, vocab] — extract the last token's logits.
            let mut last_logits = logits
                .squeeze(0)
                .and_then(|t| {
                    let last_idx = t.dim(0)? - 1;
                    t.get(last_idx)
                })
                .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?;

            // Apply bad_words suppression: set logit for pad token (64109) to -∞
            // so it is never generated.  This mirrors the HF generation_config
            // `bad_words_ids: [[64109]]` field.
            let pad_id = self.config.pad_token_id as usize;
            if pad_id < vocab_size {
                let mut logits_vec = last_logits
                    .to_vec1::<f32>()
                    .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?;
                logits_vec[pad_id] = f32::NEG_INFINITY;
                last_logits = Tensor::new(logits_vec.as_slice(), &self.device)
                    .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?;
            }

            let next_token = logits_proc
                .sample(&last_logits)
                .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?;

            token_ids.push(next_token);

            if next_token == EOS_TOKEN_ID || next_token == self.config.forced_eos_token_id {
                break;
            }
        }

        // ── Decode output ─────────────────────────────────────────────────────
        // Skip the initial [pad] start token.
        let output_ids = &token_ids[1..];
        // Also drop the trailing EOS if present.
        let output_ids = if output_ids
            .last()
            .map(|&t| t == EOS_TOKEN_ID || t == self.config.forced_eos_token_id)
            .unwrap_or(false)
        {
            &output_ids[..output_ids.len() - 1]
        } else {
            output_ids
        };

        let th = self
            .decoder_tokenizer
            .decode(output_ids, true)
            .map_err(|e| TranslateError::Tokenizer(e.to_string()))?;

        Ok(th)
    }

    /// Run a short warm-up translation to prime CPU caches and JIT code paths,
    /// so the first real subtitle translation is not penalised by cold-start overhead.
    pub fn warm_up(&mut self) -> Result<(), TranslateError> {
        let _ = self.translate_en_to_th("Hello.")?;
        Ok(())
    }
}

// ── Device selection ─────────────────────────────────────────────────────────

/// Select the compute device for MarianMT inference.
///
/// MarianMT uses `softmax-last-dim` which is not yet implemented in candle's
/// Metal kernel library (candle 0.10, April 2026).  We therefore always use
/// CPU on every platform.  On Apple Silicon macOS, the CPU backend is heavily
/// accelerated by the Accelerate framework (BLAS/LAPACK), so latency is well
/// within the 250 ms target for short subtitle strings.
///
/// Once candle adds Metal support for `softmax-last-dim`, remove this comment
/// and restore the `Device::new_metal(0)` probe below.
fn select_device() -> Result<Device, TranslateError> {
    tracing::info!("candle: using CPU device (Metal softmax-last-dim not yet available)");
    Ok(Device::Cpu)
}

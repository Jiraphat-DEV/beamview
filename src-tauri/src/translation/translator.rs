//! Offline EN→TH translator using NLLB-200-distilled-600M via ONNX Runtime.
//!
//! Model: Xenova/nllb-200-distilled-600M (int8 quantized ONNX export)
//! Files loaded from the model directory (populated by `ModelStore::download`):
//!   - `encoder_model_quantized.onnx`
//!   - `decoder_model_merged_quantized.onnx`
//!   - `tokenizer.json`
//!
//! Architecture: M2M100ForConditionalGeneration (NLLB variant)
//! - 12-layer, 1024-dim, 16-head transformer (encoder + decoder)
//! - The "merged" decoder ONNX handles both the first decode step
//!   (use_cache_branch = false) and subsequent steps (use_cache_branch = true).
//!
//! Decoding strategy: greedy (argmax at each step), max 200 new tokens.
//!
//! Language forcing:
//!   - Source: the NLLB tokenizer's TemplateProcessing post-processor
//!     automatically prepends `eng_Latn` (id 256047) to the encoder input_ids.
//!   - Target: we set `decoder_start_token_id = EOS (2)` on step 0, then force
//!     the first generated token to `tha_Thai` (256175) — equivalent to
//!     `forced_bos_token_id` in HuggingFace's generation API.
//!
//! Thread safety: `Translator` requires `&mut self` because `ort::Session`
//! takes `&mut self` on `.run()`. Wrap in `tokio::sync::Mutex<Translator>`
//! at the M3 orchestrator layer.

use std::path::Path;

use ndarray::{Array1, Array2, Array4};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use tokenizers::Tokenizer;

use crate::translation::types::TranslateError;

// ── CoreML cache directory ────────────────────────────────────────────────────

/// Where the CoreML compiled model artefacts are cached between launches.
/// Using a persistent cache avoids the 10–30 s compile-on-first-use overhead
/// on every app restart.  The directory is created lazily by the ORT runtime.
#[cfg(target_os = "macos")]
fn coreml_cache_dir() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("com", "beamview", "Beamview")
        .map(|proj| proj.cache_dir().join("coreml"))
}

// ── NLLB token constants ──────────────────────────────────────────────────────

/// `eng_Latn` special token id (256047).
/// The tokenizer's TemplateProcessing prepends this automatically.
#[allow(dead_code)]
const ENG_LATN_TOKEN_ID: i64 = 256_047;

/// `tha_Thai` special token id (256175).
/// Forced as the first generated output token to select Thai as the target
/// language (equivalent to `forced_bos_token_id` in HF's generation API).
const THA_THAI_TOKEN_ID: i64 = 256_175;

/// `</s>` / EOS token id (2).  Also used as `decoder_start_token_id`.
const EOS_TOKEN_ID: i64 = 2;

/// Maximum new tokens to generate per translation call.
const MAX_NEW_TOKENS: usize = 200;

/// Number of decoder (and encoder cross-attention) layers in the 600M model.
const N_LAYERS: usize = 12;

/// Number of attention heads per layer.
const N_HEADS: usize = 16;

/// Dimensionality of each attention head key/value vector
/// (d_model / n_heads = 1024 / 16 = 64).
const HEAD_DIM: usize = 64;

// ── Translator ────────────────────────────────────────────────────────────────

/// Offline EN→TH translator backed by NLLB-200-distilled-600M ONNX models.
///
/// Load once at startup (off main thread) via `Translator::load`, then call
/// `translate_en_to_th` for each subtitle line.
pub struct Translator {
    encoder: Session,
    decoder: Session,
    tokenizer: Tokenizer,
}

impl Translator {
    /// Load the encoder + decoder ONNX models and the tokenizer from
    /// `model_dir`.  Must be called off the main thread.
    ///
    /// Expected files in `model_dir`:
    ///  - `encoder_model_quantized.onnx`
    ///  - `decoder_model_merged_quantized.onnx`
    ///  - `tokenizer.json`
    pub fn load(model_dir: &Path) -> Result<Self, TranslateError> {
        let encoder_path = model_dir.join("encoder_model_quantized.onnx");
        let decoder_path = model_dir.join("decoder_model_merged_quantized.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");

        if !encoder_path.exists() || !decoder_path.exists() || !tokenizer_path.exists() {
            return Err(TranslateError::ModelNotReady);
        }

        // ── Build execution-provider list ──────────────────────────────────────
        // On macOS we prefer CoreML (targets Apple Neural Engine / Metal GPU)
        // and fall back to CPU automatically if CoreML rejects the model
        // (e.g. unsupported quantization layout).  ORT logs a warning and
        // continues on the CPU provider — the session build does NOT fail.
        //
        // On Linux / Windows the CoreML feature is not compiled in, so only
        // the CPU provider is registered.
        let encoder = build_session(&encoder_path, 1)?;
        let decoder = build_session(&decoder_path, 2)?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| TranslateError::Tokenizer(e.to_string()))?;

        Ok(Self {
            encoder,
            decoder,
            tokenizer,
        })
    }

    /// Translate `text` from English to Thai.
    ///
    /// Synchronous, CPU-bound.  For async contexts, wrap in
    /// `tokio::task::spawn_blocking`.
    pub fn translate_en_to_th(&mut self, text: &str) -> Result<String, TranslateError> {
        // ── 1. Tokenize ────────────────────────────────────────────────────────
        // The NLLB tokenizer's TemplateProcessing post-processor automatically
        // produces: [eng_Latn(256047), tok1, ..., tokN, </s>(2)]
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| TranslateError::Tokenizer(e.to_string()))?;

        let input_ids_vec: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let seq_len = input_ids_vec.len();

        let enc_input = make_i64_tensor(
            "encoder input_ids",
            Array2::from_shape_vec((1, seq_len), input_ids_vec)
                .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?,
        )?;
        let enc_mask_arr = Array2::<i64>::ones((1, seq_len));
        let enc_mask_tensor = make_i64_tensor("attention_mask", enc_mask_arr.clone())?;

        // ── 2. Encoder forward pass ────────────────────────────────────────────
        let enc_out = self
            .encoder
            .run(vec![
                ("input_ids".to_owned(), enc_input),
                ("attention_mask".to_owned(), enc_mask_tensor),
            ])
            .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?;

        // last_hidden_state: [1, seq_len, 1024]
        let hidden3: ndarray::Array3<f32> = {
            let view: ndarray::ArrayViewD<f32> =
                enc_out["last_hidden_state"]
                    .try_extract_array()
                    .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?;
            view.into_dimensionality::<ndarray::Ix3>()
                .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?
                .to_owned()
        };
        drop(enc_out);

        // ── 3. Decoder step 0 (init, no cache) ───────────────────────────────
        // decoder_start_token_id = 2 (EOS) with use_cache_branch = false.
        // This initialises the encoder cross-attention KV cache.
        let dec_start = make_i64_tensor(
            "decoder input_ids",
            Array2::from_shape_vec((1, 1), vec![EOS_TOKEN_ID])
                .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?,
        )?;
        let use_cache_false = make_bool_tensor("use_cache_branch", Array1::from_vec(vec![false]))?;
        let use_cache_true = Array1::from_vec(vec![true]);

        // Empty past-KV tensors: [batch=1, heads=16, past_seq=0, head_dim=64]
        let empty_kv: Array4<f32> = Array4::zeros((1, N_HEADS, 0, HEAD_DIM));

        let step0_out = {
            let mut inputs: Vec<(String, ort::value::DynValue)> = vec![
                (
                    "encoder_attention_mask".to_owned(),
                    make_i64_tensor("encoder_attention_mask", enc_mask_arr.clone())?,
                ),
                ("input_ids".to_owned(), dec_start),
                (
                    "encoder_hidden_states".to_owned(),
                    make_f32_tensor("encoder_hidden_states", hidden3.clone())?,
                ),
                ("use_cache_branch".to_owned(), use_cache_false),
            ];
            for i in 0..N_LAYERS {
                push_kv_input(&mut inputs, i, &empty_kv)?;
            }
            self.decoder
                .run(inputs)
                .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?
        };

        // Extract KV cache from step-0 outputs.
        let mut dec_kv_key: Vec<Array4<f32>> = Vec::with_capacity(N_LAYERS);
        let mut dec_kv_val: Vec<Array4<f32>> = Vec::with_capacity(N_LAYERS);
        let mut enc_kv_key: Vec<Array4<f32>> = Vec::with_capacity(N_LAYERS);
        let mut enc_kv_val: Vec<Array4<f32>> = Vec::with_capacity(N_LAYERS);

        for i in 0..N_LAYERS {
            dec_kv_key.push(extract_kv4(
                &step0_out,
                &format!("present.{i}.decoder.key"),
            )?);
            dec_kv_val.push(extract_kv4(
                &step0_out,
                &format!("present.{i}.decoder.value"),
            )?);
            enc_kv_key.push(extract_kv4(
                &step0_out,
                &format!("present.{i}.encoder.key"),
            )?);
            enc_kv_val.push(extract_kv4(
                &step0_out,
                &format!("present.{i}.encoder.value"),
            )?);
        }
        drop(step0_out);

        // ── 4. Greedy decode loop ──────────────────────────────────────────────
        // Step 1 forces next_token = THA_THAI_TOKEN_ID.
        // Subsequent steps: argmax over logits.

        let mut output_ids: Vec<i64> = Vec::with_capacity(64);
        let mut next_token: i64 = THA_THAI_TOKEN_ID;

        for _step in 0..MAX_NEW_TOKENS {
            output_ids.push(next_token);

            let step_ids = make_i64_tensor(
                "decoder input_ids",
                Array2::from_shape_vec((1, 1), vec![next_token])
                    .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?,
            )?;

            let step_out = {
                let mut inputs: Vec<(String, ort::value::DynValue)> = vec![
                    (
                        "encoder_attention_mask".to_owned(),
                        make_i64_tensor("encoder_attention_mask", enc_mask_arr.clone())?,
                    ),
                    ("input_ids".to_owned(), step_ids),
                    (
                        "encoder_hidden_states".to_owned(),
                        make_f32_tensor("encoder_hidden_states", hidden3.clone())?,
                    ),
                    (
                        "use_cache_branch".to_owned(),
                        make_bool_tensor("use_cache_branch", use_cache_true.clone())?,
                    ),
                ];
                for i in 0..N_LAYERS {
                    push_kv_cache_input(
                        &mut inputs,
                        i,
                        &dec_kv_key[i],
                        &dec_kv_val[i],
                        &enc_kv_key[i],
                        &enc_kv_val[i],
                    )?;
                }
                self.decoder
                    .run(inputs)
                    .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?
            };

            // logits: [1, 1, vocab_size]
            let logits_view: ndarray::ArrayViewD<f32> = step_out["logits"]
                .try_extract_array()
                .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?;
            let vocab_len = logits_view.shape()[2];

            next_token = greedy_argmax(logits_view.as_slice().unwrap_or(&[]), vocab_len);

            // Update decoder KV (encoder KV is fixed after step 0).
            for i in 0..N_LAYERS {
                dec_kv_key[i] = extract_kv4(&step_out, &format!("present.{i}.decoder.key"))?;
                dec_kv_val[i] = extract_kv4(&step_out, &format!("present.{i}.decoder.value"))?;
            }
            drop(step_out);

            if next_token == EOS_TOKEN_ID {
                break;
            }
        }

        // ── 5. Detokenize ─────────────────────────────────────────────────────
        let output_ids_u32: Vec<u32> = output_ids
            .iter()
            .copied()
            .filter(|&id| id != EOS_TOKEN_ID)
            .map(|id| id as u32)
            .collect();

        self.tokenizer
            .decode(&output_ids_u32, true)
            .map_err(|e| TranslateError::Tokenizer(e.to_string()))
    }
}

// ── Private tensor-construction helpers ──────────────────────────────────────

fn make_i64_tensor<D: ndarray::Dimension + 'static>(
    name: &str,
    array: ndarray::Array<i64, D>,
) -> Result<ort::value::DynValue, TranslateError> {
    Tensor::from_array(array)
        .map(|t| t.into_dyn())
        .map_err(|e| TranslateError::InferenceFailed(format!("{name}: {e}")))
}

fn make_f32_tensor<D: ndarray::Dimension + 'static>(
    name: &str,
    array: ndarray::Array<f32, D>,
) -> Result<ort::value::DynValue, TranslateError> {
    Tensor::from_array(array)
        .map(|t| t.into_dyn())
        .map_err(|e| TranslateError::InferenceFailed(format!("{name}: {e}")))
}

fn make_bool_tensor<D: ndarray::Dimension + 'static>(
    name: &str,
    array: ndarray::Array<bool, D>,
) -> Result<ort::value::DynValue, TranslateError> {
    Tensor::from_array(array)
        .map(|t| t.into_dyn())
        .map_err(|e| TranslateError::InferenceFailed(format!("{name}: {e}")))
}

/// Push empty KV inputs for all 4 KV slots of layer `i`.
fn push_kv_input(
    inputs: &mut Vec<(String, ort::value::DynValue)>,
    i: usize,
    empty: &Array4<f32>,
) -> Result<(), TranslateError> {
    for part in [
        "decoder.key",
        "decoder.value",
        "encoder.key",
        "encoder.value",
    ] {
        inputs.push((
            format!("past_key_values.{i}.{part}"),
            make_f32_tensor(&format!("past_key_values.{i}.{part}"), empty.clone())?,
        ));
    }
    Ok(())
}

/// Push cached KV inputs for layer `i`.
fn push_kv_cache_input(
    inputs: &mut Vec<(String, ort::value::DynValue)>,
    i: usize,
    dk: &Array4<f32>,
    dv: &Array4<f32>,
    ek: &Array4<f32>,
    ev: &Array4<f32>,
) -> Result<(), TranslateError> {
    inputs.push((
        format!("past_key_values.{i}.decoder.key"),
        make_f32_tensor(&format!("past_key_values.{i}.decoder.key"), dk.clone())?,
    ));
    inputs.push((
        format!("past_key_values.{i}.decoder.value"),
        make_f32_tensor(&format!("past_key_values.{i}.decoder.value"), dv.clone())?,
    ));
    inputs.push((
        format!("past_key_values.{i}.encoder.key"),
        make_f32_tensor(&format!("past_key_values.{i}.encoder.key"), ek.clone())?,
    ));
    inputs.push((
        format!("past_key_values.{i}.encoder.value"),
        make_f32_tensor(&format!("past_key_values.{i}.encoder.value"), ev.clone())?,
    ));
    Ok(())
}

/// Extract a named past-KV tensor from `SessionOutputs` as an owned `Array4`.
fn extract_kv4(
    outputs: &ort::session::SessionOutputs<'_>,
    name: &str,
) -> Result<Array4<f32>, TranslateError> {
    let view: ndarray::ArrayViewD<f32> = outputs[name]
        .try_extract_array()
        .map_err(|e| TranslateError::InferenceFailed(format!("{name}: {e}")))?;
    view.into_dimensionality::<ndarray::Ix4>()
        .map(|v| v.to_owned())
        .map_err(|e| TranslateError::InferenceFailed(format!("{name} reshape: {e}")))
}

/// Greedy argmax over the last token position's logits.
/// `flat` is the row-major flat buffer of `[batch=1, seq=1, vocab]` logits.
fn greedy_argmax(flat: &[f32], vocab_len: usize) -> i64 {
    if flat.is_empty() || vocab_len == 0 {
        return EOS_TOKEN_ID;
    }
    let offset = flat.len().saturating_sub(vocab_len);
    flat[offset..]
        .iter()
        .copied()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Less))
        .map(|(idx, _)| idx as i64)
        .unwrap_or(EOS_TOKEN_ID)
}

// ── Session builder helper ────────────────────────────────────────────────────

/// Build an ORT session from `model_path` using the best available execution
/// provider on this platform.
///
/// On macOS the session is configured with:
///   1. CoreML EP (targeting CPU + Neural Engine — widest compatibility)
///   2. CPU EP as automatic fallback
///
/// If CoreML cannot support the model's operators (e.g. int8 quantisation
/// layout is unsupported), ORT logs a warning and silently falls back to CPU.
/// The session creation itself does not fail in this case.
///
/// On Linux / Windows only the CPU provider is registered (the `coreml` Cargo
/// feature is not compiled in for those targets).
fn build_session(model_path: &Path, intra_threads: usize) -> Result<Session, TranslateError> {
    let mut builder = Session::builder()
        .map_err(|e| TranslateError::DeviceInitFailed(e.to_string()))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| TranslateError::DeviceInitFailed(e.to_string()))?
        .with_intra_threads(intra_threads)
        .map_err(|e| TranslateError::DeviceInitFailed(e.to_string()))?;

    // Register execution providers.  The list is tried left-to-right; any EP
    // that cannot handle a subgraph falls back to the next in the list.
    // `fail_silently()` is the default, so a missing / incompatible CoreML EP
    // only logs a warning rather than returning an error.
    #[cfg(target_os = "macos")]
    {
        use ort::ep;

        // Cache compiled CoreML artefacts between launches so the second
        // session load is fast.  If we can't determine the cache dir we
        // skip caching — CoreML still works, just recompiles each launch.
        let coreml_ep = if let Some(cache) = coreml_cache_dir() {
            log::info!("[translator] CoreML cache dir: {}", cache.display());
            ep::CoreML::default()
                .with_compute_units(ep::coreml::ComputeUnits::CPUAndNeuralEngine)
                .with_model_cache_dir(cache.to_string_lossy())
                .build()
        } else {
            ep::CoreML::default()
                .with_compute_units(ep::coreml::ComputeUnits::CPUAndNeuralEngine)
                .build()
        };

        builder = builder
            .with_execution_providers([coreml_ep, ep::CPU::default().build()])
            .map_err(|e| TranslateError::DeviceInitFailed(e.to_string()))?;

        log::info!(
            "[translator] Session EPs: CoreML (CPUAndNeuralEngine) + CPU fallback for {}",
            model_path.file_name().unwrap_or_default().to_string_lossy()
        );
    }
    #[cfg(not(target_os = "macos"))]
    {
        use ort::ep;
        builder = builder
            .with_execution_providers([ep::CPU::default().build()])
            .map_err(|e| TranslateError::DeviceInitFailed(e.to_string()))?;
        log::info!(
            "[translator] Session EP: CPU only for {}",
            model_path.file_name().unwrap_or_default().to_string_lossy()
        );
    }

    builder
        .commit_from_file(model_path)
        .map_err(|e| TranslateError::DeviceInitFailed(e.to_string()))
}

// ── From<ort::Error> ──────────────────────────────────────────────────────────

impl From<ort::Error> for TranslateError {
    fn from(e: ort::Error) -> Self {
        TranslateError::InferenceFailed(e.to_string())
    }
}

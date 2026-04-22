//! Offline EN→TH translator using ONNX Runtime with multi-arch support.
//!
//! Supports two model architectures dispatched via `ModelArch`:
//!
//! ## NLLB-200 (Nllb arch)
//!   Model: Xenova/nllb-200-distilled-600M (int8 quantized ONNX)
//!   - Source: NLLB tokenizer's TemplateProcessing prepends `eng_Latn` (256047)
//!   - Target: forced BOS = `tha_Thai` (256175)
//!   - Layers: 12, Heads: 16, HeadDim: 64
//!
//! ## M2M-100 (M2M100 arch)
//!   Model: Xenova/m2m100_418M (int8 quantized ONNX)
//!   - Source: `__en__` (128022) prepended manually to encoder input_ids
//!   - Target: forced BOS = `__th__` (128090)
//!   - Layers: 12, Heads: 16, HeadDim: 64
//!
//! Both arches use the merged decoder (past-KV) and greedy decoding.
//! N_LAYERS / N_HEADS / HEAD_DIM are the same for both 418M and 600M variants.
//!
//! Thread safety: `Translator` requires `&mut self` because `ort::Session`
//! takes `&mut self` on `.run()`. Wrap in `tokio::sync::Mutex<Translator>`
//! at the engine layer.

use std::path::Path;

use ndarray::{Array1, Array2, Array4};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use tokenizers::Tokenizer;

use crate::translation::model_store::ModelArch;
use crate::translation::types::TranslateError;

// ── CoreML cache directory ────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn coreml_cache_dir() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("com", "beamview", "Beamview")
        .map(|proj| proj.cache_dir().join("coreml"))
}

// ── Arch-specific token constants ─────────────────────────────────────────────

/// `</s>` / EOS token id — shared by NLLB and M2M100.
const EOS_TOKEN_ID: i64 = 2;

/// Maximum new tokens to generate per translation call.
const MAX_NEW_TOKENS: usize = 200;

// NLLB-200 specifics.
/// `eng_Latn` special token id (256047).  Prepended by NLLB TemplateProcessing.
#[allow(dead_code)]
const NLLB_ENG_LATN_TOKEN_ID: i64 = 256_047;
/// `tha_Thai` special token id (256175).  Forced as first generated token.
const NLLB_THA_THAI_TOKEN_ID: i64 = 256_175;

// M2M-100 specifics.
/// `__en__` token id (128022).  Must be manually prepended to encoder input.
const M2M_EN_TOKEN_ID: i64 = 128_022;
/// `__th__` token id (128090).  Forced as first generated token.
const M2M_TH_TOKEN_ID: i64 = 128_090;

// ── Shared architecture constants ─────────────────────────────────────────────
// Both NLLB-600M and M2M100-418M have the same encoder/decoder dimensions.

/// Number of decoder (and cross-attention) layers.
const N_LAYERS: usize = 12;
/// Number of attention heads per layer.
const N_HEADS: usize = 16;
/// Dimensionality of each attention head key/value vector (d_model / n_heads).
const HEAD_DIM: usize = 64;

// ── Translator ────────────────────────────────────────────────────────────────

/// Offline EN→TH translator backed by ONNX models.
///
/// Load once via `Translator::load(model_dir, arch)`, then call
/// `translate_en_to_th` for each subtitle line.
pub struct Translator {
    encoder: Session,
    decoder: Session,
    tokenizer: Tokenizer,
    arch: ModelArch,
}

impl Translator {
    /// Load the encoder + decoder ONNX models and the tokenizer from `model_dir`.
    ///
    /// For NLLB: loads `encoder_model_quantized.onnx`, `decoder_model_merged_quantized.onnx`,
    /// `tokenizer.json`.
    ///
    /// For M2M100: same file names but also requires `sentencepiece.bpe.model`
    /// (loaded by the tokenizers crate automatically via `tokenizer.json`).
    pub fn load(model_dir: &Path, arch: ModelArch) -> Result<Self, TranslateError> {
        let encoder_path = model_dir.join("encoder_model_quantized.onnx");
        let decoder_path = model_dir.join("decoder_model_merged_quantized.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");

        if !encoder_path.exists() || !decoder_path.exists() || !tokenizer_path.exists() {
            return Err(TranslateError::ModelNotReady);
        }

        // ── Session configuration ──────────────────────────────────────────────
        // intra_threads: encoder is large (1 pass) → benefit from more threads.
        // decoder is called per-token → fewer threads avoid over-scheduling.
        let encoder = build_session(&encoder_path, 4)?;
        let decoder = build_session(&decoder_path, 2)?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| TranslateError::Tokenizer(e.to_string()))?;

        log::info!(
            "[translator] loaded arch={arch:?} from {}",
            model_dir.display()
        );

        Ok(Self {
            encoder,
            decoder,
            tokenizer,
            arch,
        })
    }

    /// Translate `text` from English to Thai.
    ///
    /// Synchronous, CPU-bound.  For async contexts, wrap in
    /// `tokio::task::spawn_blocking`.
    pub fn translate_en_to_th(&mut self, text: &str) -> Result<String, TranslateError> {
        match self.arch {
            ModelArch::Nllb => self.translate_nllb(text),
            ModelArch::M2M100 => self.translate_m2m100(text),
        }
    }

    // ── NLLB decode path ──────────────────────────────────────────────────────

    fn translate_nllb(&mut self, text: &str) -> Result<String, TranslateError> {
        // 1. Tokenize — NLLB TemplateProcessing prepends eng_Latn(256047) + </s>(2)
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

        // 2. Encoder pass
        let enc_out = self
            .encoder
            .run(vec![
                ("input_ids".to_owned(), enc_input),
                ("attention_mask".to_owned(), enc_mask_tensor),
            ])
            .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?;

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

        // 3. Decoder step 0 (init KV cache)
        let dec_start = make_i64_tensor(
            "decoder input_ids",
            Array2::from_shape_vec((1, 1), vec![EOS_TOKEN_ID])
                .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?,
        )?;
        let use_cache_false = make_bool_tensor("use_cache_branch", Array1::from_vec(vec![false]))?;
        let use_cache_true = Array1::from_vec(vec![true]);
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

        let (mut dec_kv_key, mut dec_kv_val, enc_kv_key, enc_kv_val) = extract_all_kv(&step0_out)?;
        drop(step0_out);

        // 4. Greedy decode — step 1 forces tha_Thai BOS
        self.greedy_decode(
            &enc_mask_arr,
            &hidden3,
            NLLB_THA_THAI_TOKEN_ID,
            &mut dec_kv_key,
            &mut dec_kv_val,
            &enc_kv_key,
            &enc_kv_val,
            &use_cache_true,
        )
    }

    // ── M2M100 decode path ────────────────────────────────────────────────────

    fn translate_m2m100(&mut self, text: &str) -> Result<String, TranslateError> {
        // 1. Tokenize
        // M2M100 tokenizer does NOT use TemplateProcessing for the source lang
        // token — we prepend `__en__` (128022) manually to the input_ids.
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| TranslateError::Tokenizer(e.to_string()))?;

        let mut input_ids_vec: Vec<i64> = vec![M2M_EN_TOKEN_ID];
        input_ids_vec.extend(encoding.get_ids().iter().map(|&id| id as i64));
        let seq_len = input_ids_vec.len();

        let enc_input = make_i64_tensor(
            "encoder input_ids",
            Array2::from_shape_vec((1, seq_len), input_ids_vec)
                .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?,
        )?;
        let enc_mask_arr = Array2::<i64>::ones((1, seq_len));
        let enc_mask_tensor = make_i64_tensor("attention_mask", enc_mask_arr.clone())?;

        // 2. Encoder pass
        let enc_out = self
            .encoder
            .run(vec![
                ("input_ids".to_owned(), enc_input),
                ("attention_mask".to_owned(), enc_mask_tensor),
            ])
            .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?;

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

        // 3. Decoder step 0 — M2M100 decoder_start_token_id = 2 (EOS), same as NLLB
        let dec_start = make_i64_tensor(
            "decoder input_ids",
            Array2::from_shape_vec((1, 1), vec![EOS_TOKEN_ID])
                .map_err(|e| TranslateError::InferenceFailed(e.to_string()))?,
        )?;
        let use_cache_false = make_bool_tensor("use_cache_branch", Array1::from_vec(vec![false]))?;
        let use_cache_true = Array1::from_vec(vec![true]);
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

        let (mut dec_kv_key, mut dec_kv_val, enc_kv_key, enc_kv_val) = extract_all_kv(&step0_out)?;
        drop(step0_out);

        // 4. Greedy decode — step 1 forces __th__ BOS
        self.greedy_decode(
            &enc_mask_arr,
            &hidden3,
            M2M_TH_TOKEN_ID,
            &mut dec_kv_key,
            &mut dec_kv_val,
            &enc_kv_key,
            &enc_kv_val,
            &use_cache_true,
        )
    }

    // ── Shared greedy decode loop ─────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn greedy_decode(
        &mut self,
        enc_mask_arr: &Array2<i64>,
        hidden3: &ndarray::Array3<f32>,
        bos_token: i64,
        dec_kv_key: &mut [Array4<f32>],
        dec_kv_val: &mut [Array4<f32>],
        enc_kv_key: &[Array4<f32>],
        enc_kv_val: &[Array4<f32>],
        use_cache_true: &Array1<bool>,
    ) -> Result<String, TranslateError> {
        let mut output_ids: Vec<i64> = Vec::with_capacity(64);
        let mut next_token: i64 = bos_token;

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

        // Detokenize
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

type KvQuad = (
    Vec<Array4<f32>>,
    Vec<Array4<f32>>,
    Vec<Array4<f32>>,
    Vec<Array4<f32>>,
);

fn extract_all_kv(outputs: &ort::session::SessionOutputs<'_>) -> Result<KvQuad, TranslateError> {
    let mut dec_kv_key = Vec::with_capacity(N_LAYERS);
    let mut dec_kv_val = Vec::with_capacity(N_LAYERS);
    let mut enc_kv_key = Vec::with_capacity(N_LAYERS);
    let mut enc_kv_val = Vec::with_capacity(N_LAYERS);

    for i in 0..N_LAYERS {
        dec_kv_key.push(extract_kv4(outputs, &format!("present.{i}.decoder.key"))?);
        dec_kv_val.push(extract_kv4(outputs, &format!("present.{i}.decoder.value"))?);
        enc_kv_key.push(extract_kv4(outputs, &format!("present.{i}.encoder.key"))?);
        enc_kv_val.push(extract_kv4(outputs, &format!("present.{i}.encoder.value"))?);
    }

    Ok((dec_kv_key, dec_kv_val, enc_kv_key, enc_kv_val))
}

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

// ── Session builder ───────────────────────────────────────────────────────────

/// Build an ORT session from `model_path`.
///
/// On macOS: CoreML (CPUAndNeuralEngine) + CPU fallback.
/// On other platforms: CPU only.
///
/// `intra_threads`: encoder benefits from more threads (1 pass per sentence);
/// decoder benefits from fewer (called per-token, scheduling overhead adds up).
fn build_session(model_path: &Path, intra_threads: usize) -> Result<Session, TranslateError> {
    let mut builder = Session::builder()
        .map_err(|e| TranslateError::DeviceInitFailed(e.to_string()))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| TranslateError::DeviceInitFailed(e.to_string()))?
        .with_intra_threads(intra_threads)
        .map_err(|e| TranslateError::DeviceInitFailed(e.to_string()))?;

    #[cfg(target_os = "macos")]
    {
        use ort::ep;

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

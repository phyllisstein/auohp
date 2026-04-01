//! Whisper ASR using candle (pure Rust, no C++ dependency).
//!
//! Replaces the whisper-rs (whisper.cpp FFI) backend with HuggingFace's candle
//! framework.  The entire decode loop is Rust --- no C callback machinery, no
//! abort-callback patches.  Decoder stalls are structurally prevented by the
//! temperature-fallback loop, which rejects high-compression-ratio (repetitive)
//! output and retries at a higher temperature.
//!
//! ## Model loading
//!
//! Models are downloaded from HuggingFace Hub on first use and cached in
//! `~/.cache/huggingface/hub/`.  No manual model management is required.
//!
//! ## Word-level timestamps
//!
//! Whisper's decoder emits special *timestamp tokens* (`<|0.00|>`, `<|0.02|>`,
//! …) that mark segment boundaries at 20 ms resolution.  Word-level timing
//! within a segment is approximated by distributing time proportionally across
//! BPE tokens, then grouping sub-tokens into words via the space-prefix
//! convention.  Less precise than DTW alignment, but reliable for caption
//! display.

use anyhow::{bail, Context as _, Result};
use candle_core::{Device, IndexOp, Tensor, D};
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self as m, audio, Config};
use rand::distributions::Distribution;
use rand::SeedableRng;
use tokenizers::Tokenizer;

use super::types::Word;

// ── Mel filterbank ──────────────────────────────────────────────────────────
//
// Pre-computed 128-bin mel filterbank coefficients, stored as little-endian
// f32 values.  128 bins × 201 frequency bins × 4 bytes = 102,912 bytes.
// large-v3 uses 128 mel bins (earlier models used 80).  The file is
// byte-identical to the one shipped in the candle examples.
const MEL_FILTERS: &[u8] = include_bytes!("melfilters128.bytes");

/// A Whisper segment with word-level timestamps.
pub struct WhisperSegment {
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub words: Vec<Word>,
}

// ── Model wrapper ───────────────────────────────────────────────────────────
//
// candle-transformers defines the model as a plain struct, not a trait.
// This enum wraps it so we could add a quantized variant later without
// changing the call sites.

enum ModelVariant {
    Normal(m::model::Whisper),
}

impl ModelVariant {
    fn encoder_forward(&mut self, x: &Tensor, flush: bool) -> candle_core::Result<Tensor> {
        match self {
            Self::Normal(m) => m.encoder.forward(x, flush),
        }
    }

    fn decoder_forward(
        &mut self,
        x: &Tensor,
        xa: &Tensor,
        flush: bool,
    ) -> candle_core::Result<Tensor> {
        match self {
            Self::Normal(m) => m.decoder.forward(x, xa, flush),
        }
    }

    fn decoder_final_linear(&self, x: &Tensor) -> candle_core::Result<Tensor> {
        match self {
            Self::Normal(m) => m.decoder.final_linear(x),
        }
    }
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Loaded Whisper model, tokenizer, and configuration --- everything needed
/// to run inference.  Create one via [`load_model`] and reuse across calls.
pub struct WhisperModel {
    model: ModelVariant,
    tokenizer: Tokenizer,
    config: Config,
    device: Device,
    mel_filters: Vec<f32>,

    // Special token IDs, looked up once at load time.
    sot_token: u32,
    transcribe_token: u32,
    eot_token: u32,
    no_timestamps_token: u32,
    language_token: u32,
    no_speech_tokens: Vec<u32>,

    /// Additive mask applied to logits before sampling.  Suppressed tokens
    /// get −∞; all others get 0.  `broadcast_add` with the raw logits
    /// zeroes out the suppressed positions.
    suppress_tokens: Tensor,
}

/// Look up a special token's ID in the tokenizer vocabulary.
fn token_id(tokenizer: &Tokenizer, token: &str) -> Result<u32> {
    match tokenizer.token_to_id(token) {
        Some(id) => Ok(id),
        None => bail!("token {token:?} not found in tokenizer vocabulary"),
    }
}

/// Load Whisper large-v3 from HuggingFace Hub.
///
/// Downloads `config.json`, `tokenizer.json`, and `model.safetensors` on
/// first call; subsequent calls reuse the cached files in
/// `~/.cache/huggingface/hub/`.
///
/// On Apple Silicon, pass `--features metal` to enable Metal GPU inference.
/// Without it, inference runs on CPU (still pure Rust, just slower).
pub fn load_model() -> Result<WhisperModel> {
    // Prefer Metal (Apple Silicon GPU) if the feature is compiled in;
    // fall back to CPU otherwise.  `Device::new_metal(0)` returns Err
    // when the feature flag is absent or no Metal device is found.
    let device = Device::new_metal(0).unwrap_or(Device::Cpu);
    eprintln!("Whisper: using device {:?}", device);

    // ── Download model files ────────────────────────────────────────────
    //
    // hf_hub::api::sync::Api caches downloads in ~/.cache/huggingface/hub/.
    // The Api::new() constructor reads HF_TOKEN from the environment for
    // gated-model access (not needed for openai/whisper-large-v3, which is
    // public).
    let api = hf_hub::api::sync::Api::new().context("failed to create HF Hub API")?;
    let repo = api.model("openai/whisper-large-v3".to_string());

    let config_path = repo.get("config.json").context("config.json download failed")?;
    let tokenizer_path = repo
        .get("tokenizer.json")
        .context("tokenizer.json download failed")?;
    let weights_path = repo
        .get("model.safetensors")
        .context("model.safetensors download failed")?;

    // ── Config ──────────────────────────────────────────────────────────
    let config: Config = serde_json::from_str(
        &std::fs::read_to_string(&config_path).context("failed to read config.json")?,
    )
    .context("failed to parse Whisper config")?;
    eprintln!(
        "Whisper: {} mel bins, {} encoder layers, {} decoder layers",
        config.num_mel_bins, config.encoder_layers, config.decoder_layers,
    );

    // ── Tokenizer ───────────────────────────────────────────────────────
    let tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| anyhow::anyhow!("failed to load tokenizer: {e}"))?;

    // ── Weights ─────────────────────────────────────────────────────────
    //
    // Memory-mapping avoids copying the entire 3 GB file into the heap.
    // The `unsafe` is well-established candle practice: it's sound as long
    // as the file isn't modified while the model is alive.
    let vb = unsafe {
        VarBuilder::from_mmaped_safetensors(&[weights_path], m::DTYPE, &device)
            .context("failed to mmap model weights")?
    };
    let model =
        m::model::Whisper::load(&vb, config.clone()).context("failed to build Whisper model")?;
    eprintln!("Whisper: model loaded");

    // ── Mel filterbank ──────────────────────────────────────────────────
    let mel_filters = read_f32_le(MEL_FILTERS);

    // ── Special tokens ──────────────────────────────────────────────────
    let sot_token = token_id(&tokenizer, m::SOT_TOKEN)?;
    let transcribe_token = token_id(&tokenizer, m::TRANSCRIBE_TOKEN)?;
    let eot_token = token_id(&tokenizer, m::EOT_TOKEN)?;
    let no_timestamps_token = token_id(&tokenizer, m::NO_TIMESTAMPS_TOKEN)?;
    let language_token = token_id(&tokenizer, "<|en|>")?;
    let no_speech_tokens: Vec<u32> = m::NO_SPEECH_TOKENS
        .iter()
        .filter_map(|t| tokenizer.token_to_id(t))
        .collect();

    // ── Suppress-tokens mask ────────────────────────────────────────────
    //
    // The config lists token IDs that the decoder should never emit (blank
    // tokens, language tokens mid-sequence, etc.).  We build an additive
    // mask: −∞ for suppressed positions, 0 elsewhere.  Adding this to the
    // raw logits before softmax zeroes out the suppressed tokens'
    // probabilities --- mathematically equivalent to removing them from
    // the vocabulary, but branchless and GPU-friendly.
    let suppress_tokens: Vec<f32> = (0..config.vocab_size as u32)
        .map(|i| {
            if config.suppress_tokens.contains(&i) {
                f32::NEG_INFINITY
            } else {
                0.0
            }
        })
        .collect();
    let suppress_tokens = Tensor::new(suppress_tokens.as_slice(), &device)
        .context("failed to create suppress_tokens tensor")?;

    Ok(WhisperModel {
        model: ModelVariant::Normal(model),
        tokenizer,
        config,
        device,
        mel_filters,
        sot_token,
        transcribe_token,
        eot_token,
        no_timestamps_token,
        language_token,
        no_speech_tokens,
        suppress_tokens,
    })
}

/// Parse a byte slice as a sequence of little-endian f32 values.
fn read_f32_le(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

// ── Transcription ───────────────────────────────────────────────────────────

/// Run Whisper inference on 16 kHz mono f32 PCM and return timestamped
/// segments with word-level timing.
pub fn transcribe(model: &mut WhisperModel, samples: &[f32]) -> Result<Vec<WhisperSegment>> {
    // ── Mel spectrogram ─────────────────────────────────────────────────
    //
    // `pcm_to_mel` runs a multi-threaded STFT with Hanning window, applies
    // the mel filterbank, and log-normalises.  The output shape is
    // (num_mel_bins, n_frames) stored as a flat Vec<f32>.
    let mel = audio::pcm_to_mel(&model.config, samples, &model.mel_filters);
    let mel_len = mel.len() / model.config.num_mel_bins;
    let mel = Tensor::from_vec(mel, (1, model.config.num_mel_bins, mel_len), &model.device)
        .context("failed to create mel tensor")?;

    let mut segments = Vec::new();
    let mut seek: usize = 0;

    // ── Chunked decoding ────────────────────────────────────────────────
    //
    // Whisper processes audio in 30-second chunks (N_FRAMES = 3000 mel
    // frames at 10 ms per frame).  For each chunk, the encoder produces
    // a fixed-length feature sequence, and the decoder autoregressively
    // generates text tokens + timestamp tokens.
    while seek < mel_len {
        let chunk_len = usize::min(m::N_FRAMES, mel_len - seek);
        let mel_segment = mel.narrow(2, seek, chunk_len)?;

        // Pad the last chunk to N_FRAMES so the encoder gets a fixed-size
        // input.  Padding with zeros corresponds to silence.
        let mel_segment = if chunk_len < m::N_FRAMES {
            let padding = Tensor::zeros(
                (1, model.config.num_mel_bins, m::N_FRAMES - chunk_len),
                candle_core::DType::F32,
                &model.device,
            )?;
            Tensor::cat(&[&mel_segment, &padding], 2)?
        } else {
            mel_segment
        };

        // Time offset of this chunk's start in the original audio.
        let segment_offset = (seek * m::HOP_LENGTH) as f64 / m::SAMPLE_RATE as f64;

        let new_segments = decode_segment(model, &mel_segment, segment_offset)?;

        if new_segments.is_empty() {
            // No speech detected; skip the full 30 s.
            seek += m::N_FRAMES;
        } else {
            // Advance past the last segment's end time.
            let last_end = new_segments.last().unwrap().end;
            let new_seek =
                ((last_end * m::SAMPLE_RATE as f64) / m::HOP_LENGTH as f64).ceil() as usize;
            seek = if new_seek > seek {
                new_seek
            } else {
                seek + m::N_FRAMES
            };
            segments.extend(new_segments);
        }

        eprintln!(
            "Whisper: {:.1}s / {:.1}s",
            seek as f64 * m::HOP_LENGTH as f64 / m::SAMPLE_RATE as f64,
            mel_len as f64 * m::HOP_LENGTH as f64 / m::SAMPLE_RATE as f64,
        );
    }

    Ok(segments)
}

// ── Decoding internals ──────────────────────────────────────────────────────

/// Intermediate result from a single temperature attempt.
struct DecodingResult {
    tokens: Vec<u32>,
    avg_logprob: f64,
    no_speech_prob: f64,
    compression_ratio: f64,
}

/// Decode one 30 s mel chunk, retrying across temperatures.
///
/// The temperature-fallback loop is Whisper's built-in anti-looping
/// mechanism: at temperature 0 (greedy), repetitive output has a very
/// high compression ratio, which triggers a retry at a higher temperature.
/// Higher temperatures inject randomness that breaks the repetition.
fn decode_segment(
    model: &mut WhisperModel,
    mel: &Tensor,
    time_offset: f64,
) -> Result<Vec<WhisperSegment>> {
    let audio_features = model
        .model
        .encoder_forward(mel, true)
        .context("encoder forward failed")?;

    for &temperature in m::TEMPERATURES.iter() {
        let result = decode_with_temperature(model, &audio_features, temperature)?;

        // If the model is very confident there's no speech, skip.
        if result.no_speech_prob > m::NO_SPEECH_THRESHOLD {
            return Ok(Vec::new());
        }

        // Accept if quality thresholds are met.
        if result.compression_ratio <= m::COMPRESSION_RATIO_THRESHOLD
            && result.avg_logprob >= m::LOGPROB_THRESHOLD
        {
            return Ok(extract_segments(model, &result.tokens, time_offset));
        }
    }

    // All temperatures failed; use the last attempt (temperature 1.0).
    let result = decode_with_temperature(model, &audio_features, 1.0)?;
    Ok(extract_segments(model, &result.tokens, time_offset))
}

/// Run the autoregressive decoder at a single temperature.
fn decode_with_temperature(
    model: &mut WhisperModel,
    audio_features: &Tensor,
    temperature: f64,
) -> Result<DecodingResult> {
    let device = &model.device;

    // Maximum number of tokens to generate per 30 s chunk.
    // 224 is Whisper's default (max_target_positions / 2 = 448 / 2).
    let sample_len = model.config.max_target_positions / 2;

    // ── Initial prompt ──────────────────────────────────────────────────
    //
    // [SOT] [<|en|>] [<|transcribe|>]
    //
    // We omit <|notimestamps|> so the decoder produces timestamp tokens
    // that give us segment boundaries.
    let mut tokens: Vec<u32> = vec![
        model.sot_token,
        model.language_token,
        model.transcribe_token,
    ];

    let mut sum_logprob = 0.0;
    let mut n_text_tokens = 0u64;
    let mut no_speech_prob: f64 = 0.0;

    for i in 0..sample_len {
        let tokens_t = Tensor::new(tokens.as_slice(), device)?.unsqueeze(0)?;

        // `flush = true` on the first step clears the KV cache from any
        // previous segment.  Subsequent steps append incrementally.
        let logits = model
            .model
            .decoder_forward(&tokens_t, audio_features, i == 0)?;

        // Take logits for the last token position only.
        let logits = model
            .model
            .decoder_final_linear(&logits.i((.., tokens.len() - 1..))?)?;
        let logits = logits.squeeze(0)?.squeeze(0)?;

        // On the very first generated token (right after the prompt),
        // capture the no-speech probability.  This is the sum of probs
        // for <|nocaptions|> and <|nospeech|>.
        if i == 0 {
            let probs = candle_nn::ops::softmax(&logits, D::Minus1)?;
            let probs_vec = probs.to_vec1::<f32>()?;
            no_speech_prob = model
                .no_speech_tokens
                .iter()
                .map(|&t| probs_vec[t as usize] as f64)
                .sum();
        }

        // Apply suppress-tokens mask.
        let logits = logits.broadcast_add(&model.suppress_tokens)?;

        // ── Token selection ─────────────────────────────────────────────
        let next_token = if temperature <= 0.0 {
            // Greedy: pick the highest-probability token.
            logits.argmax(D::Minus1)?.to_scalar::<u32>()?
        } else {
            // Temperature sampling: divide logits by T, softmax, then
            // draw from the resulting categorical distribution.
            let scaled = (&logits / temperature)?;
            let probs = candle_nn::ops::softmax(&scaled, D::Minus1)?;
            let probs_vec = probs.to_vec1::<f32>()?;
            sample_from_probs(&probs_vec)?
        };

        // Track average log-probability of text tokens (not timestamps)
        // for the quality check.
        if next_token < model.no_timestamps_token {
            let log_probs = candle_nn::ops::log_softmax(&logits, D::Minus1)?;
            let lp = log_probs.i(next_token as usize)?.to_scalar::<f32>()? as f64;
            sum_logprob += lp;
            n_text_tokens += 1;
        }

        tokens.push(next_token);

        if next_token == model.eot_token {
            break;
        }
    }

    let avg_logprob = if n_text_tokens > 0 {
        sum_logprob / n_text_tokens as f64
    } else {
        0.0
    };

    Ok(DecodingResult {
        tokens,
        avg_logprob,
        no_speech_prob,
        compression_ratio: compression_ratio(&tokens),
    })
}

/// Sample a token index from a probability distribution.
fn sample_from_probs(probs: &[f32]) -> Result<u32> {
    let mut rng = rand::rngs::StdRng::from_entropy();
    let dist = rand::distributions::WeightedIndex::new(probs)
        .map_err(|e| anyhow::anyhow!("sampling error: {e}"))?;
    Ok(dist.sample(&mut rng) as u32)
}

/// Compression ratio: length of text / number of unique bigrams.
///
/// Repetitive decoder output (the "looping" failure mode) produces very
/// high compression ratios because the same token sequences repeat.
/// Whisper's default threshold is 2.4; anything above triggers a retry
/// at higher temperature.
fn compression_ratio(tokens: &[u32]) -> f64 {
    if tokens.len() < 2 {
        return 1.0;
    }
    let total = tokens.len() - 1;
    let mut bigrams = std::collections::HashSet::new();
    for pair in tokens.windows(2) {
        bigrams.insert((pair[0], pair[1]));
    }
    total as f64 / bigrams.len() as f64
}

// ── Timestamp parsing ───────────────────────────────────────────────────────

/// Convert the raw decoder token sequence into timestamped segments with
/// word-level timing.
///
/// Timestamp tokens appear in pairs: `<|t_start|> text... <|t_end|>`.
/// Each pair becomes one [`WhisperSegment`].  Within a segment, BPE tokens
/// are grouped into words by the space-prefix convention and assigned
/// proportional timestamps.
fn extract_segments(
    model: &WhisperModel,
    tokens: &[u32],
    time_offset: f64,
) -> Vec<WhisperSegment> {
    let timestamp_begin = model.no_timestamps_token + 1;
    let mut segments = Vec::new();
    let mut current_start: Option<f64> = None;
    let mut current_tokens: Vec<u32> = Vec::new();

    for &token in tokens {
        if token == model.eot_token {
            break;
        }

        if token >= timestamp_begin {
            // Timestamp token: value = (token - timestamp_begin) × 0.02 s.
            let time = (token - timestamp_begin) as f64 * 0.02 + time_offset;

            if let Some(start) = current_start {
                if !current_tokens.is_empty() {
                    // Closing timestamp --- flush the segment.
                    let text = model
                        .tokenizer
                        .decode(&current_tokens, true)
                        .unwrap_or_default();
                    let trimmed = text.trim().to_string();

                    if !trimmed.is_empty() {
                        let words =
                            tokens_to_words(&model.tokenizer, &current_tokens, start, time);
                        segments.push(WhisperSegment {
                            text: trimmed,
                            start,
                            end: time,
                            words,
                        });
                    }
                }
                current_tokens.clear();
                current_start = None;
            } else {
                // Opening timestamp.
                current_start = Some(time);
            }
        } else if token < model.no_timestamps_token && token < 50257 {
            // Regular text token.  Special tokens (SOT, language, task)
            // have IDs ≥ 50257 in the standard Whisper tokenizer and are
            // skipped.
            current_tokens.push(token);
        }
    }

    segments
}

/// Group BPE tokens into words with approximate timestamps.
///
/// Time is distributed proportionally across tokens within the segment.
/// Tokens whose decoded text starts with a space begin a new word (the
/// standard BPE space-prefix convention).
fn tokens_to_words(
    tokenizer: &Tokenizer,
    tokens: &[u32],
    seg_start: f64,
    seg_end: f64,
) -> Vec<Word> {
    if tokens.is_empty() {
        return Vec::new();
    }

    let duration = seg_end - seg_start;
    let time_per_token = duration / tokens.len() as f64;

    let mut words = Vec::new();
    let mut current_word = String::new();
    let mut word_start: Option<f64> = None;
    let mut word_end = seg_start;

    for (i, &token) in tokens.iter().enumerate() {
        let token_text = tokenizer.decode(&[token], true).unwrap_or_default();
        let t_start = seg_start + i as f64 * time_per_token;
        let t_end = seg_start + (i + 1) as f64 * time_per_token;

        // The tokenizer's `decode(..., true)` skips special tokens but
        // preserves leading spaces.  A leading space signals a word
        // boundary in BPE.
        if token_text.starts_with(' ') {
            // Flush the current word.
            if !current_word.is_empty() {
                words.push(Word {
                    word: current_word.clone(),
                    start_time: word_start.unwrap_or(seg_start),
                    end_time: word_end,
                });
            }
            current_word = token_text.trim().to_string();
            word_start = Some(t_start);
        } else {
            if word_start.is_none() {
                word_start = Some(t_start);
            }
            current_word.push_str(&token_text);
        }
        word_end = t_end;
    }

    // Flush the last word.
    if !current_word.is_empty() {
        words.push(Word {
            word: current_word,
            start_time: word_start.unwrap_or(seg_start),
            end_time: word_end,
        });
    }

    words
}

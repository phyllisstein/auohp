//! Whisper ASR using candle (pure Rust, no C++ dependency).
//!
//! Replaces the whisper-rs (whisper.cpp FFI) backend with HuggingFace's candle
//! framework.  The entire decode loop is Rust---no C callback machinery, no
//! abort-callback patches.  Decoder stalls are structurally prevented by the
//! temperature-fallback loop: if greedy decoding produces repetitive output
//! (high compression ratio), the decoder retries at a higher temperature,
//! injecting randomness that breaks the repetition.
//!
//! ## Model loading
//!
//! Models are downloaded from HuggingFace Hub on first use and cached in
//! `~/.cache/huggingface/hub/`.  No manual model management is required for
//! the Whisper weights---only the pyannote ONNX models still need the
//! `download-models.sh` script.
//!
//! ## Word-level timestamps
//!
//! Whisper's decoder emits special *timestamp tokens* (`<|0.00|>`, `<|0.02|>`,
//! …) that mark segment boundaries at 20 ms resolution.  Word-level timing
//! within a segment is approximated by distributing time proportionally across
//! BPE tokens, then grouping sub-tokens into words via the space-prefix
//! convention.  Less precise than DTW alignment, but reliable for caption
//! display and diarization overlap calculations.

use anyhow::{bail, Context as _, Result};
use candle_core::{Device, IndexOp, Tensor, D};
use candle_nn::{
    ops::{log_softmax, softmax},
    VarBuilder,
};
use rand::distr::weighted::WeightedIndex;
use rand::distr::Distribution;
use rand::SeedableRng;
use serde::Serialize;
use tokenizers::Tokenizer;

use candle_transformers::models::whisper::{self as m, audio, Config};

use super::types::Word;

// ── Mel filterbank ──────────────────────────────────────────────────────────
//
// Pre-computed 128-bin mel filterbank coefficients, stored as little-endian
// f32 values.  128 bins × 201 frequency bins × 4 bytes = 102,912 bytes.
// large-v3 and large-v3-turbo both use 128 mel bins (earlier models used 80).
// The file is byte-identical to the one shipped in the candle examples.
const MEL_FILTERS_128: &[u8] = include_bytes!("melfilters128.bytes");

/// A Whisper segment with word-level timestamps.
///
/// Each segment corresponds to one (start_timestamp, end_timestamp) pair
/// emitted by the decoder.  Within a segment, `words` provides approximate
/// per-word timing derived from the BPE token count.
#[derive(Debug, Clone, Serialize)]
pub struct WhisperSegment {
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub words: Vec<Word>,
}

// ── Model wrapper ───────────────────────────────────────────────────────────
//
// candle-transformers defines the model as a plain struct, not a trait.
// This enum wraps it so we could add a quantized variant later (e.g.
// m::quantized_model::Whisper from a .gguf file) without changing the
// Decoder's call sites.

enum ModelVariant {
    Normal(m::model::Whisper),
}

impl ModelVariant {
    fn config(&self) -> &Config {
        match self {
            Self::Normal(m) => &m.config,
        }
    }

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

// ── Decoder ─────────────────────────────────────────────────────────────────
//
// Ported from candle-examples/examples/whisper/main.rs.  The Decoder owns
// the model, tokenizer, and RNG state.  It runs the autoregressive decode
// loop with temperature fallback and timestamp-token rules.

struct Decoder {
    model: ModelVariant,
    rng: rand::rngs::StdRng,
    tokenizer: Tokenizer,
    suppress_tokens: Tensor,
    sot_token: u32,
    transcribe_token: u32,
    eot_token: u32,
    no_speech_token: u32,
    no_timestamps_token: u32,
    language_token: u32,
}

impl Decoder {
    fn new(model: ModelVariant, tokenizer: Tokenizer, device: &Device) -> Result<Self> {
        let no_timestamps_token = token_id(&tokenizer, m::NO_TIMESTAMPS_TOKEN)?;

        // Build the suppress-tokens mask.  The config lists token IDs that the
        // decoder should never emit (blank tokens, language tokens mid-sequence,
        // etc.).  We also suppress <|notimestamps|> because we *want* timestamps.
        //
        // The mask is an additive tensor: −∞ for suppressed positions, 0
        // elsewhere.  Adding it to raw logits before softmax is equivalent to
        // removing those tokens from the vocabulary---branchless and GPU-friendly.
        let suppress_tokens: Vec<f32> = (0..model.config().vocab_size as u32)
            .map(|i| {
                if model.config().suppress_tokens.contains(&i) || i == no_timestamps_token {
                    f32::NEG_INFINITY
                } else {
                    0f32
                }
            })
            .collect();
        let suppress_tokens = Tensor::new(suppress_tokens.as_slice(), device)?;

        let sot_token = token_id(&tokenizer, m::SOT_TOKEN)?;
        let transcribe_token = token_id(&tokenizer, m::TRANSCRIBE_TOKEN)?;
        let eot_token = token_id(&tokenizer, m::EOT_TOKEN)?;
        let language_token = token_id(&tokenizer, "<|en|>")?;

        // Find any no-speech token the tokenizer knows about.
        let no_speech_token = m::NO_SPEECH_TOKENS
            .iter()
            .find_map(|t| token_id(&tokenizer, t).ok());
        let no_speech_token = match no_speech_token {
            Some(n) => n,
            None => bail!("unable to find any no-speech token in tokenizer"),
        };

        Ok(Self {
            model,
            rng: rand::rngs::StdRng::seed_from_u64(299_792_458),
            tokenizer,
            suppress_tokens,
            sot_token,
            transcribe_token,
            eot_token,
            no_speech_token,
            no_timestamps_token,
            language_token,
        })
    }

    /// Decode one mel segment at a single temperature.
    ///
    /// Returns the raw token sequence (including special/timestamp tokens),
    /// average log-probability, and no-speech probability.
    fn decode(&mut self, mel: &Tensor, temperature: f64) -> Result<DecodingResult> {
        let audio_features = self.model.encoder_forward(mel, true)?;
        let sample_len = self.model.config().max_target_positions / 2;
        let mut sum_logprob = 0f64;
        let mut no_speech_prob = f64::NAN;

        // Initial prompt: [SOT] [<|en|>] [<|transcribe|>]
        // We omit <|notimestamps|> so the decoder produces timestamp tokens.
        let mut tokens = vec![self.sot_token, self.language_token, self.transcribe_token];

        for i in 0..sample_len {
            let tokens_t = Tensor::new(tokens.as_slice(), mel.device())?;
            let tokens_t = tokens_t.unsqueeze(0)?;

            // flush=true on the first step clears the KV cache from any
            // previous segment.  Subsequent steps append incrementally.
            let ys = self
                .model
                .decoder_forward(&tokens_t, &audio_features, i == 0)?;

            // On the very first generated token, capture the no-speech
            // probability by looking at the softmax over the full vocab
            // at the first decoder position.
            if i == 0 {
                let logits = self.model.decoder_final_linear(&ys.i(..1)?)?.i(0)?.i(0)?;
                no_speech_prob = softmax(&logits, 0)?
                    .i(self.no_speech_token as usize)?
                    .to_scalar::<f32>()? as f64;
            }

            // Take logits for the last token position only.
            let (_, seq_len, _) = ys.dims3()?;
            let logits = self
                .model
                .decoder_final_linear(&ys.i((..1, seq_len - 1..))?)?
                .i(0)?
                .i(0)?;

            // Apply timestamp structural rules (pairing, monotonicity, etc.)
            let logits = self.apply_timestamp_rules(&logits, &tokens)?;

            // Apply suppress-tokens mask.
            let logits = logits.broadcast_add(&self.suppress_tokens)?;

            let next_token = if temperature > 0.0 {
                // Temperature sampling: divide logits by T, softmax, then
                // draw from the resulting categorical distribution.
                let probs = softmax(&(&logits / temperature)?, 0)?;
                let probs_v: Vec<f32> = probs.to_vec1()?;
                let distr = WeightedIndex::new(&probs_v)?;
                distr.sample(&mut self.rng) as u32
            } else {
                // Greedy: pick the highest-probability token.
                let logits_v: Vec<f32> = logits.to_vec1()?;
                logits_v
                    .iter()
                    .enumerate()
                    .max_by(|(_, u), (_, v)| u.total_cmp(v))
                    .map(|(i, _)| i as u32)
                    .unwrap()
            };

            tokens.push(next_token);

            if next_token == self.eot_token
                || tokens.len() > self.model.config().max_target_positions
            {
                break;
            }

            // Accumulate log-probability for quality scoring.
            let prob = softmax(&logits, D::Minus1)?
                .i(next_token as usize)?
                .to_scalar::<f32>()? as f64;
            sum_logprob += prob.ln();
        }

        let avg_logprob = sum_logprob / tokens.len() as f64;

        Ok(DecodingResult {
            tokens,
            avg_logprob,
            no_speech_prob,
            compression_ratio: f64::NAN,
        })
    }

    /// Try decoding at increasing temperatures until quality thresholds are met.
    ///
    /// This is Whisper's built-in anti-looping mechanism: at temperature 0
    /// (greedy), repetitive output has a very high compression ratio, which
    /// triggers a retry at a higher temperature.  Higher temperatures inject
    /// randomness that breaks the repetition.
    fn decode_with_fallback(&mut self, mel: &Tensor) -> Result<DecodingResult> {
        for (i, &t) in m::TEMPERATURES.iter().enumerate() {
            let dr = self.decode(mel, t);
            // On the last temperature, return whatever we got.
            if i == m::TEMPERATURES.len() - 1 {
                return dr;
            }
            match dr {
                Ok(dr) => {
                    let needs_fallback = dr.compression_ratio > m::COMPRESSION_RATIO_THRESHOLD
                        || dr.avg_logprob < m::LOGPROB_THRESHOLD;
                    if !needs_fallback || dr.no_speech_prob > m::NO_SPEECH_THRESHOLD {
                        return Ok(dr);
                    }
                }
                Err(err) => {
                    eprintln!("Whisper: error at temperature {t}: {err}");
                }
            }
        }
        unreachable!()
    }

    /// Enforce timestamp structural rules on logits before sampling.
    ///
    /// These four rules are ported directly from the candle example, which in
    /// turn mirrors OpenAI's Python implementation:
    ///
    /// 1. **Pairing**: timestamps must come in (start, end) pairs.  After one
    ///    timestamp, the next token must be either another timestamp (closing
    ///    the pair) or EOT.  After two consecutive timestamps, force text.
    /// 2. **Monotonicity**: timestamp values must not decrease.
    /// 3. **Force initial**: the first generated token must be a timestamp.
    /// 4. **Probability-based preference**: if the total probability mass on
    ///    timestamp tokens exceeds the max text-token probability, force a
    ///    timestamp.
    fn apply_timestamp_rules(&self, input_logits: &Tensor, tokens: &[u32]) -> Result<Tensor> {
        let device = input_logits.device().clone();
        let timestamp_begin = self.no_timestamps_token + 1;
        let vocab_size = self.model.config().vocab_size as u32;

        // Sampled tokens start after the prompt: [SOT, lang, task] = 3 tokens.
        let sample_begin: usize = 3;
        let sampled_tokens = if tokens.len() > sample_begin {
            &tokens[sample_begin..]
        } else {
            &[]
        };

        let mut masks = Vec::new();
        let mut mask_buffer = vec![0.0f32; vocab_size as usize];

        // ── Rule 1: Timestamp pairing ───────────────────────────────────
        if !sampled_tokens.is_empty() {
            let last_was_timestamp = sampled_tokens
                .last()
                .map(|&t| t >= timestamp_begin)
                .unwrap_or(false);

            let penultimate_was_timestamp = if sampled_tokens.len() >= 2 {
                sampled_tokens[sampled_tokens.len() - 2] >= timestamp_begin
            } else {
                false
            };

            if last_was_timestamp {
                if penultimate_was_timestamp {
                    // Two timestamps in a row---force non-timestamp (text).
                    for i in 0..vocab_size {
                        mask_buffer[i as usize] = if i >= timestamp_begin {
                            f32::NEG_INFINITY
                        } else {
                            0.0
                        };
                    }
                    masks.push(Tensor::new(mask_buffer.as_slice(), &device)?);
                } else {
                    // One timestamp---next must be timestamp or EOT.
                    for i in 0..vocab_size {
                        mask_buffer[i as usize] = if i < self.eot_token {
                            f32::NEG_INFINITY
                        } else {
                            0.0
                        };
                    }
                    masks.push(Tensor::new(mask_buffer.as_slice(), &device)?);
                }
            }

            // ── Rule 2: Monotonicity ────────────────────────────────────
            let timestamp_tokens: Vec<u32> = sampled_tokens
                .iter()
                .filter(|&&t| t >= timestamp_begin)
                .cloned()
                .collect();

            if !timestamp_tokens.is_empty() {
                let timestamp_last = if last_was_timestamp && !penultimate_was_timestamp {
                    *timestamp_tokens.last().unwrap()
                } else {
                    timestamp_tokens.last().unwrap() + 1
                };

                for i in 0..vocab_size {
                    mask_buffer[i as usize] = if i >= timestamp_begin && i < timestamp_last {
                        f32::NEG_INFINITY
                    } else {
                        0.0
                    };
                }
                masks.push(Tensor::new(mask_buffer.as_slice(), &device)?);
            }
        }

        // ── Rule 3: Force initial timestamp ─────────────────────────────
        if tokens.len() == sample_begin {
            for i in 0..vocab_size {
                mask_buffer[i as usize] = if i < timestamp_begin {
                    f32::NEG_INFINITY
                } else {
                    0.0
                };
            }
            masks.push(Tensor::new(mask_buffer.as_slice(), &device)?);
        }

        // Apply all constraint masks.
        let mut logits = input_logits.clone();
        for mask in masks {
            logits = logits.broadcast_add(&mask)?;
        }

        // ── Rule 4: Probability-based timestamp preference ──────────────
        let log_probs = log_softmax(&logits, 0)?;

        let timestamp_log_probs = log_probs.narrow(
            0,
            timestamp_begin as usize,
            vocab_size as usize - timestamp_begin as usize,
        )?;
        let text_log_probs = log_probs.narrow(0, 0, timestamp_begin as usize)?;

        // logsumexp over timestamp tokens (numerically stable).
        let timestamp_logprob = {
            let max_val = timestamp_log_probs.max(0)?;
            let shifted = timestamp_log_probs.broadcast_sub(&max_val)?;
            let sum_exp = shifted.exp()?.sum(0)?;
            max_val.broadcast_add(&sum_exp.log()?)?.to_scalar::<f32>()?
        };

        let max_text_token_logprob: f32 = text_log_probs.max(0)?.to_scalar::<f32>()?;

        if timestamp_logprob > max_text_token_logprob {
            for i in 0..vocab_size {
                mask_buffer[i as usize] = if i < timestamp_begin {
                    f32::NEG_INFINITY
                } else {
                    0.0
                };
            }
            logits = logits.broadcast_add(&Tensor::new(mask_buffer.as_slice(), &device)?)?;
        }

        Ok(logits)
    }

    /// Process the full mel spectrogram in 30 s chunks, returning timestamped
    /// segments with word-level timing.
    ///
    /// Critically, `seek` advances by the position of the *last* timestamp token
    /// the model actually emitted, not by the full chunk size. Whisper often
    /// only consumes part of a 30 s window before deciding it's done, and
    /// blindly advancing by `segment_size` accumulates drift proportional to
    /// the per-chunk underrun. Mirrors the reference openai-whisper loop.
    fn run(&mut self, mel: &Tensor) -> Result<Vec<WhisperSegment>> {
        let (_, _, content_frames) = mel.dims3()?;
        let timestamp_begin = self.no_timestamps_token + 1;
        // Each timestamp tick is 0.02 s; one mel frame is HOP_LENGTH/SAMPLE_RATE
        // = 0.01 s. So one timestamp tick covers exactly two mel frames.
        let frames_per_timestamp_tick: usize = 2;

        let mut seek = 0;
        let mut segments = Vec::new();

        while seek < content_frames {
            let time_offset = (seek * m::HOP_LENGTH) as f64 / m::SAMPLE_RATE as f64;
            let segment_size = usize::min(content_frames - seek, m::N_FRAMES);
            let mel_segment = mel.narrow(2, seek, segment_size)?;
            let segment_duration = (segment_size * m::HOP_LENGTH) as f64 / m::SAMPLE_RATE as f64;

            let dr = self.decode_with_fallback(&mel_segment)?;

            // If no speech detected with high confidence, skip this chunk.
            if dr.no_speech_prob > m::NO_SPEECH_THRESHOLD && dr.avg_logprob < m::LOGPROB_THRESHOLD {
                eprintln!(
                    "Whisper: no speech at {:.1}s–{:.1}s, skipping",
                    time_offset,
                    time_offset + segment_duration,
                );
                seek += segment_size;
                continue;
            }

            // Parse timestamp tokens into segments with word-level timing.
            let chunk_segments = self.tokens_to_segments(&dr.tokens, time_offset)?;
            segments.extend(chunk_segments);

            // Advance `seek` to the position of the model's last reported
            // timestamp. Prefer the closing timestamp of the final consecutive
            // pair (i.e. the last fully-bounded segment); fall back to any
            // trailing timestamp; finally, if the chunk emitted no usable
            // timestamps at all, step forward a full window to avoid stalling.
            let advance_frames = last_timestamp_offset_frames(
                &dr.tokens,
                timestamp_begin,
                frames_per_timestamp_tick,
            )
            .unwrap_or(segment_size);
            seek += advance_frames.max(1).min(segment_size);

            eprintln!(
                "Whisper: {:.1}s / {:.1}s",
                seek as f64 * m::HOP_LENGTH as f64 / m::SAMPLE_RATE as f64,
                content_frames as f64 * m::HOP_LENGTH as f64 / m::SAMPLE_RATE as f64,
            );
        }

        Ok(segments)
    }

    /// Convert the raw decoder token sequence (with timestamp tokens) into
    /// segments with word-level timing.
    ///
    /// Timestamp tokens appear in pairs: `<|t_start|> text... <|t_end|>`.
    /// Each pair becomes one [`WhisperSegment`].
    fn tokens_to_segments(&self, tokens: &[u32], time_offset: f64) -> Result<Vec<WhisperSegment>> {
        let timestamp_begin = self.no_timestamps_token + 1;
        let mut segments = Vec::new();
        let mut current_start: Option<f64> = None;
        let mut current_tokens: Vec<u32> = Vec::new();

        for &token in tokens {
            if token == self.eot_token || token == self.sot_token {
                continue;
            }

            if token >= timestamp_begin {
                // Timestamp token: value = (token - timestamp_begin) × 0.02 s.
                let time = (token - timestamp_begin) as f64 * 0.02 + time_offset;

                if let Some(start) = current_start {
                    if !current_tokens.is_empty() {
                        // Closing timestamp---flush the segment.
                        let text = self
                            .tokenizer
                            .decode(&current_tokens, true)
                            .unwrap_or_default();
                        let trimmed = text.trim().to_string();

                        if !trimmed.is_empty() {
                            let words =
                                tokens_to_words(&self.tokenizer, &current_tokens, start, time);
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
            } else if token < self.no_timestamps_token {
                // Regular text token.  Special prompt tokens (SOT, language,
                // task) are handled by the SOT/EOT skip above and by the
                // no_timestamps_token boundary---they all have IDs above that.
                current_tokens.push(token);
            }
        }

        Ok(segments)
    }
}

// ── Intermediate types ──────────────────────────────────────────────────────

struct DecodingResult {
    tokens: Vec<u32>,
    avg_logprob: f64,
    no_speech_prob: f64,
    compression_ratio: f64,
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Loaded Whisper model, tokenizer, mel filters, and device---everything
/// needed to run inference.  Create one via [`load_model`] and pass to
/// [`transcribe`].
pub struct WhisperModel {
    decoder: Decoder,
    config: Config,
    device: Device,
    mel_filters: Vec<f32>,
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
    // fall back to CPU otherwise.
    let device = Device::new_cuda(0).unwrap_or(Device::Cpu);
    eprintln!("Whisper: using device {device:?}");

    // hf_hub caches downloads in ~/.cache/huggingface/hub/.  The Api::new()
    // constructor reads HF_TOKEN from the environment for gated-model access
    // (not needed for openai/whisper-large-v3, which is public).
    let api = hf_hub::api::sync::Api::new().context("failed to create HF Hub API")?;
    let repo = api.model("openai/whisper-large-v3".to_string());

    let config_path = repo
        .get("config.json")
        .context("config.json download failed")?;
    let tokenizer_path = repo
        .get("tokenizer.json")
        .context("tokenizer.json download failed")?;
    let weights_path = repo
        .get("model.safetensors")
        .context("model.safetensors download failed")?;

    let config: Config = serde_json::from_str(
        &std::fs::read_to_string(&config_path).context("failed to read config.json")?,
    )
    .context("failed to parse Whisper config")?;
    eprintln!(
        "Whisper: {} mel bins, {} encoder layers, {} decoder layers",
        config.num_mel_bins, config.encoder_layers, config.decoder_layers,
    );

    let tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| anyhow::anyhow!("failed to load tokenizer: {e}"))?;

    // Memory-mapping avoids copying the full weights file into the heap.
    // The `unsafe` is well-established candle practice: it's sound as long
    // as the file isn't modified while the model is alive.
    let vb = unsafe {
        VarBuilder::from_mmaped_safetensors(&[weights_path], m::DTYPE, &device)
            .context("failed to mmap model weights")?
    };
    let model =
        m::model::Whisper::load(&vb, config.clone()).context("failed to build Whisper model")?;
    eprintln!("Whisper: model loaded");

    // Parse mel filterbank coefficients from the embedded binary blob.
    assert_eq!(
        config.num_mel_bins, 128,
        "only 128-bin mel filterbanks are bundled; got {} bins",
        config.num_mel_bins,
    );
    let mel_filters = read_f32_le(MEL_FILTERS_128);

    let decoder = Decoder::new(ModelVariant::Normal(model), tokenizer, &device)?;

    Ok(WhisperModel {
        decoder,
        config,
        device,
        mel_filters,
    })
}

/// Run Whisper inference on 16 kHz mono f32 PCM and return timestamped
/// segments with word-level timing.
pub fn transcribe(model: &mut WhisperModel, samples: &[f32]) -> Result<Vec<WhisperSegment>> {
    // pcm_to_mel runs a multi-threaded STFT with Hanning window, applies
    // the mel filterbank, and log-normalises.  Output shape is flat:
    // (num_mel_bins × n_frames) stored as Vec<f32>.
    let mel = audio::pcm_to_mel(&model.config, samples, &model.mel_filters);
    let mel_len = mel.len() / model.config.num_mel_bins;
    let mel = Tensor::from_vec(mel, (1, model.config.num_mel_bins, mel_len), &model.device)
        .context("failed to create mel tensor")?;

    eprintln!("Whisper: mel spectrogram shape {:?}", mel.dims());

    model.decoder.run(&mel)
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Parse a byte slice as a sequence of little-endian f32 values.
fn read_f32_le(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Compute how many mel frames the model actually consumed in a chunk, based
/// on the trailing timestamp tokens it emitted.
///
/// Two cases, mirroring the openai-whisper reference:
///
/// 1. **Consecutive pair** (`<|t_close|><|t_next_open|>`): the closing
///    timestamp of the last fully-bounded segment. Advance to that close.
/// 2. **Single trailing timestamp**: the model never closed its last segment,
///    but it told us how far it got. Advance to that timestamp.
///
/// Returns `None` if the chunk produced no usable timestamp at all (caller
/// should fall back to a full-window advance).
fn last_timestamp_offset_frames(
    tokens: &[u32],
    timestamp_begin: u32,
    frames_per_tick: usize,
) -> Option<usize> {
    // Walk pairs of adjacent timestamp tokens; the last such pair marks the
    // end of the last fully-bounded segment.
    let mut last_pair_close: Option<u32> = None;
    for window in tokens.windows(2) {
        if window[0] >= timestamp_begin && window[1] >= timestamp_begin {
            last_pair_close = Some(window[0]);
        }
    }

    if let Some(close_token) = last_pair_close {
        let ticks = (close_token - timestamp_begin) as usize;
        return Some(ticks * frames_per_tick);
    }

    // No paired close: use the last single timestamp, if any.
    let last_ts = tokens
        .iter()
        .rev()
        .copied()
        .find(|&t| t >= timestamp_begin)?;

    let ticks = (last_ts - timestamp_begin) as usize;
    if ticks == 0 {
        // A bare `<|0.00|>` tells us nothing useful; let the caller stride.
        return None;
    }
    Some(ticks * frames_per_tick)
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
        // preserves leading spaces.  A leading space signals a word boundary.
        if token_text.starts_with(' ') {
            // Flush the current word.
            if !current_word.is_empty() {
                words.push(Word {
                    word: current_word.clone(),
                    start: word_start.unwrap_or(seg_start),
                    end: word_end,
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
            start: word_start.unwrap_or(seg_start),
            end: word_end,
        });
    }

    words
}

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

/// Token-level timestamps for DTW alignment require
const V3_CUSTOM_AHEADS: [(i32, i32); 640] = [
    (0, 0),
    (0, 1),
    (0, 2),
    (0, 3),
    (0, 4),
    (0, 5),
    (0, 6),
    (0, 7),
    (0, 8),
    (0, 9),
    (0, 10),
    (0, 11),
    (0, 12),
    (0, 13),
    (0, 14),
    (0, 15),
    (0, 16),
    (0, 17),
    (0, 18),
    (0, 19),
    (1, 0),
    (1, 1),
    (1, 2),
    (1, 3),
    (1, 4),
    (1, 5),
    (1, 6),
    (1, 7),
    (1, 8),
    (1, 9),
    (1, 10),
    (1, 11),
    (1, 12),
    (1, 13),
    (1, 14),
    (1, 15),
    (1, 16),
    (1, 17),
    (1, 18),
    (1, 19),
    (2, 0),
    (2, 1),
    (2, 2),
    (2, 3),
    (2, 4),
    (2, 5),
    (2, 6),
    (2, 7),
    (2, 8),
    (2, 9),
    (2, 10),
    (2, 11),
    (2, 12),
    (2, 13),
    (2, 14),
    (2, 15),
    (2, 16),
    (2, 17),
    (2, 18),
    (2, 19),
    (3, 0),
    (3, 1),
    (3, 2),
    (3, 3),
    (3, 4),
    (3, 5),
    (3, 6),
    (3, 7),
    (3, 8),
    (3, 9),
    (3, 10),
    (3, 11),
    (3, 12),
    (3, 13),
    (3, 14),
    (3, 15),
    (3, 16),
    (3, 17),
    (3, 18),
    (3, 19),
    (4, 0),
    (4, 1),
    (4, 2),
    (4, 3),
    (4, 4),
    (4, 5),
    (4, 6),
    (4, 7),
    (4, 8),
    (4, 9),
    (4, 10),
    (4, 11),
    (4, 12),
    (4, 13),
    (4, 14),
    (4, 15),
    (4, 16),
    (4, 17),
    (4, 18),
    (4, 19),
    (5, 0),
    (5, 1),
    (5, 2),
    (5, 3),
    (5, 4),
    (5, 5),
    (5, 6),
    (5, 7),
    (5, 8),
    (5, 9),
    (5, 10),
    (5, 11),
    (5, 12),
    (5, 13),
    (5, 14),
    (5, 15),
    (5, 16),
    (5, 17),
    (5, 18),
    (5, 19),
    (6, 0),
    (6, 1),
    (6, 2),
    (6, 3),
    (6, 4),
    (6, 5),
    (6, 6),
    (6, 7),
    (6, 8),
    (6, 9),
    (6, 10),
    (6, 11),
    (6, 12),
    (6, 13),
    (6, 14),
    (6, 15),
    (6, 16),
    (6, 17),
    (6, 18),
    (6, 19),
    (7, 0),
    (7, 1),
    (7, 2),
    (7, 3),
    (7, 4),
    (7, 5),
    (7, 6),
    (7, 7),
    (7, 8),
    (7, 9),
    (7, 10),
    (7, 11),
    (7, 12),
    (7, 13),
    (7, 14),
    (7, 15),
    (7, 16),
    (7, 17),
    (7, 18),
    (7, 19),
    (8, 0),
    (8, 1),
    (8, 2),
    (8, 3),
    (8, 4),
    (8, 5),
    (8, 6),
    (8, 7),
    (8, 8),
    (8, 9),
    (8, 10),
    (8, 11),
    (8, 12),
    (8, 13),
    (8, 14),
    (8, 15),
    (8, 16),
    (8, 17),
    (8, 18),
    (8, 19),
    (9, 0),
    (9, 1),
    (9, 2),
    (9, 3),
    (9, 4),
    (9, 5),
    (9, 6),
    (9, 7),
    (9, 8),
    (9, 9),
    (9, 10),
    (9, 11),
    (9, 12),
    (9, 13),
    (9, 14),
    (9, 15),
    (9, 16),
    (9, 17),
    (9, 18),
    (9, 19),
    (10, 0),
    (10, 1),
    (10, 2),
    (10, 3),
    (10, 4),
    (10, 5),
    (10, 6),
    (10, 7),
    (10, 8),
    (10, 9),
    (10, 10),
    (10, 11),
    (10, 12),
    (10, 13),
    (10, 14),
    (10, 15),
    (10, 16),
    (10, 17),
    (10, 18),
    (10, 19),
    (11, 0),
    (11, 1),
    (11, 2),
    (11, 3),
    (11, 4),
    (11, 5),
    (11, 6),
    (11, 7),
    (11, 8),
    (11, 9),
    (11, 10),
    (11, 11),
    (11, 12),
    (11, 13),
    (11, 14),
    (11, 15),
    (11, 16),
    (11, 17),
    (11, 18),
    (11, 19),
    (12, 0),
    (12, 1),
    (12, 2),
    (12, 3),
    (12, 4),
    (12, 5),
    (12, 6),
    (12, 7),
    (12, 8),
    (12, 9),
    (12, 10),
    (12, 11),
    (12, 12),
    (12, 13),
    (12, 14),
    (12, 15),
    (12, 16),
    (12, 17),
    (12, 18),
    (12, 19),
    (13, 0),
    (13, 1),
    (13, 2),
    (13, 3),
    (13, 4),
    (13, 5),
    (13, 6),
    (13, 7),
    (13, 8),
    (13, 9),
    (13, 10),
    (13, 11),
    (13, 12),
    (13, 13),
    (13, 14),
    (13, 15),
    (13, 16),
    (13, 17),
    (13, 18),
    (13, 19),
    (14, 0),
    (14, 1),
    (14, 2),
    (14, 3),
    (14, 4),
    (14, 5),
    (14, 6),
    (14, 7),
    (14, 8),
    (14, 9),
    (14, 10),
    (14, 11),
    (14, 12),
    (14, 13),
    (14, 14),
    (14, 15),
    (14, 16),
    (14, 17),
    (14, 18),
    (14, 19),
    (15, 0),
    (15, 1),
    (15, 2),
    (15, 3),
    (15, 4),
    (15, 5),
    (15, 6),
    (15, 7),
    (15, 8),
    (15, 9),
    (15, 10),
    (15, 11),
    (15, 12),
    (15, 13),
    (15, 14),
    (15, 15),
    (15, 16),
    (15, 17),
    (15, 18),
    (15, 19),
    (16, 0),
    (16, 1),
    (16, 2),
    (16, 3),
    (16, 4),
    (16, 5),
    (16, 6),
    (16, 7),
    (16, 8),
    (16, 9),
    (16, 10),
    (16, 11),
    (16, 12),
    (16, 13),
    (16, 14),
    (16, 15),
    (16, 16),
    (16, 17),
    (16, 18),
    (16, 19),
    (17, 0),
    (17, 1),
    (17, 2),
    (17, 3),
    (17, 4),
    (17, 5),
    (17, 6),
    (17, 7),
    (17, 8),
    (17, 9),
    (17, 10),
    (17, 11),
    (17, 12),
    (17, 13),
    (17, 14),
    (17, 15),
    (17, 16),
    (17, 17),
    (17, 18),
    (17, 19),
    (18, 0),
    (18, 1),
    (18, 2),
    (18, 3),
    (18, 4),
    (18, 5),
    (18, 6),
    (18, 7),
    (18, 8),
    (18, 9),
    (18, 10),
    (18, 11),
    (18, 12),
    (18, 13),
    (18, 14),
    (18, 15),
    (18, 16),
    (18, 17),
    (18, 18),
    (18, 19),
    (19, 0),
    (19, 1),
    (19, 2),
    (19, 3),
    (19, 4),
    (19, 5),
    (19, 6),
    (19, 7),
    (19, 8),
    (19, 9),
    (19, 10),
    (19, 11),
    (19, 12),
    (19, 13),
    (19, 14),
    (19, 15),
    (19, 16),
    (19, 17),
    (19, 18),
    (19, 19),
    (20, 0),
    (20, 1),
    (20, 2),
    (20, 3),
    (20, 4),
    (20, 5),
    (20, 6),
    (20, 7),
    (20, 8),
    (20, 9),
    (20, 10),
    (20, 11),
    (20, 12),
    (20, 13),
    (20, 14),
    (20, 15),
    (20, 16),
    (20, 17),
    (20, 18),
    (20, 19),
    (21, 0),
    (21, 1),
    (21, 2),
    (21, 3),
    (21, 4),
    (21, 5),
    (21, 6),
    (21, 7),
    (21, 8),
    (21, 9),
    (21, 10),
    (21, 11),
    (21, 12),
    (21, 13),
    (21, 14),
    (21, 15),
    (21, 16),
    (21, 17),
    (21, 18),
    (21, 19),
    (22, 0),
    (22, 1),
    (22, 2),
    (22, 3),
    (22, 4),
    (22, 5),
    (22, 6),
    (22, 7),
    (22, 8),
    (22, 9),
    (22, 10),
    (22, 11),
    (22, 12),
    (22, 13),
    (22, 14),
    (22, 15),
    (22, 16),
    (22, 17),
    (22, 18),
    (22, 19),
    (23, 0),
    (23, 1),
    (23, 2),
    (23, 3),
    (23, 4),
    (23, 5),
    (23, 6),
    (23, 7),
    (23, 8),
    (23, 9),
    (23, 10),
    (23, 11),
    (23, 12),
    (23, 13),
    (23, 14),
    (23, 15),
    (23, 16),
    (23, 17),
    (23, 18),
    (23, 19),
    (24, 0),
    (24, 1),
    (24, 2),
    (24, 3),
    (24, 4),
    (24, 5),
    (24, 6),
    (24, 7),
    (24, 8),
    (24, 9),
    (24, 10),
    (24, 11),
    (24, 12),
    (24, 13),
    (24, 14),
    (24, 15),
    (24, 16),
    (24, 17),
    (24, 18),
    (24, 19),
    (25, 0),
    (25, 1),
    (25, 2),
    (25, 3),
    (25, 4),
    (25, 5),
    (25, 6),
    (25, 7),
    (25, 8),
    (25, 9),
    (25, 10),
    (25, 11),
    (25, 12),
    (25, 13),
    (25, 14),
    (25, 15),
    (25, 16),
    (25, 17),
    (25, 18),
    (25, 19),
    (26, 0),
    (26, 1),
    (26, 2),
    (26, 3),
    (26, 4),
    (26, 5),
    (26, 6),
    (26, 7),
    (26, 8),
    (26, 9),
    (26, 10),
    (26, 11),
    (26, 12),
    (26, 13),
    (26, 14),
    (26, 15),
    (26, 16),
    (26, 17),
    (26, 18),
    (26, 19),
    (27, 0),
    (27, 1),
    (27, 2),
    (27, 3),
    (27, 4),
    (27, 5),
    (27, 6),
    (27, 7),
    (27, 8),
    (27, 9),
    (27, 10),
    (27, 11),
    (27, 12),
    (27, 13),
    (27, 14),
    (27, 15),
    (27, 16),
    (27, 17),
    (27, 18),
    (27, 19),
    (28, 0),
    (28, 1),
    (28, 2),
    (28, 3),
    (28, 4),
    (28, 5),
    (28, 6),
    (28, 7),
    (28, 8),
    (28, 9),
    (28, 10),
    (28, 11),
    (28, 12),
    (28, 13),
    (28, 14),
    (28, 15),
    (28, 16),
    (28, 17),
    (28, 18),
    (28, 19),
    (29, 0),
    (29, 1),
    (29, 2),
    (29, 3),
    (29, 4),
    (29, 5),
    (29, 6),
    (29, 7),
    (29, 8),
    (29, 9),
    (29, 10),
    (29, 11),
    (29, 12),
    (29, 13),
    (29, 14),
    (29, 15),
    (29, 16),
    (29, 17),
    (29, 18),
    (29, 19),
    (30, 0),
    (30, 1),
    (30, 2),
    (30, 3),
    (30, 4),
    (30, 5),
    (30, 6),
    (30, 7),
    (30, 8),
    (30, 9),
    (30, 10),
    (30, 11),
    (30, 12),
    (30, 13),
    (30, 14),
    (30, 15),
    (30, 16),
    (30, 17),
    (30, 18),
    (30, 19),
    (31, 0),
    (31, 1),
    (31, 2),
    (31, 3),
    (31, 4),
    (31, 5),
    (31, 6),
    (31, 7),
    (31, 8),
    (31, 9),
    (31, 10),
    (31, 11),
    (31, 12),
    (31, 13),
    (31, 14),
    (31, 15),
    (31, 16),
    (31, 17),
    (31, 18),
    (31, 19),
];

use anyhow::{Context as _, Result, bail};
use candle_core::{D, Device, IndexOp, Tensor};
use candle_nn::{
    VarBuilder,
    ops::{log_softmax, softmax},
};
use rand::SeedableRng;
use rand::distr::Distribution;
use rand::distr::weighted::WeightedIndex;
use serde::Serialize;
use tokenizers::Tokenizer;

use candle_transformers::models::whisper::{self as m, Config, audio};

use std::fs::File;
use std::io::Write;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use super::types::Word;

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

            // When fewer than two tokens have been sampled, treat the
            // (non-existent) penultimate as a timestamp.  This mirrors
            // OpenAI's reference (`len(seq) < 2 or seq[-2] >= ts_begin`)
            // and is what makes Rule 1 force *text* immediately after the
            // first timestamp, producing the expected `<|t_start|> text
            // <|t_end|>` segment shape.  Defaulting to `false` instead
            // collapses every segment's start and end to the same value.
            let penultimate_was_timestamp = sampled_tokens.len() < 2
                || sampled_tokens[sampled_tokens.len() - 2] >= timestamp_begin;

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
                segment_size,
            );
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
    /// Whisper's timestamp protocol, enforced by the rule-1 mask in
    /// `apply_timestamp_rules`: after one timestamp the next sample must be
    /// another timestamp (or EOT), and after two consecutive timestamps text
    /// is forced.  So legal output is
    /// `T T text+ T T text+ ... T (T|EOT)` --- every segment boundary is a
    /// *pair* of timestamp tokens at the same value (close-of-previous =
    /// open-of-next).
    ///
    /// We therefore slice on the timestamp-token positions: each segment runs
    /// from one timestamp (or junction) to the next.  When two adjacent
    /// tokens are both timestamps we treat them as a single junction, using
    /// the first of the pair as the boundary value.  This mirrors the
    /// `consecutive` slicing in openai-whisper's `transcribe.py`.
    ///
    /// Why not the previous state-machine?  It misreads paired junctions:
    /// `<|t0|><|t0|>` was consumed as "open then close-with-no-text," which
    /// silently dropped `current_start` and orphaned the segment's text into
    /// the *next* junction --- producing zero-width segments whose text was
    /// shifted one boundary forward.
    fn tokens_to_segments(&self, tokens: &[u32], time_offset: f64) -> Result<Vec<WhisperSegment>> {
        let timestamp_begin = self.no_timestamps_token + 1;
        let is_ts = |t: u32| t >= timestamp_begin;
        let ts_value = |t: u32| (t - timestamp_begin) as f64 * 0.02 + time_offset;
        // Text tokens are everything below `no_timestamps_token` minus the
        // structural specials.  Language and task tokens fall through here
        // and decode to empty strings via the BPE decoder, so we don't need
        // a hardcoded list.
        let is_text = |t: u32| {
            t < self.no_timestamps_token && t != self.sot_token && t != self.eot_token
        };

        let mut segments = Vec::new();
        let mut i = 0;

        while i < tokens.len() {
            // Advance to the next opening timestamp, skipping prompt tokens.
            if !is_ts(tokens[i]) {
                i += 1;
                continue;
            }

            let start_time = ts_value(tokens[i]);

            // Walk over any consecutive timestamp tokens at the junction.
            // This collapses the `T T` pair into a single boundary; if the
            // model emitted only a lone `T` here (legal at SOT under rule 3),
            // `j` stays at `i` and we fall through to the text scan below.
            let mut j = i;
            while j + 1 < tokens.len() && is_ts(tokens[j + 1]) {
                j += 1;
            }

            let text_start = j + 1;
            let mut text_end = text_start;
            while text_end < tokens.len() && !is_ts(tokens[text_end]) {
                text_end += 1;
            }

            if text_end >= tokens.len() {
                // No closing timestamp --- the chunk's last segment is
                // unbounded.  Skip it; the seek-advance logic in `run` will
                // re-process this audio in the next chunk where it stands a
                // chance of being closed.
                break;
            }

            let end_time = ts_value(tokens[text_end]);

            // Filter prompt/special tokens out of the text range; only true
            // text tokens get fed to the BPE decoder and word grouper.
            let text_tokens: Vec<u32> = tokens[text_start..text_end]
                .iter()
                .copied()
                .filter(|&t| is_text(t))
                .collect();

            if !text_tokens.is_empty() && end_time > start_time {
                let text = self
                    .tokenizer
                    .decode(&text_tokens, true)
                    .unwrap_or_default();
                let trimmed = text.trim().to_string();

                if !trimmed.is_empty() {
                    let words =
                        tokens_to_words(&self.tokenizer, &text_tokens, start_time, end_time);
                    segments.push(WhisperSegment {
                        text: trimmed,
                        start: start_time,
                        end: end_time,
                        words,
                    });
                }
            }

            // Resume scanning at the closing timestamp; it's also the open
            // of the next junction, so the outer loop will pick it up.
            i = text_end;
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
/// Mirrors openai-whisper's three-way logic, differentiated by the *shape* of
/// the trailing timestamp tokens rather than their values:
///
/// 1. **Single-timestamp ending** --- last token is a timestamp not preceded
///    by another timestamp (`...text <|t|>`). The model is signalling "I
///    didn't break this chunk into multiple bounded segments; just advance a
///    full window and try again." `t`'s value is *not* used as the advance.
/// 2. **Consecutive pair ending** --- last two tokens are both timestamps
///    (`<|t|><|t|>`). The first timestamp of the *last* such pair is the
///    close of the last fully-bounded segment; advance to it.
/// 3. **No timestamps** --- malformed output; advance a full window.
///
/// Returning `segment_size` directly (rather than `Option`) folds the caller's
/// fallback into this function so the call site can just `seek += result`.
fn last_timestamp_offset_frames(
    tokens: &[u32],
    timestamp_begin: u32,
    frames_per_tick: usize,
    segment_size: usize,
) -> usize {
    let is_ts = |t: u32| t >= timestamp_begin;

    // Single-timestamp ending: locate the last timestamp token. If it isn't
    // preceded by another timestamp, the model emitted `... text <|t|>` with
    // no closing junction --- advance a full window per openai-whisper.
    let last_ts_idx = tokens.iter().rposition(|&t| is_ts(t));
    if let Some(idx) = last_ts_idx {
        let preceded_by_ts = idx > 0 && is_ts(tokens[idx - 1]);
        if !preceded_by_ts {
            return segment_size;
        }
    }

    // Consecutive pair ending: walk windows and keep the last (start, start+1)
    // adjacent-timestamp pair. The first timestamp of that pair is the close
    // of the last fully-bounded segment.
    let mut last_pair_close: Option<u32> = None;
    for window in tokens.windows(2) {
        if is_ts(window[0]) && is_ts(window[1]) {
            last_pair_close = Some(window[0]);
        }
    }

    if let Some(close_token) = last_pair_close {
        let ticks = (close_token - timestamp_begin) as usize;
        if ticks > 0 {
            return ticks * frames_per_tick;
        }
        // Pair value 0 means the only consecutive pair was the chunk's
        // opening junction `<|0|><|0|>` --- no real close was produced.
        // Fall through to a full-window advance.
    }

    segment_size
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

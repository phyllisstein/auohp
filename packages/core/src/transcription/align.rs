//! CTC forced alignment via wav2vec2 (ONNX Runtime).
//!
//! Given a *known* transcript and audio, this module uses wav2vec2 to produce
//! per-frame character probabilities, then runs the Viterbi algorithm over
//! CTC's expanded label sequence to find the optimal alignment. The result is
//! precise per-word timestamps at ≈20 ms resolution.
//!
//! ## What this is for, and what it is deliberately not wired into
//!
//! This module previously did double duty as the pipeline's *only* source of
//! word-level timing --- it refined Whisper's coarse (~1 s) segment
//! timestamps into per-word timestamps by force-aligning Whisper's own
//! transcript back onto the audio. It was removed in commit `b00a30c`
//! ("Replace wav2vec alignment with whisper.cpp VAD") in favor of enabling
//! `set_token_timestamps` on whisper.cpp directly: DTW over whisper's own
//! cross-attention heads gives per-token, then per-word, timestamps for free
//! during decode (see the module doc on [`super::whisper`]). That replacement
//! is still in place and still does that job well --- restoring wav2vec2 to
//! compete with it for the *same* text would be exactly the "one blunt
//! abstraction" this project's inference-tooling convention argues against.
//!
//! What CTC forced alignment is *for*, that DTW fundamentally cannot do, is
//! aligning text DTW never saw: an externally supplied or human-corrected
//! transcript. DTW's timing comes from Whisper's own decode pass, so it can
//! only ever time-stamp the words Whisper itself produced. If a caption
//! editor corrects a mis-transcribed word, or if AUOHP ever needs to
//! time-align an archival paper transcript that predates the audio pipeline
//! entirely, wav2vec2 forced alignment gives exact per-word timing for
//! *that* text without re-running (expensive, autoregressive) ASR at all ---
//! an encoder-single-shot job, squarely `ort`'s house lane. This module is
//! restored as that standalone capability: [`Aligner::align`] takes
//! caller-supplied text, not Whisper's. It is not called anywhere in
//! [`super::pipeline::run_with`] by default; the intended call site is a
//! future editor-driven realignment path (see the caption editor's write-path
//! notes) or ad hoc QC, such as `examples/align_reference.rs` in this crate,
//! which force-aligns this project's own human-transcribed validation
//! reference against its audio as a way to sanity-check the model on real
//! interview speech.
//!
//! ## Model
//!
//! `onnx-community/wav2vec2-base-960h-ONNX` (quantized, ≈95 MB). Downloaded
//! once by `scripts/download-models.sh` into `$MODELS_DIR`, matching this
//! project's "no network I/O at inference" convention --- the old version of
//! this module fetched the model from HuggingFace Hub lazily on first call,
//! which this restoration deliberately does not repeat. The model runs
//! through ONNX Runtime via the `ort` crate---the same runtime already used
//! for diarization and embeddings, so there's no new native dependency.

use std::path::Path;

use anyhow::{Context, Result};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;

use super::types::Word;

/// Sample rate expected by wav2vec2 (same as Whisper).
const SAMPLE_RATE: f64 = 16_000.0;

/// The product of wav2vec2's 7 convolutional strides (5×2×2×2×2×2×2 = 320).
/// Each output frame covers this many input samples, giving 50 frames/second
/// = 20 ms per frame.
const SAMPLES_PER_FRAME: usize = 320;

/// CTC blank token index in wav2vec2-base-960h's vocabulary.
const BLANK: usize = 0;

/// wav2vec2-base-960h vocabulary: 32 tokens.
///
/// Index 0 is the CTC blank (also labeled `<pad>`). Indices 1-3 are special
/// tokens (`<s>`, `</s>`, `<unk>`). Index 4 is the word separator `|`.
/// Indices 5-31 are uppercase English letters + apostrophe, in frequency order.
const VOCAB: &[u8] = b"\0\x01\x02\x03|ETAONIHSRDLUMWCFGYPBVK'XJQZ";

/// Loaded wav2vec2 model ready for forced alignment.
pub struct Aligner {
    session: Session,
    /// Maps ASCII byte -> vocab index. Only populated for characters that
    /// appear in the vocabulary (uppercase A-Z, apostrophe, pipe).
    char_to_idx: [Option<u8>; 128],
}

impl Aligner {
    /// Load the wav2vec2 ONNX model from `model_path` (pre-downloaded by
    /// `scripts/download-models.sh`).
    pub fn load(model_path: &Path) -> Result<Self> {
        tracing::debug!("Aligner: loading wav2vec2 from {}", model_path.display());

        // See `segmentation::Segmenter::new` for why these are `.map_err`
        // rather than `?`: `ort::Error<SessionBuilder>` isn't `Send + Sync`.
        let session = Session::builder()
            .map_err(|e| anyhow::anyhow!("failed to create session builder: {e}"))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow::anyhow!("failed to set optimization level: {e}"))?
            .commit_from_file(model_path)
            .with_context(|| format!("failed to load {}", model_path.display()))?;

        let mut char_to_idx = [None; 128];
        for (idx, &byte) in VOCAB.iter().enumerate() {
            if (byte as usize) < 128 {
                char_to_idx[byte as usize] = Some(idx as u8);
            }
        }

        tracing::debug!("Aligner: wav2vec2 loaded");
        Ok(Self {
            session,
            char_to_idx,
        })
    }

    /// Force-align known `text` onto `audio` (16 kHz mono f32 samples,
    /// covering exactly the span `text` was spoken over), returning per-word
    /// timestamps offset by `time_offset` seconds.
    ///
    /// Returns an empty `Vec` if the audio is too short or the text has no
    /// alignable characters --- callers decide what "no alignment" means for
    /// their use case rather than this module guessing at a fallback.
    pub fn align(&mut self, audio: &[f32], text: &str, time_offset: f64) -> Result<Vec<Word>> {
        if audio.len() < SAMPLES_PER_FRAME || text.is_empty() {
            return Ok(Vec::new());
        }

        // Split text into words BEFORE uppercasing so we preserve the original
        // casing in the output Word structs.
        let word_strs: Vec<&str> = text.split_whitespace().collect();
        if word_strs.is_empty() {
            return Ok(Vec::new());
        }

        // Build the character index sequence: words joined by the | separator.
        let upper = text.to_uppercase();
        let joined: String = upper.split_whitespace().collect::<Vec<_>>().join("|");
        let char_indices: Vec<usize> = joined
            .bytes()
            .filter_map(|b| {
                if (b as usize) < 128 {
                    self.char_to_idx[b as usize].map(|i| i as usize)
                } else {
                    None
                }
            })
            .collect();

        if char_indices.is_empty() {
            return Ok(Vec::new());
        }

        let normalized = normalize(audio);
        let (n_frames, n_vocab, logits) = self.forward(&normalized)?;
        if n_frames < 2 {
            return Ok(Vec::new());
        }

        let char_frames = ctc_forced_align(&logits, n_frames, n_vocab, &char_indices);

        let secs_per_frame = (audio.len() as f64 / SAMPLE_RATE) / n_frames as f64;
        let mut words = Vec::with_capacity(word_strs.len());
        let mut char_pos: usize = 0;

        for &word_text in &word_strs {
            let word_len = word_text
                .to_uppercase()
                .bytes()
                .filter(|&b| (b as usize) < 128 && self.char_to_idx[b as usize].is_some())
                .count();

            if word_len == 0 || char_pos + word_len > char_frames.len() {
                char_pos += word_len + 1; // +1 for separator
                continue;
            }

            let start_frame = char_frames[char_pos];
            let end_frame = if char_pos + word_len < char_frames.len() {
                char_frames[char_pos + word_len] // the separator frame
            } else {
                char_frames[char_pos + word_len - 1] + 1
            };

            words.push(Word {
                word: word_text.to_string(),
                start: time_offset + start_frame as f64 * secs_per_frame,
                end: time_offset + end_frame as f64 * secs_per_frame,
                p: 1.0,
            });

            char_pos += word_len + 1; // +1 for separator
        }

        Ok(words)
    }

    /// Run wav2vec2 inference on normalised 16 kHz audio.
    ///
    /// Returns `(n_frames, n_vocab, logits)` where `logits` is a flat
    /// row-major `Vec<f32>` of shape `(n_frames, n_vocab)`. Values are
    /// log-softmax probabilities.
    fn forward(&mut self, samples: &[f32]) -> Result<(usize, usize, Vec<f32>)> {
        let input =
            ort::value::Tensor::from_array(([1i64, samples.len() as i64], samples.to_vec()))?;

        let outputs = self.session.run(ort::inputs!["input_values" => input])?;

        let output = &outputs["logits"];
        let (shape, data) = output.try_extract_tensor::<f32>()?;

        // Shape: [1, T, V] where T = frames, V = vocab size (32).
        let n_frames = shape[1] as usize;
        let n_vocab = shape[2] as usize;

        // Convert raw logits to log-probabilities (log-softmax along vocab axis).
        let mut log_probs = Vec::with_capacity(n_frames * n_vocab);
        for t in 0..n_frames {
            let row_start = t * n_vocab;
            let row = &data[row_start..row_start + n_vocab];

            let max = row.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let log_sum_exp: f32 = row.iter().map(|&x| (x - max).exp()).sum::<f32>().ln() + max;
            for &x in row {
                log_probs.push(x - log_sum_exp);
            }
        }

        Ok((n_frames, n_vocab, log_probs))
    }
}

// ── Audio normalisation ─────────────────────────────────────────────────────

/// Per-utterance zero-mean, unit-variance normalisation.
///
/// This matches wav2vec2's `Wav2Vec2FeatureExtractor` with
/// `do_normalize: true`. Each audio segment is normalised independently.
fn normalize(samples: &[f32]) -> Vec<f32> {
    let n = samples.len() as f64;
    let mean = samples.iter().map(|&s| s as f64).sum::<f64>() / n;
    let variance = samples
        .iter()
        .map(|&s| ((s as f64) - mean).powi(2))
        .sum::<f64>()
        / n;
    let std = variance.sqrt().max(1e-7); // avoid division by zero

    samples
        .iter()
        .map(|&s| ((s as f64 - mean) / std) as f32)
        .collect()
}

// ── CTC forced alignment (Viterbi) ─────────────────────────────────────────

/// Run CTC forced alignment between frame-level log-probabilities and a
/// character index sequence.
///
/// Returns a `Vec<usize>` of length `char_indices.len()`, where each element
/// is the frame index at which that character was first emitted.
///
/// ## Algorithm
///
/// CTC alignment operates on an *expanded* label sequence that interleaves
/// blank tokens between every character: `[b, c₁, b, c₂, b, …, cₙ, b]`.
/// This sequence has length `2N + 1` where `N` is the number of characters.
///
/// The Viterbi DP walks forward through frames and expanded positions:
///
/// ```text
/// α(t, s) = log_prob(expanded[s], frame t)
///         + max(α(t-1, s),           // stay (repeat label)
///               α(t-1, s-1),         // advance one step
///               α(t-1, s-2))         // skip blank (only if allowed)
/// ```
///
/// The skip transition is allowed when the current label is not blank AND
/// differs from the label two positions back (to handle repeated characters
/// like "LL" which must have a blank between them).
///
/// Backtracking recovers the path, and we extract the first frame where each
/// non-blank character appears.
fn ctc_forced_align(
    log_probs: &[f32],
    n_frames: usize,
    n_vocab: usize,
    char_indices: &[usize],
) -> Vec<usize> {
    let n_chars = char_indices.len();
    let expanded_len = 2 * n_chars + 1;

    let mut expanded = vec![BLANK; expanded_len];
    for (i, &c) in char_indices.iter().enumerate() {
        expanded[2 * i + 1] = c;
    }

    let neg_inf = f32::NEG_INFINITY;
    let mut prev = vec![neg_inf; expanded_len];
    let mut curr = vec![neg_inf; expanded_len];

    // bp[t * expanded_len + s] = the expanded position at frame t-1 that led
    // to (t, s).
    let mut bp = vec![0usize; n_frames * expanded_len];

    let lp = |t: usize, c: usize| -> f32 { log_probs[t * n_vocab + c] };

    prev[0] = lp(0, BLANK);
    if expanded_len > 1 {
        prev[1] = lp(0, expanded[1]);
    }

    for t in 1..n_frames {
        for s in 0..expanded_len {
            let emit = lp(t, expanded[s]);

            let mut best = prev[s];
            let mut best_s = s;

            if s > 0 && prev[s - 1] > best {
                best = prev[s - 1];
                best_s = s - 1;
            }

            if s > 1 && expanded[s] != BLANK && expanded[s] != expanded[s - 2] && prev[s - 2] > best
            {
                best = prev[s - 2];
                best_s = s - 2;
            }

            curr[s] = best + emit;
            bp[t * expanded_len + s] = best_s;
        }

        std::mem::swap(&mut prev, &mut curr);
        curr.fill(neg_inf);
    }

    let last_t = n_frames - 1;
    let mut s = expanded_len - 1;
    if expanded_len >= 2 && prev[expanded_len - 2] > prev[expanded_len - 1] {
        s = expanded_len - 2;
    }

    let mut path = vec![0usize; n_frames];
    path[last_t] = s;
    for t in (1..n_frames).rev() {
        s = bp[t * expanded_len + s];
        path[t - 1] = s;
    }

    // Odd positions in the expanded sequence are characters; extract the
    // first frame where each one appears.
    let mut char_frames = vec![0usize; n_chars];
    let mut found = vec![false; n_chars];

    for (t, &exp_pos) in path.iter().enumerate() {
        if exp_pos % 2 == 1 {
            let char_idx = exp_pos / 2;
            if char_idx < n_chars && !found[char_idx] {
                char_frames[char_idx] = t;
                found[char_idx] = true;
            }
        }
    }

    char_frames
}

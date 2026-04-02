//! CTC forced alignment via wav2vec2 (ONNX Runtime).
//!
//! Given Whisper's text output and the original audio, this module uses
//! wav2vec2 to produce per-frame character probabilities, then runs the
//! Viterbi algorithm over CTC's expanded label sequence to find the optimal
//! alignment.  The result is precise per-word timestamps at ≈20 ms resolution.
//!
//! ## Why wav2vec2?
//!
//! Whisper's timestamp tokens give segment-level boundaries (20 ms resolution),
//! but within a segment, word timing must be inferred.  Wav2vec2 is a CTC
//! model that produces per-frame character logits from raw audio---the forced
//! alignment then maps the known transcript to those frames, giving us
//! character-level timing that we group into words.  This is the same approach
//! WhisperX uses in its Python pipeline.
//!
//! ## Model
//!
//! We use `onnx-community/wav2vec2-base-960h-ONNX` (quantized, ≈95 MB),
//! downloaded automatically from HuggingFace Hub on first use.  The model
//! runs through ONNX Runtime via the `ort` crate---the same runtime already
//! loaded for pyannote diarization, so there's no new native dependency.

use anyhow::{Context, Result};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;

use super::types::Word;
use super::whisper::WhisperSegment;

const WAV2VEC2_REPO: &str = "onnx-community/wav2vec2-base-960h-ONNX";
const WAV2VEC2_MODEL: &str = "onnx/model_quantized.onnx";

/// Sample rate expected by wav2vec2 (same as Whisper).
const SAMPLE_RATE: f64 = 16_000.0;

/// The product of wav2vec2's 7 convolutional strides (5×2×2×2×2×2×2 = 320).
/// Each output frame covers this many input samples, giving 50 frames/second
/// = 20 ms per frame.
const SAMPLES_PER_FRAME: usize = 320;

/// CTC blank token index in wav2vec2-base-960h's vocabulary.
const BLANK: usize = 0;

/// Word-separator token (`|`) index---represents a space between words.
const SEPARATOR: usize = 4;

/// wav2vec2-base-960h vocabulary: 32 tokens.
///
/// Index 0 is the CTC blank (also labeled `<pad>`).  Indices 1–3 are special
/// tokens (`<s>`, `</s>`, `<unk>`).  Index 4 is the word separator `|`.
/// Indices 5–31 are uppercase English letters + apostrophe, in frequency order.
const VOCAB: &[u8] = b"\0\x01\x02\x03|ETAONIHSRDLUMWCFGYPBVK'XJQZ";

/// Loaded wav2vec2 model ready for forced alignment.
pub struct Aligner {
    session: Session,
    /// Maps ASCII byte → vocab index.  Only populated for characters that
    /// appear in the vocabulary (uppercase A–Z, apostrophe, pipe).
    char_to_idx: [Option<u8>; 128],
}

impl Aligner {
    /// Download (if needed) and load the wav2vec2 ONNX model.
    pub fn load() -> Result<Self> {
        let api = hf_hub::api::sync::Api::new().context("failed to create HF Hub API")?;
        let repo = api.model(WAV2VEC2_REPO.to_string());
        let model_path = repo
            .get(WAV2VEC2_MODEL)
            .context("wav2vec2 ONNX download failed")?;

        eprintln!("Aligner: loading wav2vec2 from {}", model_path.display());

        let session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .commit_from_file(&model_path)?;

        // Build ASCII-byte → vocab-index lookup table.
        let mut char_to_idx = [None; 128];
        for (idx, &byte) in VOCAB.iter().enumerate() {
            if (byte as usize) < 128 {
                char_to_idx[byte as usize] = Some(idx as u8);
            }
        }

        eprintln!("Aligner: wav2vec2 loaded");
        Ok(Self {
            session,
            char_to_idx,
        })
    }

    /// Replace approximate word timestamps in Whisper segments with precise
    /// CTC-aligned timestamps.
    ///
    /// For each segment, extracts the corresponding audio slice, runs wav2vec2,
    /// and aligns the segment's text to the per-frame character logits.  If
    /// alignment fails for a segment (e.g. text contains unsupported characters),
    /// the original approximate words are preserved.
    pub fn refine_segments(
        &mut self,
        segments: &mut [WhisperSegment],
        samples: &[f32],
    ) -> Result<()> {
        for seg in segments.iter_mut() {
            // Extract the audio slice for this segment.
            let start_sample = (seg.start * SAMPLE_RATE) as usize;
            let end_sample = ((seg.end * SAMPLE_RATE) as usize).min(samples.len());
            if start_sample >= end_sample {
                continue;
            }
            let audio_slice = &samples[start_sample..end_sample];

            match self.align_segment(audio_slice, &seg.text, seg.start) {
                Ok(words) if !words.is_empty() => {
                    seg.words = words;
                }
                Ok(_) => {
                    // Empty alignment---keep original approximate words.
                }
                Err(e) => {
                    eprintln!(
                        "Aligner: alignment failed for segment at {:.1}s, keeping approximate words: {e}",
                        seg.start
                    );
                }
            }
        }

        Ok(())
    }

    /// Align a single segment's text to its audio slice, returning per-word
    /// timestamps.
    fn align_segment(&mut self, audio: &[f32], text: &str, time_offset: f64) -> Result<Vec<Word>> {
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

        // Normalise audio (per-utterance zero-mean, unit-variance).
        let normalized = normalize(audio);

        // Run wav2vec2 forward pass → (T, 32) log-probabilities.
        let (n_frames, n_vocab, logits) = self.forward(&normalized)?;

        if n_frames < 2 {
            return Ok(Vec::new());
        }

        // CTC forced alignment: find the optimal frame-to-character mapping.
        let char_frames = ctc_forced_align(&logits, n_frames, n_vocab, &char_indices);

        // Map character frame assignments → word boundaries.
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
            // End frame: start of next separator, or last char frame + 1.
            let end_frame = if char_pos + word_len < char_frames.len() {
                char_frames[char_pos + word_len] // the separator frame
            } else {
                char_frames[char_pos + word_len - 1] + 1
            };

            words.push(Word {
                word: word_text.to_string(),
                start: time_offset + start_frame as f64 * secs_per_frame,
                end: time_offset + end_frame as f64 * secs_per_frame,
            });

            char_pos += word_len + 1; // +1 for separator
        }

        Ok(words)
    }

    /// Run wav2vec2 inference on normalised 16 kHz audio.
    ///
    /// Returns `(n_frames, n_vocab, logits)` where `logits` is a flat
    /// row-major `Vec<f32>` of shape `(n_frames, n_vocab)`.  Values are
    /// log-softmax probabilities.
    fn forward(&mut self, samples: &[f32]) -> Result<(usize, usize, Vec<f32>)> {
        let input =
            ort::value::Tensor::from_array(([1i64, samples.len() as i64], samples.to_vec()))?;

        let outputs = self.session.run(ort::inputs!["input_values" => input])?;

        let output = &outputs["logits"];
        let (shape, data) = output.try_extract_tensor::<f32>()?;

        // Shape: [1, T, V] where T = frames, V = vocab size (32).
        // Shape derefs to &[i64], so we can index directly.
        let n_frames = shape[1] as usize;
        let n_vocab = shape[2] as usize;

        // Convert raw logits to log-probabilities (log-softmax along vocab axis).
        let mut log_probs = Vec::with_capacity(n_frames * n_vocab);
        for t in 0..n_frames {
            let row_start = t * n_vocab;
            let row = &data[row_start..row_start + n_vocab];

            // log-softmax: log(exp(x_i) / sum(exp(x_j))) = x_i - log(sum(exp(x_j)))
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
/// `do_normalize: true`.  Each audio segment is normalised independently.
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
    // Expanded sequence: [blank, c0, blank, c1, ..., cN-1, blank]
    let expanded_len = 2 * n_chars + 1;

    // Build the expanded label sequence.
    let mut expanded = vec![BLANK; expanded_len];
    for (i, &c) in char_indices.iter().enumerate() {
        expanded[2 * i + 1] = c;
    }

    // DP tables: alpha[s] = best log-probability at current frame for
    // expanded position s.  We only need two columns (current and previous).
    let neg_inf = f32::NEG_INFINITY;
    let mut prev = vec![neg_inf; expanded_len];
    let mut curr = vec![neg_inf; expanded_len];

    // Backpointer table: bp[t * expanded_len + s] = the expanded position
    // at frame t-1 that led to (t, s).
    let mut bp = vec![0usize; n_frames * expanded_len];

    // Helper: get log-probability of label `c` at frame `t`.
    let lp = |t: usize, c: usize| -> f32 { log_probs[t * n_vocab + c] };

    // Initialise frame 0.
    prev[0] = lp(0, BLANK);
    if expanded_len > 1 {
        prev[1] = lp(0, expanded[1]);
    }

    // Forward pass.
    for t in 1..n_frames {
        for s in 0..expanded_len {
            let emit = lp(t, expanded[s]);

            // Candidate 1: stay at same position.
            let mut best = prev[s];
            let mut best_s = s;

            // Candidate 2: advance from s-1.
            if s > 0 && prev[s - 1] > best {
                best = prev[s - 1];
                best_s = s - 1;
            }

            // Candidate 3: skip from s-2 (only when the current label is
            // not blank and differs from the label at s-2, to handle
            // CTC's repeat-character rule).
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

    // Find the best ending position (last blank or last character).
    let last_t = n_frames - 1;
    let mut s = expanded_len - 1;
    if expanded_len >= 2 && prev[expanded_len - 2] > prev[expanded_len - 1] {
        s = expanded_len - 2;
    }

    // Backtrack to recover the full path.
    let mut path = vec![0usize; n_frames];
    path[last_t] = s;
    for t in (1..n_frames).rev() {
        s = bp[t * expanded_len + s];
        path[t - 1] = s;
    }

    // Extract the first frame where each character (non-blank) appears.
    // Expanded position 2*i+1 corresponds to char_indices[i].
    let mut char_frames = vec![0usize; n_chars];
    let mut found = vec![false; n_chars];

    for (t, &exp_pos) in path.iter().enumerate() {
        // Odd positions in the expanded sequence are characters.
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

//! Whisper ASR via whisper-rs (whisper.cpp FFI).
//!
//! whisper.cpp handles the full decode loop---mel spectrogram, encoder,
//! autoregressive decoder, and timestamp extraction---in highly optimised C++.
//! This module is a thin Rust wrapper that:
//!   1. Loads the ggml Whisper model and silero VAD model from caller-supplied paths.
//!   2. Feeds 16 kHz mono f32 PCM to the decoder.
//!   3. Harvests timestamped segments and per-token DTW timing, then groups
//!      tokens into words using the BPE space-prefix convention.
//!
//! ## Model management
//!
//! Both ggml files are downloaded once by `scripts/download-models.sh` into
//! `$MODELS_DIR` (default `/opt/auohp/models`).  The pipeline resolves paths
//! and passes them here---no network I/O at inference time.
//!
//! ## Voice Activity Detection
//!
//! whisper.cpp's built-in silero VAD pre-segments the audio before handing it
//! to Whisper.  For the Q&A interview pattern (short question / long answer)
//! this prevents Whisper from stitching unrelated speech into a single segment.
//! `WhisperVadParams::set_max_speech_duration` caps long answers at a hard
//! ceiling by forcing a segment break at the nearest silence boundary.
//!
//! ## Word-level timestamps
//!
//! `set_token_timestamps(true)` enables DTW: whisper.cpp runs Dynamic Time
//! Warping over its cross-attention heads to pin each BPE token to a
//! centisecond-resolution `(t0, t1)` frame range.  This module converts those
//! centiseconds to seconds and groups consecutive tokens into words.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use whisper_rs::{
    DtwMode, DtwModelPreset, DtwParameters, FullParams, SamplingStrategy, WhisperContext,
    WhisperContextParameters, WhisperVadParams,
};

use super::types::Word;

// ── Public types ─────────────────────────────────────────────────────────────

/// A transcription segment returned by Whisper.
///
/// Times are in seconds (f64).  `words` holds per-word timing from DTW
/// token timestamps; the wav2vec2 aligner will later replace these with
/// CTC-aligned timestamps for finer precision.
pub struct WhisperSegment {
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub words: Vec<Word>,
}

/// Loaded Whisper model, ready for repeated inference calls.
pub struct WhisperModel {
    ctx: WhisperContext,
    /// Path to the silero VAD ggml model, stored here so `transcribe` can
    /// reference it without an extra parameter on every call.
    vad_model_path: PathBuf,
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Load the Whisper ggml model from `model_path`.
///
/// Both files must already exist---use `scripts/download-models.sh` to
/// fetch them.  The pipeline resolves paths from `$MODELS_DIR` before
/// calling this function.  `vad_model_path` is stored in the returned
/// `WhisperModel` and referenced on every `transcribe` call.
pub fn load_model(model_path: &Path, vad_model_path: &Path) -> Result<WhisperModel> {
    eprintln!("Whisper: loading model from {}", model_path.display());

    // WhisperContextParameters::default() already sets use_gpu based on whether
    // the `metal` / `cuda` feature compiled in, so this call is belt-and-
    // suspenders --- but it's explicit and costs nothing.
    let mut ctx_params = WhisperContextParameters::default();
    ctx_params.use_gpu(true);

    ctx_params.dtw_parameters(DtwParameters {
        mode: DtwMode::ModelPreset {
            model_preset: DtwModelPreset::LargeV3,
        },
        dtw_mem_size: 1024 * 1024 * 128,
    });

    // new_with_params accepts any P: AsRef<Path>.
    let ctx = WhisperContext::new_with_params(model_path, ctx_params)
        .context("failed to load Whisper model")?;

    eprintln!("Whisper: model loaded");
    Ok(WhisperModel {
        ctx,
        vad_model_path: vad_model_path.to_path_buf(),
    })
}

/// Run Whisper inference on 16 kHz mono f32 PCM and return timestamped
/// segments with word-level timing.
pub fn transcribe(model: &mut WhisperModel, samples: &[f32]) -> Result<Vec<WhisperSegment>> {
    let mut state = model
        .ctx
        .create_state()
        .context("failed to create Whisper state")?;

    // to_str() borrows from model.vad_model_path, which lives for the
    // duration of this call.  We bind it before constructing `params` so the
    // borrow lifetime unambiguously covers `params`'s use.
    let vad_path_str = model
        .vad_model_path
        .to_str()
        .context("VAD model path is not valid UTF-8")?;

    let mut params = FullParams::new(SamplingStrategy::BeamSearch {
        beam_size: 5,
        patience: 1.0,
    });
    params.set_language(Some("en"));
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_entropy_thold(3.0);
    params.set_no_context(true);
    // DTW token timestamps: whisper.cpp pins each BPE token to a (t0, t1)
    // frame range via Dynamic Time Warping on the cross-attention heads.
    // Costlier than pure greedy decode but required for word-level timing.
    params.set_token_timestamps(true);

    // VAD: silero pre-segments the audio before handing it to Whisper,
    // preventing unrelated speech from merging into one segment.
    // set_vad_model_path MUST be called before enable_vad --- the FFI wrapper
    // checks for a null pointer and panics otherwise.
    params.set_vad_model_path(Some(vad_path_str));
    params.enable_vad(true);
    // Cap speech segments at 60 s.  whisper.cpp will split at the nearest
    // silence of ≥98 ms, so long answers get broken at natural pause points
    // rather t han at arbitrary frame boundaries.
    let mut vad_params = WhisperVadParams::new();
    vad_params.set_max_speech_duration(60.0);
    params.set_vad_params(vad_params);

    eprintln!("Whisper: running inference on {} samples", samples.len());
    state
        .full(params, samples)
        .context("Whisper inference failed")?;

    // full_n_segments returns a bare c_int --- no Result, no ? needed.
    let n_segs = state.full_n_segments();
    eprintln!("Whisper: {} segments", n_segs);

    let mut segments = Vec::with_capacity(n_segs as usize);
    for i in 0..n_segs {
        // get_segment returns Option<WhisperSegment<'_>>, borrowing from `state`.
        // Since i < n_segs, this is always Some --- the .context() turns the
        // Option into a Result for the ? operator.
        let seg = state
            .get_segment(i)
            .context("segment index out of bounds")?;

        let text = seg
            .to_str()
            .context("failed to read segment text")?
            .trim()
            .to_string();

        // Timestamps from whisper.cpp are in centiseconds (1/100 s); round to
        // that same 2-decimal grid so serialised times carry no float noise.
        let start = round_to(seg.start_timestamp() as f64 / 100.0, 2);
        let end = round_to(seg.end_timestamp() as f64 / 100.0, 2);

        let words = collect_words(&seg, start)?;
        segments.push(WhisperSegment {
            text,
            start,
            end,
            words,
        });
    }

    Ok(segments)
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Round `x` to `places` decimal places.
///
/// We *round* (nearest) rather than truncate (toward zero): truncation would
/// bias every value downward --- confidences always a hair low, start times
/// always a hair early --- and that bias compounds across thousands of words.
/// Whisper's timing grid is centiseconds and `p` is a coarse editorial signal,
/// so two decimals is the real information content; more is invented precision.
fn round_to(x: f64, places: i32) -> f64 {
    let factor = 10f64.powi(places);
    (x * factor).round() / factor
}

/// Group per-token DTW timing from a single segment into words.
///
/// whisper.cpp uses the BPE space-prefix convention: a token whose decoded
/// text begins with an ASCII space marks the start of a new word.  Special
/// control tokens (`<|startoftranscript|>`, `<|en|>`, `<|0.00|>`, etc.) are
/// enclosed in `<|…|>` and are skipped.
///
/// `token.token_data()` returns `WhisperTokenData` (a direct C struct alias),
/// whose `t0` / `t1` fields are in centiseconds.  This function converts them
/// to seconds before storing.
///
/// The parameter type uses `whisper_rs::WhisperSegment<'_>` (fully qualified)
/// to avoid a name collision with our own public `WhisperSegment` struct.
fn collect_words(seg: &whisper_rs::WhisperSegment<'_>, seg_start: f64) -> Result<Vec<Word>> {
    // n_tokens is a bare c_int --- no Result.
    let n_tokens = seg.n_tokens();

    let mut words: Vec<Word> = Vec::new();
    let mut current_word = String::new();
    let mut word_start = seg_start;
    let mut word_end = seg_start;
    let mut confidence: f32 = 0.0;

    for j in 0..n_tokens {
        // get_token returns Option<WhisperToken<'_, '_>>; bounds are guaranteed here.
        let token = seg.get_token(j).context("token index out of bounds")?;

        // token_data() returns WhisperTokenData (whisper_rs_sys::whisper_token_data)
        // directly --- not a Result.  The t0/t1 fields are i64 centiseconds.
        let token_data = token.token_data();
        let token_text = token.to_str().context("failed to read token text")?;

        // `<|…|>` tokens are whisper.cpp's control tokens (language tag, task
        // tag, timestamp markers, EOT, etc.) --- they carry no word content.
        if token_text.starts_with("<|") {
            continue;
        }

        // Clamp negative timestamps that whisper.cpp can emit when DTW
        // alignment is uncertain, convert centiseconds → seconds, and round to
        // whisper's native centisecond grid (more precision is fiction).
        let t0 = round_to(token_data.t0.max(0) as f64 / 100.0, 2);
        let t1 = round_to(token_data.t1.max(0) as f64 / 100.0, 2);
        let p = token_data.p;

        if token_text.starts_with(' ') {
            // Leading space = new word. Flush the word we just finished with its
            // accumulated min-confidence, then open the next word with this
            // token as its first (and so far only) member.
            if !current_word.is_empty() {
                words.push(Word {
                    word: current_word.clone(),
                    start: word_start,
                    end: word_end,
                    p: round_to(confidence as f64, 2) as f32,
                });
            }
            current_word = token_text.trim_start_matches(' ').to_string();
            word_start = t0;
            confidence = p; // seed the running min with the new word's first token
        } else if current_word.is_empty() {
            // First content token of the segment (no leading space): open the
            // word and seed the min --- do NOT zero it, or min() pins it to 0.
            word_start = t0;
            confidence = p;
            current_word.push_str(token_text);
        } else {
            // Continuation of the current word: fold this sub-token's
            // probability into the running minimum (weakest link wins).
            confidence = confidence.min(p);
            current_word.push_str(token_text);
        }
        word_end = t1;
    }

    // Flush the final word.
    if !current_word.is_empty() {
        words.push(Word {
            word: current_word,
            start: word_start,
            end: word_end,
            p: confidence,
        });
    }

    Ok(words)
}

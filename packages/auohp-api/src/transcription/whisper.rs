//! Whisper ASR with DTW word-level timestamps.
//!
//! Uses whisper-rs (whisper.cpp bindings) with DTW alignment to produce
//! per-word timing. BPE sub-tokens are merged into words using the
//! space-prefix convention.
//!
//! ## Stall detection
//!
//! whisper.cpp's decoder can get trapped in an infinite loop when the audio
//! contains low-energy or repetitive regions: the internal `seek` cursor
//! fails to advance, so the same 30s chunk is re-decoded forever. We detect
//! this via the progress callback (which fires when `seek` advances) and
//! abort via the abort callback when progress stalls. Processing then
//! resumes from past the stuck region so we only lose ~30s of transcript
//! rather than hanging indefinitely.

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use whisper_rs::{
    DtwMode, DtwModelPreset, FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
};

use super::types::Word;

/// How long to wait for the progress percentage to advance before aborting.
/// A 30s audio chunk typically processes in well under a minute; 90s is
/// generous enough for slow hardware but catches infinite decoder loops.
const STALL_TIMEOUT: Duration = Duration::from_secs(60);

/// When the decoder stalls, skip ahead by this many milliseconds past the
/// last completed segment. One chunk width (30s) jumps cleanly past the
/// problematic audio region.
const SKIP_MS: i32 = 30_000;

/// A Whisper segment with word-level DTW timestamps.
pub struct WhisperSegment {
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub words: Vec<Word>,
}

/// Load the Whisper model with DTW alignment enabled.
pub fn load_model(model_path: &Path) -> Result<WhisperContext> {
    let mut ctx_params = WhisperContextParameters::default();
    ctx_params.dtw_parameters.mode = DtwMode::ModelPreset {
        model_preset: DtwModelPreset::LargeV3,
    };

    WhisperContext::new_with_params(
        model_path
            .to_str()
            .context("model path is not valid UTF-8")?,
        ctx_params,
    )
    .context("failed to load Whisper model")
}

/// Run Whisper inference on 16 kHz mono f32 audio and return segments with
/// word-level DTW timestamps.
///
/// If the decoder stalls on a region of audio, processing is aborted and
/// resumed from past the stuck point. A warning is printed to stderr for
/// each skipped region. The returned segments cover all non-skipped audio.
pub fn transcribe(ctx: &WhisperContext, audio: &[f32]) -> Result<Vec<WhisperSegment>> {
    let total_ms = ((audio.len() as f64 / 16_000.0) * 1000.0) as i32;
    let mut all_segments: Vec<WhisperSegment> = Vec::new();
    let mut offset_ms: i32 = 0;

    // Each iteration processes from `offset_ms` to end-of-audio. Normally
    // that means one iteration covers everything. If the decoder stalls,
    // we abort, salvage partial results, and loop again from past the
    // stuck region.
    while offset_ms < total_ms {
        let mut state = ctx
            .create_state()
            .context("failed to create Whisper state")?;

        // --- Shared stall-detection state (lock-free) ---
        //
        // The progress callback fires each time whisper.cpp's seek cursor
        // advances to a new audio chunk. The abort callback is polled by
        // whisper.cpp during decoding; returning `true` kills processing.
        //
        // `clock` is the reference instant. `last_advance_ms` stores the
        // elapsed time (relative to `clock`) when progress last advanced.
        // `last_progress` tracks the percent value to detect duplicates.
        // All three are shared between the two callbacks via Arc + atomics,
        // which is safe and lock-free---no risk of deadlock even though
        // both callbacks run on the same thread.
        let clock = Instant::now();
        let last_progress = Arc::new(AtomicI32::new(-1));
        let last_advance_ms = Arc::new(AtomicI64::new(0));
        let aborted = Arc::new(AtomicBool::new(false));

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 5 });

        // Force English decoding. large-v3 is a multilingual model, so
        // without this hint Whisper spends the first ~30s of each file
        // running language detection. Pinning to "en" skips that probe
        // and prevents accidental code-switches mid-sentence.
        params.set_language(Some("en"));
        params.set_token_timestamps(true);
        params.set_print_progress(false);
        params.set_print_realtime(false);

        // Resume from where we left off after a stall-abort.
        if offset_ms > 0 {
            params.set_offset_ms(offset_ms);
        }

        // --- Anti-looping parameters ---
        //
        // These make individual decoder runs terminate faster so the
        // abort callback gets a chance to fire. They don't prevent the
        // stall on their own (that's the abort callback's job), but they
        // reduce the window between abort-callback polls.

        // Don't condition on previous segment text. If segment N loops,
        // its output would prime N+1 to loop too. Independent segments
        // break this cross-segment feedback chain.
        params.set_no_context(true);

        // Cap tokens per segment: 64 tokens ≈ 40--50 words. Forces the
        // segment to end so the fallback/retry logic can kick in.
        params.set_max_tokens(32);

        // Tighter entropy threshold (default 2.4). Rejects repetitive
        // output earlier, triggering temperature-bump retry.
        params.set_entropy_thold(1.8);

        // Larger temperature increments on retry (default 0.2). Three
        // retries (0.0, 0.4, 0.8) instead of six, so doomed chunks
        // fail faster.
        params.set_temperature_inc(0.5);

        // Suppress non-speech tokens---reduces hallucinated filler that
        // can seed a repetition loop.
        params.set_suppress_nst(true);

        // --- Stall-detection callbacks ---

        // Progress callback: record when progress last advanced. The
        // `pct` parameter is the seek position as a percentage of total
        // audio length (0--100).
        {
            let progress = last_progress.clone();
            let advance = last_advance_ms.clone();
            let t0 = clock;
            params.set_progress_callback_safe(move |pct: i32| {
                let prev = progress.swap(pct, Ordering::Relaxed);
                if pct > prev {
                    advance.store(t0.elapsed().as_millis() as i64, Ordering::Relaxed);
                }
            });
        }

        // Abort callback: fire if progress hasn't advanced for
        // STALL_TIMEOUT. whisper.cpp polls this during decoding and
        // stops the main loop when it returns true.
        {
            let advance = last_advance_ms.clone();
            let stall = aborted.clone();
            let t0 = clock;
            params.set_abort_callback_safe(move || -> bool {
                let last = advance.load(Ordering::Relaxed);
                let now = t0.elapsed().as_millis() as i64;
                let stalled = (now - last) > STALL_TIMEOUT.as_millis() as i64;
                if stalled {
                    stall.store(true, Ordering::Relaxed);
                }
                stalled
            });
        }

        // --- Run inference ---

        let result = state.full(params, audio);
        let did_abort = aborted.load(Ordering::Relaxed);

        // Extract whatever segments completed before the abort (or all
        // of them on normal completion). whisper.cpp stores segments
        // incrementally, so partial results survive an abort.
        all_segments.extend(extract_segments_from_state(&state));

        if did_abort {
            // Skip 30s past the last completed segment to jump over the
            // problematic audio region. If no segments were extracted at
            // all, skip 30s from the current offset.
            let resume_ms = all_segments
                .last()
                .map(|s| (s.end * 1000.0) as i32 + SKIP_MS)
                .unwrap_or(offset_ms + SKIP_MS);

            eprintln!(
                "Whisper: decoder stalled at offset {:.1}s, resuming at {:.1}s",
                offset_ms as f64 / 1000.0,
                resume_ms as f64 / 1000.0,
            );

            offset_ms = resume_ms;
        } else {
            // Not a stall. If `full()` returned an error for some other
            // reason, propagate it. Otherwise processing completed
            // normally.
            result.context("Whisper inference failed")?;
            break;
        }
    }

    Ok(all_segments)
}

/// Pull completed segments (with word-level timestamps) out of a
/// `WhisperState` after a `full()` call. Works regardless of whether
/// `full()` completed normally or was aborted mid-stream.
fn extract_segments_from_state(state: &whisper_rs::WhisperState) -> Vec<WhisperSegment> {
    let n_segments = state.full_n_segments();
    let mut segments = Vec::with_capacity(n_segments as usize);

    for i in 0..n_segments {
        let segment = match state.get_segment(i) {
            Some(seg) => seg,
            None => continue,
        };

        let text = match segment.to_str_lossy() {
            Ok(t) => t.trim().to_string(),
            Err(_) => continue,
        };
        if text.is_empty() {
            continue;
        }

        // Timestamps are in centiseconds, absolute from the start of the
        // audio buffer (not relative to offset_ms).
        let start = segment.start_timestamp() as f64 / 100.0;
        let end = segment.end_timestamp() as f64 / 100.0;

        let words = extract_words(&segment);

        segments.push(WhisperSegment {
            text,
            start,
            end,
            words,
        });
    }

    segments
}

/// Merge BPE sub-tokens into words using the space-prefix convention.
/// Tokens beginning with a space start a new word; all others are continuations.
fn extract_words(segment: &whisper_rs::WhisperSegment<'_>) -> Vec<Word> {
    let mut words: Vec<Word> = Vec::new();
    let mut current_text = String::new();
    let mut current_start: Option<f64> = None;
    let mut current_end: f64 = 0.0;

    for t in 0..segment.n_tokens() {
        let token = match segment.get_token(t) {
            Some(tok) => tok,
            None => continue,
        };

        let token_data = token.token_data();

        // Skip special tokens (e.g. [SOT], [EOT], [BLANK]).
        if token_data.id >= 50_000 {
            continue;
        }

        let token_text = match token.to_str_lossy() {
            Ok(t) => t.to_string(),
            Err(_) => continue,
        };

        let dtw_time = token_data.t_dtw as f64 / 100.0; // centiseconds -> seconds

        if token_text.starts_with(' ') {
            // Flush current word.
            if !current_text.is_empty() {
                words.push(Word {
                    word: current_text.clone(),
                    start_time: current_start.unwrap_or(0.0),
                    end_time: current_end,
                });
            }
            current_text = token_text.trim_start().to_string();
            current_start = Some(dtw_time);
            current_end = dtw_time;
        } else {
            if current_start.is_none() {
                current_start = Some(dtw_time);
            }
            current_text.push_str(&token_text);
            current_end = dtw_time;
        }
    }

    // Flush the last word.
    if !current_text.is_empty() {
        words.push(Word {
            word: current_text,
            start_time: current_start.unwrap_or(0.0),
            end_time: current_end,
        });
    }

    words
}

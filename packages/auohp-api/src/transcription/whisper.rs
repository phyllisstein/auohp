//! Whisper ASR with DTW word-level timestamps.
//!
//! Uses whisper-rs (whisper.cpp bindings) with DTW alignment to produce
//! per-word timing. BPE sub-tokens are merged into words using the
//! space-prefix convention.

use anyhow::{Context, Result};
use std::path::Path;
use whisper_rs::{
    DtwMode, DtwModelPreset, FullParams, SamplingStrategy, WhisperContext,
    WhisperContextParameters,
};

use super::types::{ProgressEvent, ProgressTx, TranscriptionPhase, Word};

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
pub fn transcribe(
    ctx: &WhisperContext,
    audio: &[f32],
    progress: Option<&ProgressTx>,
) -> Result<Vec<WhisperSegment>> {
    let mut state = ctx.create_state().context("failed to create Whisper state")?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 5 });
    params.set_language(Some("en"));
    params.set_token_timestamps(true);
    params.set_print_progress(false);
    params.set_print_realtime(false);

    // Bridge progress callback to our broadcast channel.
    if let Some(tx) = progress {
        let tx = tx.clone();
        params.set_progress_callback_safe(move |pct: i32| {
            let _ = tx.send(ProgressEvent::new(
                TranscriptionPhase::Transcribing,
                pct as f32 / 100.0,
            ));
        });
    }

    state
        .full(params, audio)
        .context("Whisper inference failed")?;

    // Extract segments with DTW word timestamps using the segment iterator.
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

        let start = segment.start_timestamp() as f64 / 100.0; // centiseconds → seconds
        let end = segment.end_timestamp() as f64 / 100.0;

        let words = extract_words(&segment);

        segments.push(WhisperSegment {
            text,
            start,
            end,
            words,
        });
    }

    Ok(segments)
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

        let dtw_time = token_data.t_dtw as f64 / 100.0; // centiseconds → seconds

        if token_text.starts_with(' ') {
            // Flush current word.
            if !current_text.is_empty() {
                words.push(Word {
                    word: current_text.clone(),
                    start: current_start.unwrap_or(0.0),
                    end: current_end,
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
            start: current_start.unwrap_or(0.0),
            end: current_end,
        });
    }

    words
}

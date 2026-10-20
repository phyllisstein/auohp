//! Whisper transcription inference execution loop.

use anyhow::{Context as _, Result};
use whisper_rs::{FullParams, SamplingStrategy};

use super::super::config::TranscribeConfig;
use super::super::types::Word;
use super::alignment::{collect_words, round_to, strip_turn_dash};
use super::model::WhisperModel;
use super::vad::{apply_vad, VadTimeline};

/// A transcription segment returned by Whisper.
///
/// Times are seconds; `words` carries the DTW per-word timing.
#[derive(Debug, Clone)]
pub struct WhisperSegment {
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub words: Vec<Word>,
}

/// Run Whisper inference on 16 kHz mono f32 PCM and return timestamped
/// segments with word-level timing.
pub fn transcribe(
    model: &mut WhisperModel,
    samples: &[f32],
    cfg: &TranscribeConfig,
) -> Result<Vec<WhisperSegment>> {
    // Silero runs here, not inside whisper.cpp---`whisper_full_with_state`
    // never reads `params.vad`.
    let (audio, timeline) = if cfg.vad.enabled {
        apply_vad(samples, model.vad_model_path(), &cfg.vad)?
    } else {
        (samples.to_vec(), VadTimeline::identity())
    };

    // Special tokens occupy a reserved id range starting at `token_eot`, so one
    // comparison classifies all of them.
    let special_min = model.token_eot();

    let mut state = model.create_state()?;

    let d = &cfg.decode;
    let mut params = FullParams::new(SamplingStrategy::BeamSearch {
        beam_size: d.beam_size,
        patience: d.patience,
    });
    params.set_language(d.language.as_deref());
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_entropy_thold(d.entropy_thold);
    params.set_no_context(d.no_context);
    // Word-level timing requires DTW token timestamps.
    params.set_token_timestamps(d.token_timestamps);

    if let Some(v) = d.logprob_thold {
        params.set_logprob_thold(v);
    }
    if let Some(v) = d.no_speech_thold {
        params.set_no_speech_thold(v);
    }
    if let Some(v) = d.temperature {
        params.set_temperature(v);
    }
    if let Some(v) = d.temperature_inc {
        params.set_temperature_inc(v);
    }
    if let Some(v) = d.suppress_nst {
        params.set_suppress_nst(v);
    }
    if let Some(v) = d.max_len {
        params.set_max_len(v);
    }
    if let Some(v) = d.split_on_word {
        params.set_split_on_word(v);
    }
    if let Some(p) = d.initial_prompt.as_deref() {
        params.set_initial_prompt(p);
    }

    tracing::debug!("Whisper: running inference on {} samples", audio.len());
    state
        .full(params, &audio)
        .context("Whisper inference failed")?;

    let n_segs = state.full_n_segments();
    tracing::debug!("Whisper: {} segments", n_segs);

    let mut segments = Vec::with_capacity(n_segs as usize);
    for i in 0..n_segs {
        let seg = state
            .get_segment(i)
            .context("segment index out of bounds")?;

        // Lossy is acceptable per segment: a split mid-character is rare and
        // there is no larger unit to accumulate into. Per token it would corrupt
        // text; see `collect_words`.
        let text = strip_turn_dash(
            seg.to_str_lossy()
                .context("failed to read segment text")?
                .trim(),
        )
        .to_string();

        // whisper.cpp timestamps are centiseconds; round to that grid.
        let start = round_to(seg.start_timestamp() as f64 / 100.0, 2);
        let end = round_to(seg.end_timestamp() as f64 / 100.0, 2);

        // Words are assembled on the filtered timeline, then mapped back.
        let words = collect_words(&seg, start, end, special_min)?
            .into_iter()
            .map(|w| Word {
                start: round_to(timeline.to_original(w.start), 2),
                end: round_to(timeline.to_original(w.end), 2),
                ..w
            })
            .collect();

        segments.push(WhisperSegment {
            text,
            start: round_to(timeline.to_original(start), 2),
            end: round_to(timeline.to_original(end), 2),
            words,
        });
    }

    Ok(segments)
}

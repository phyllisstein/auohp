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
/// Times are in seconds (f64). `words` holds per-word timing from DTW token
/// timestamps, and that is the pipeline's final word timing.
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

    // Whisper's vocabulary is partitioned: ordinary text tokens occupy the low
    // ids and every special token (`[_BEG_]`, `[_EOT_]`, the `[_TT_n]` timestamp
    // block) sits in a reserved range at the top, starting at `token_eot`. So a
    // single numeric comparison classifies them all---no string matching, and
    // no need to enumerate forms whisper.cpp might emit.
    //
    // Read before `create_state` purely for clarity; both borrows are shared.
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
    // DTW token timestamps: whisper.cpp pins each BPE token to an instant via
    // Dynamic Time Warping on the cross-attention heads. Costlier than pure
    // greedy decode but required for word-level timing.
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
    // Seeds the decoder with domain vocabulary---the main lever for proper
    // nouns and terms of art. Borrowed from `cfg`, which outlives `params`.
    if let Some(p) = d.initial_prompt.as_deref() {
        params.set_initial_prompt(p);
    }

    // Note what is *not* here: `set_vad_model_path` / `enable_vad`. Those are a
    // no-op through `whisper_full_with_state`, so setting them would only make
    // the config look honoured when it is not. `apply_vad` above did the work.

    tracing::debug!("Whisper: running inference on {} samples", audio.len());
    state
        .full(params, &audio)
        .context("Whisper inference failed")?;

    // full_n_segments returns a bare c_int---no Result, no ? needed.
    let n_segs = state.full_n_segments();
    tracing::debug!("Whisper: {} segments", n_segs);

    let mut segments = Vec::with_capacity(n_segs as usize);
    for i in 0..n_segs {
        // get_segment returns Option<WhisperSegment<'_>>, borrowing from `state`.
        // Since i < n_segs, this is always Some---the .context() turns the
        // Option into a Result for the ? operator.
        let seg = state
            .get_segment(i)
            .context("segment index out of bounds")?;

        // Lossy is right *here* and wrong at the token level. A segment is a
        // complete run of tokens, so its bytes are valid UTF-8 unless whisper.cpp
        // split the segment mid-character---rare, and when it happens there is no
        // larger unit to accumulate into, so one replacement char is the best
        // available answer. Per *token*, by contrast, fragments are routine and
        // lossy decoding would corrupt ordinary text; see `collect_words`.
        //
        // Not hypothetical: `to_str()` here and in `collect_words` aborted the
        // whole of run `033-full-108` after 60 minutes of completed inference.
        let text = strip_turn_dash(
            seg.to_str_lossy()
                .context("failed to read segment text")?
                .trim(),
        )
        .to_string();

        // Timestamps from whisper.cpp are in centiseconds (1/100 s); round to
        // that same 2-decimal grid so serialised times carry no float noise.
        let start = round_to(seg.start_timestamp() as f64 / 100.0, 2);
        let end = round_to(seg.end_timestamp() as f64 / 100.0, 2);

        // Words are assembled in the filtered timeline, then mapped back---so
        // `collect_words` still sees a self-consistent segment and only the final
        // values cross back into real-recording time.
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

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
//! ## Voice Activity Detection --- why it is applied here rather than by whisper.cpp
//!
//! Setting `FullParams::enable_vad(true)` does nothing on this code path, and
//! that is not a bug we introduced. whisper.cpp reads `params.vad` only in
//! `whisper_full` and `whisper_full_parallel`; `whisper_full_with_state` --- the
//! entry point `WhisperState::full()` calls --- never looks at it. So VAD was
//! silently inert for every run this pipeline ever made, `max_speech_duration`
//! included.
//!
//! Rather than switch to the context-owned `whisper_full` (which would give up
//! the caller-managed state this pipeline is built around), [`apply_vad`] runs
//! silero explicitly and hands Whisper the filtered audio. That reproduces
//! whisper.cpp's own construction (`whisper.cpp:6641-6700`): concatenate the
//! detected speech regions, extend all but the last by `samples_overlap`, and
//! glue them with 0.1 s of silence.
//!
//! The cost of doing it ourselves is that **Whisper's timestamps then refer to
//! the filtered timeline**, which is shorter than the real recording and has the
//! silences removed. whisper.cpp keeps an internal `vad_mapping_table` for this;
//! we keep [`VadTimeline`] and map every segment and word time back before the
//! result leaves this module. Skipping that step would produce a transcript whose
//! timings drift further out of sync the more silence the recording contains ---
//! wrong in a way that looks plausible right up until someone scrubs the video.
//!
//! ## Word-level timestamps
//!
//! `set_token_timestamps(true)` enables DTW: whisper.cpp runs Dynamic Time
//! Warping over its cross-attention heads to pin each BPE token to a single
//! centisecond-resolution instant, published as `whisper_token_data::t_dtw`.
//!
//! Note the shape difference, because it drives the whole word-assembly design.
//! `t0`/`t1` are an *interval* per token, produced by the coarse fallback
//! heuristic (~1 s resolution); `t_dtw` is a *point*. Assembling words from
//! points means a word's end is not carried by its own tokens at all --- it is
//! the start of whatever comes next. So the assembler zips each word against its
//! successor rather than folding intervals, and the final word is the only one
//! that has to reach for the segment's end time.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use whisper_rs::{
    DtwMode, DtwModelPreset, DtwParameters, FullParams, SamplingStrategy, WhisperContext,
    WhisperContextParameters, WhisperVadContext, WhisperVadContextParams, WhisperVadParams,
};

use super::config::{TranscribeConfig, VadConfig};
use super::types::Word;

const SAMPLE_RATE: f64 = 16_000.0;

/// Silence inserted between kept speech regions, matching `whisper.cpp:6670`.
const VAD_GLUE_SECONDS: f64 = 0.1;

// ── Voice activity detection ─────────────────────────────────────────────────

/// One kept speech region, in both timelines.
#[derive(Debug, Clone, Copy, PartialEq)]
struct VadRegion {
    /// Where the region starts in the real recording.
    orig_start: f64,
    /// Where it starts in the filtered audio Whisper actually sees.
    filtered_start: f64,
    duration: f64,
}

/// The map from filtered-audio time back to real-recording time.
///
/// Filtering removes silence, so the two timelines diverge by the total silence
/// dropped before any given instant. Every time Whisper reports has to come back
/// through [`VadTimeline::to_original`] before it means anything.
#[derive(Debug, Clone, Default)]
pub struct VadTimeline {
    regions: Vec<VadRegion>,
}

impl VadTimeline {
    /// Identity map, for when VAD is disabled and the two timelines are the same.
    fn identity() -> Self {
        Self { regions: Vec::new() }
    }

    fn is_identity(&self) -> bool {
        self.regions.is_empty()
    }

    /// Map an instant in the filtered timeline back to the real recording.
    ///
    /// Times landing in the glue silence between two regions are clamped to the
    /// end of the region that precedes them. That is the honest answer: no audio
    /// exists there, so any word Whisper places in the glue belongs to the speech
    /// on one side of it, and the earlier side is the one it was decoded from.
    pub fn to_original(&self, t: f64) -> f64 {
        if self.is_identity() {
            return t;
        }
        // Regions are ordered and non-overlapping in the filtered timeline, so a
        // linear scan with early exit is both simple and fast enough --- a
        // 34-minute interview yields a few hundred regions.
        let mut last_end = self.regions[0].orig_start;
        for r in &self.regions {
            if t < r.filtered_start {
                return last_end; // inside glue silence
            }
            if t <= r.filtered_start + r.duration {
                return r.orig_start + (t - r.filtered_start);
            }
            last_end = r.orig_start + r.duration;
        }
        last_end
    }
}

/// Run silero over `samples` and return the filtered audio plus its timeline map.
///
/// Reproduces `whisper.cpp:6641-6700`: each detected region is kept, all but the
/// last are extended by `samples_overlap`, and the regions are glued with 0.1 s
/// of silence.
pub fn apply_vad(
    samples: &[f32],
    vad_model_path: &Path,
    cfg: &VadConfig,
) -> Result<(Vec<f32>, VadTimeline)> {
    let path = vad_model_path
        .to_str()
        .context("VAD model path is not valid UTF-8")?;

    let mut vctx = WhisperVadContext::new(path, WhisperVadContextParams::new())
        .map_err(|e| anyhow::anyhow!("failed to load VAD model: {e}"))?;

    let mut params = WhisperVadParams::new();
    if let Some(x) = cfg.threshold {
        params.set_threshold(x);
    }
    if let Some(x) = cfg.min_speech_duration_ms {
        params.set_min_speech_duration(x);
    }
    if let Some(x) = cfg.min_silence_duration_ms {
        params.set_min_silence_duration(x);
    }
    if let Some(x) = cfg.max_speech_duration_s {
        params.set_max_speech_duration(x);
    }
    if let Some(x) = cfg.speech_pad_ms {
        params.set_speech_pad(x);
    }
    if let Some(x) = cfg.samples_overlap_s {
        params.set_samples_overlap(x);
    }

    let segments = vctx
        .segments_from_samples(params, samples)
        .map_err(|e| anyhow::anyhow!("VAD failed: {e}"))?;

    let n = segments.num_segments();
    if n == 0 {
        // Silero found no speech at all. Returning the original audio unfiltered
        // is the safe failure: a transcript of everything beats a transcript of
        // nothing, and the caller can see `segments == 0` in the log.
        eprintln!("Whisper: VAD found no speech; passing audio through unfiltered");
        return Ok((samples.to_vec(), VadTimeline::identity()));
    }

    let overlap = cfg.samples_overlap_s.unwrap_or(0.1) as f64;
    let total = samples.len() as f64 / SAMPLE_RATE;

    let mut filtered: Vec<f32> = Vec::with_capacity(samples.len());
    let mut regions = Vec::with_capacity(n as usize);

    for i in 0..n {
        let (Some(start_cs), Some(end_cs)) = (
            segments.get_segment_start_timestamp(i),
            segments.get_segment_end_timestamp(i),
        ) else {
            continue;
        };
        // whisper-rs reports centiseconds.
        let orig_start = (start_cs as f64 / 100.0).clamp(0.0, total);
        let mut orig_end = end_cs as f64 / 100.0;
        if i < n - 1 {
            orig_end += overlap;
        }
        let orig_end = orig_end.clamp(orig_start, total);

        let (s, e) = (
            (orig_start * SAMPLE_RATE) as usize,
            (orig_end * SAMPLE_RATE) as usize,
        );
        let (s, e) = (s.min(samples.len()), e.min(samples.len()));
        if e <= s {
            continue;
        }

        if !regions.is_empty() {
            filtered.extend(std::iter::repeat_n(
                0.0,
                (VAD_GLUE_SECONDS * SAMPLE_RATE) as usize,
            ));
        }
        regions.push(VadRegion {
            orig_start,
            filtered_start: filtered.len() as f64 / SAMPLE_RATE,
            duration: (e - s) as f64 / SAMPLE_RATE,
        });
        filtered.extend_from_slice(&samples[s..e]);
    }

    if regions.is_empty() {
        return Ok((samples.to_vec(), VadTimeline::identity()));
    }

    let kept = filtered.len() as f64 / SAMPLE_RATE;
    eprintln!(
        "Whisper: VAD kept {} speech regions, {:.1}s of {:.1}s ({:.0}% dropped)",
        regions.len(),
        kept,
        total,
        100.0 * (1.0 - kept / total)
    );

    Ok((filtered, VadTimeline { regions }))
}

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
pub fn transcribe(
    model: &mut WhisperModel,
    samples: &[f32],
    cfg: &TranscribeConfig,
) -> Result<Vec<WhisperSegment>> {
    // Silero runs here, not inside whisper.cpp --- `whisper_full_with_state`
    // never reads `params.vad`. See the module docs.
    let (audio, timeline) = if cfg.vad.enabled {
        apply_vad(samples, &model.vad_model_path, &cfg.vad)?
    } else {
        (samples.to_vec(), VadTimeline::identity())
    };

    // Whisper's vocabulary is partitioned: ordinary text tokens occupy the low
    // ids and every special token (`[_BEG_]`, `[_EOT_]`, the `[_TT_n]` timestamp
    // block) sits in a reserved range at the top, starting at `token_eot`. So a
    // single numeric comparison classifies them all --- no string matching, and
    // no need to enumerate forms whisper.cpp might emit.
    //
    // Read before `create_state` purely for clarity; both borrows are shared.
    let special_min = model.ctx.token_eot();

    let mut state = model
        .ctx
        .create_state()
        .context("failed to create Whisper state")?;

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
    // Dynamic Time Warping on the cross-attention heads.  Costlier than pure
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
    // Seeds the decoder with domain vocabulary --- the main lever for proper
    // nouns and terms of art.  Borrowed from `cfg`, which outlives `params`.
    if let Some(p) = d.initial_prompt.as_deref() {
        params.set_initial_prompt(p);
    }

    // Note what is *not* here: `set_vad_model_path` / `enable_vad`. Those are a
    // no-op through `whisper_full_with_state`, so setting them would only make
    // the config look honoured when it is not. `apply_vad` above did the work.

    eprintln!("Whisper: running inference on {} samples", audio.len());
    state
        .full(params, &audio)
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

        // Words are assembled in the filtered timeline, then mapped back --- so
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
/// whisper.cpp uses the BPE space-prefix convention: a token whose decoded text
/// begins with an ASCII space marks the start of a new word.
///
/// Two things here are load-bearing:
///
/// **Special tokens are filtered by id, not by text.** The vocabulary reserves a
/// block at the top for them, so `id >= special_min` catches every form. Matching
/// on text is what let `[_BEG_]` and `[_TT_n]` through previously --- and `[_TT_n]`
/// carries no leading space, so it was silently concatenated onto the preceding
/// word (`"you.[_TT_1499]"`), corrupting the text that reaches the search index.
///
/// **`t_dtw` is an instant, not a span.** A word therefore has no end time of its
/// own; it ends where the next word begins. That is why this builds the token
/// list first and then zips over adjacent pairs, rather than folding intervals in
/// a single pass. The last word is the only one with nothing to zip against, and
/// takes the segment's end.
///
/// The parameter type uses `whisper_rs::WhisperSegment<'_>` (fully qualified) to
/// avoid a name collision with our own public `WhisperSegment` struct.
fn collect_words(
    seg: &whisper_rs::WhisperSegment<'_>,
    seg_start: f64,
    seg_end: f64,
    special_min: i32,
) -> Result<Vec<Word>> {
    /// One content token: its text, the instant DTW placed it at, and its
    /// probability.
    struct Tok<'a> {
        text: &'a str,
        at: f64,
        p: f32,
        starts_word: bool,
    }

    // n_tokens is a bare c_int --- no Result.
    let n_tokens = seg.n_tokens();
    let mut toks: Vec<Tok<'_>> = Vec::with_capacity(n_tokens as usize);

    // whisper.cpp initialises every token to `{..., t0: -1, t1: -1, t_dtw: -1, ...}`
    // and then assigns `t_dtw` only where the DTW backtrace changes column. Tokens
    // it never reaches keep the sentinel, so an unset time is normal rather than
    // exceptional and has to degrade gracefully.
    //
    // Carrying the last known instant forward is what keeps the stream monotonic.
    // Substituting 0.0 (which is what clamping a negative would do) would drag a
    // mid-segment word back to the start of the recording -- far worse than a
    // word that merely shares a timestamp with its predecessor.
    let mut last_at = seg_start;

    for j in 0..n_tokens {
        // get_token returns Option<WhisperToken<'_, '_>>; bounds are guaranteed here.
        let token = seg.get_token(j).context("token index out of bounds")?;

        // token_data() returns WhisperTokenData (whisper_rs_sys::whisper_token_data)
        // directly --- not a Result.  Times are i64 centiseconds.
        let td = token.token_data();
        if td.id >= special_min {
            continue;
        }

        let text = token.to_str().context("failed to read token text")?;

        let at = if td.t_dtw >= 0 {
            td.t_dtw as f64 / 100.0
        } else if td.t0 >= 0 {
            td.t0 as f64 / 100.0
        } else {
            last_at
        };
        let at = round_to(at, 2).max(last_at);
        last_at = at;

        toks.push(Tok {
            text,
            at,
            p: td.p,
            starts_word: text.starts_with(' '),
        });
    }

    // Group tokens into words. The first content token opens a word even without
    // a leading space, since a segment need not begin on a word boundary.
    let mut groups: Vec<(String, f64, f32)> = Vec::new();
    for t in &toks {
        let opens = t.starts_word || groups.is_empty();
        if opens {
            groups.push((t.text.trim_start_matches(' ').to_string(), t.at, t.p));
        } else {
            let last = groups.last_mut().expect("non-empty by construction");
            last.0.push_str(t.text);
            // Weakest link wins: a word is only as trustworthy as its least
            // confident sub-token.
            last.2 = last.2.min(t.p);
        }
    }

    // A word ends where the next begins; the last one ends with the segment.
    let words = groups
        .iter()
        .enumerate()
        .filter(|(_, (w, _, _))| !w.is_empty())
        .map(|(i, (word, start, p))| {
            let end = groups
                .get(i + 1)
                .map(|(_, next_start, _)| *next_start)
                .unwrap_or(seg_end)
                .max(*start);
            Word {
                word: word.clone(),
                start: *start,
                end,
                p: round_to(*p as f64, 2) as f32,
            }
        })
        .collect();

    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two speech regions, 10 s of silence dropped between them.
    ///
    ///   real:     [2.0 .. 5.0]            [15.0 .. 18.0]
    ///   filtered: [0.0 .. 3.0]  glue 0.1  [3.1  .. 6.1]
    fn timeline() -> VadTimeline {
        VadTimeline {
            regions: vec![
                VadRegion { orig_start: 2.0, filtered_start: 0.0, duration: 3.0 },
                VadRegion { orig_start: 15.0, filtered_start: 3.1, duration: 3.0 },
            ],
        }
    }

    #[test]
    fn identity_timeline_is_a_no_op() {
        let t = VadTimeline::identity();
        for x in [0.0, 1.5, 900.0] {
            assert_eq!(t.to_original(x), x);
        }
    }

    #[test]
    fn maps_filtered_time_back_to_the_recording() {
        let t = timeline();
        assert_eq!(t.to_original(0.0), 2.0, "first region start");
        assert_eq!(t.to_original(1.5), 3.5, "inside first region");
        assert_eq!(t.to_original(3.0), 5.0, "first region end");
        assert_eq!(t.to_original(3.1), 15.0, "second region start");
        assert!((t.to_original(4.5) - 16.4).abs() < 1e-9, "inside second region");
        assert!((t.to_original(6.1) - 18.0).abs() < 1e-9, "second region end");
    }

    #[test]
    fn the_gap_the_mapping_exists_to_close() {
        // Without mapping, a word 4.5 s into the filtered audio would be reported
        // at 4.5 s of the recording. It is really at 16.4 s -- and the error grows
        // with every silence dropped, so a long interview drifts badly.
        let t = timeline();
        assert!((t.to_original(4.5) - 4.5 - 11.9).abs() < 1e-9, "error the map removes");
    }

    #[test]
    fn glue_silence_clamps_to_the_preceding_region() {
        // No audio exists in the glue, so a time landing there belongs to the
        // speech it was decoded from -- the region before it.
        assert_eq!(timeline().to_original(3.05), 5.0);
    }

    #[test]
    fn times_past_the_end_clamp_rather_than_extrapolate() {
        // Whisper can place a trailing timestamp past the last sample. Clamping
        // keeps it inside the recording; extrapolating would invent audio.
        assert_eq!(timeline().to_original(99.0), 18.0);
    }

    #[test]
    fn mapping_is_monotonic() {
        // Word order must survive the map, or the caption editor shows words
        // jumping backwards.
        let t = timeline();
        let mut prev = f64::NEG_INFINITY;
        for i in 0..=610 {
            let cur = t.to_original(i as f64 / 100.0);
            assert!(cur >= prev, "went backwards at {}: {} < {}", i, cur, prev);
            prev = cur;
        }
    }
}

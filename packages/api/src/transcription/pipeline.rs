//! Transcription pipeline: orchestrates audio decoding --> Whisper ASR -->
//! pyannote diarization --> merge into speaker-attributed segments.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::align;
use super::audio;
use super::diarize;
use super::types::*;
use super::whisper;

/// Configuration for the transcription pipeline.
///
/// Whisper weights are downloaded automatically from HuggingFace Hub (cached
/// in `~/.cache/huggingface/hub/`), so only the pyannote ONNX models need a
/// local directory.
pub struct PipelineConfig {
    pub segmentation_model: PathBuf,
    pub embedding_model: PathBuf,
    pub max_speakers: usize,
}

impl PipelineConfig {
    pub fn from_model_dir(dir: &Path, max_speakers: usize) -> Self {
        Self {
            segmentation_model: dir.join("pyannote-segmentation-3.0.onnx"),
            embedding_model: dir.join("wespeaker_en_voxceleb_ECAPA1024.onnx"),
            max_speakers,
        }
    }

    /// Build a config from the `MODELS_DIR` environment variable, falling back
    /// to `"models"` relative to the working directory if unset.
    pub fn from_env(max_speakers: usize) -> Self {
        let dir = std::env::var("MODELS_DIR").unwrap_or_else(|_| "models".to_string());
        Self::from_model_dir(Path::new(&dir), max_speakers)
    }
}

/// Run the full transcription pipeline on an audio/video file.
///
/// This is blocking (Whisper and pyannote are CPU-bound). Call from
/// `tokio::task::spawn_blocking` to avoid stalling the async runtime.
pub fn run(config: &PipelineConfig, input_path: &Path) -> Result<TranscriptionResult> {
    let decoded = audio::decode_file(input_path)
        .with_context(|| format!("failed to decode {}", input_path.display()))?;

    let mut whisper_model = whisper::load_model()?;
    let mut whisper_segments = whisper::transcribe(&mut whisper_model, &decoded.samples)?;

    // Refine word-level timestamps via wav2vec2 CTC forced alignment.
    // This replaces the proportional approximation from Whisper's timestamp
    // tokens with precise character-level alignment (≈20 ms resolution).
    let mut aligner = align::Aligner::load()?;
    aligner.refine_segments(&mut whisper_segments, &decoded.samples)?;

    let samples_i16 = diarize::f32_to_i16(&decoded.samples);
    let diarized = diarize::diarize(
        &samples_i16,
        decoded.sample_rate,
        &config.segmentation_model,
        &config.embedding_model,
        config.max_speakers,
    )?;

    let segments = merge_whisper_with_diarization(&whisper_segments, &diarized);

    let mut speakers: Vec<String> = segments.iter().map(|s| s.speaker.clone()).collect();
    speakers.sort();
    speakers.dedup();

    Ok(TranscriptionResult {
        segments,
        speakers,
        diarized,
        whisper_segments,
    })
}

/// Assign a speaker label to each *word* by maximum temporal overlap with
/// diarized segments, then group consecutive same-speaker words into segments.
fn merge_whisper_with_diarization(
    whisper_segments: &[whisper::WhisperSegment],
    diarized: &[diarize::DiarizedSegment],
) -> Vec<Segment> {
    // Step 1: Flatten all words from every Whisper segment into one stream,
    // each labeled with the best-matching speaker.
    whisper_segments
        .iter()
        .map(|segment| {
            let speaker = best_speaker_overlap(segment.start, segment.end, diarized);
            Segment {
                speaker,
                text: segment.text.clone(),
                start_time: segment.start,
                end_time: segment.end,
                words: segment.words.clone(),
            }
        })
        .collect()
}

fn best_speaker_overlap(start: f64, end: f64, diarized: &[diarize::DiarizedSegment]) -> String {
    // Primary: pick the diarized segment with the largest *positive* temporal
    // overlap with [start, end]. Filtering out zero-overlap candidates is
    // critical---without it, zero-width Whisper segments (start == end) tie
    // every diarized segment at 0.0 and `max_by` silently returns the last
    // one (typically SPEAKER_01), discarding genuine SPEAKER_00/SPEAKER_02
    // hits in the diarization output.
    let best = diarized
        .iter()
        .filter_map(|d| {
            let overlap = (end.min(d.end) - start.max(d.start)).max(0.0);
            (overlap > 0.0).then_some((overlap, &d.speaker))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, speaker)| speaker.clone());

    if let Some(speaker) = best {
        return speaker;
    }

    // Fallback: no diarized segment overlaps [start, end] (common for the
    // zero-width Whisper segments produced when the aligner can't refine an
    // interval). Pick a "nearest" diarized segment by some rule of thumb.
    nearest_speaker_fallback(start, end, diarized)
}

/// Choose a speaker label when no diarized segment temporally overlaps the
/// Whisper interval `[start, end]`. Several reasonable rules exist; the one
/// you pick shapes how aggressively brief interviewer interjections survive
/// the merge.
fn nearest_speaker_fallback(start: f64, end: f64, diarized: &[diarize::DiarizedSegment]) -> String {
    // Edge distance: nearest gap between the query interval [start, end] and
    // any edge of a diarized segment. Better than midpoint distance for
    // catching brief speakers (a 0.4 s SPEAKER_00 tangent to the query beats
    // a 30 s SPEAKER_01 whose midpoint happens to be closer overall).
    diarized
        .iter()
        .min_by(|a, b| {
            let a_dist = (a.end - start).abs().min((end - a.start).abs());
            let b_dist = (b.end - start).abs().min((end - b.start).abs());
            a_dist.total_cmp(&b_dist)
        })
        .map(|d| d.speaker.clone())
        .unwrap_or_else(|| "SPEAKER_00".to_string())
}

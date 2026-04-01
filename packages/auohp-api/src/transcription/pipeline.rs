//! Transcription pipeline: orchestrates audio decoding --> Whisper ASR -->
//! pyannote diarization --> merge into speaker-attributed segments.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::audio;
use super::diarize;
use super::types::*;
use super::whisper;

/// Configuration for the transcription pipeline.
pub struct PipelineConfig {
    pub whisper_model: PathBuf,
    pub segmentation_model: PathBuf,
    pub embedding_model: PathBuf,
    pub max_speakers: usize,
}

impl PipelineConfig {
    pub fn from_model_dir(dir: &Path, max_speakers: usize) -> Self {
        Self {
            whisper_model: dir.join("ggml-large-v3.bin"),
            segmentation_model: dir.join("pyannote-segmentation-3.0.onnx"),
            embedding_model: dir.join("wespeaker_en_voxceleb_CAM++.onnx"),
            max_speakers,
        }
    }
}

/// Run the full transcription pipeline on an audio/video file.
///
/// This is blocking (Whisper and pyannote are CPU-bound). Call from
/// `tokio::task::spawn_blocking` to avoid stalling the async runtime.
pub fn run(config: &PipelineConfig, input_path: &Path) -> Result<TranscriptionResult> {
    let decoded = audio::decode_file(input_path)
        .with_context(|| format!("failed to decode {}", input_path.display()))?;

    let whisper_ctx = whisper::load_model(&config.whisper_model)?;
    let whisper_segments = whisper::transcribe(&whisper_ctx, &decoded.samples)?;

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

    Ok(TranscriptionResult { segments, speakers })
}

/// Assign a speaker label to each *word* by maximum temporal overlap with
/// diarized segments, then group consecutive same-speaker words into segments.
fn merge_whisper_with_diarization(
    whisper_segments: &[whisper::WhisperSegment],
    diarized: &[diarize::DiarizedSegment],
) -> Vec<Segment> {
    // Step 1: Flatten all words from every Whisper segment into one stream,
    // each labeled with the best-matching speaker.
    let labeled_words: Vec<(String, Word)> = whisper_segments
        .iter()
        .flat_map(|ws| &ws.words)
        .map(|word| {
            let speaker = best_speaker_overlap(word.start_time, word.end_time, diarized);
            (speaker, word.clone())
        })
        .collect();

    // Step 2: Group consecutive same-speaker words into Segments.
    group_labeled_words(labeled_words)
}

fn group_labeled_words(labeled_words: Vec<(String, Word)>) -> Vec<Segment> {
    let mut segments: Vec<Segment> = Vec::new();

    for (speaker, word) in labeled_words {
        let should_merge = segments
            .last()
            .map(|prev| prev.speaker == speaker)
            .unwrap_or(false);

        if should_merge {
            let current = segments.last_mut().unwrap();
            current.text.push(' ');
            current.text.push_str(&word.word);
            current.end_time = word.end_time;
            current.words.push(word);
        } else {
            segments.push(Segment {
                speaker,
                text: word.word.clone(),
                start_time: word.start_time,
                end_time: word.end_time,
                words: vec![word],
            });
        }
    }

    segments
}

fn best_speaker_overlap(start: f64, end: f64, diarized: &[diarize::DiarizedSegment]) -> String {
    diarized
        .iter()
        .map(|d| {
            let overlap = (end.min(d.end) - start.max(d.start)).max(0.0);
            (overlap, &d.speaker)
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, speaker)| speaker.clone())
        .unwrap_or_else(|| "SPEAKER_00".to_string())
}

//! Transcription pipeline: orchestrates audio decoding → Whisper ASR →
//! pyannote diarization → merge into speaker-attributed segments.

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
            whisper_model: dir.join("ggml-medium.en-q8_0.bin"),
            segmentation_model: dir.join("pyannote-segmentation-3.0.onnx"),
            embedding_model: dir.join("wespeaker-voxceleb-resnet34-LM.onnx"),
            max_speakers,
        }
    }
}

/// Run the full transcription pipeline on an audio/video file.
///
/// This is blocking (Whisper and pyannote are CPU-bound). Call from
/// `tokio::task::spawn_blocking` to avoid stalling the async runtime.
pub fn run(
    config: &PipelineConfig,
    input_path: &Path,
    progress: Option<&ProgressTx>,
) -> Result<TranscriptionResult> {
    let decoded = audio::decode_file(input_path, progress)
        .with_context(|| format!("failed to decode {}", input_path.display()))?;

    let whisper_ctx = whisper::load_model(&config.whisper_model)?;
    let whisper_segments = whisper::transcribe(&whisper_ctx, &decoded.samples, progress)?;

    let samples_i16 = diarize::f32_to_i16(&decoded.samples);
    let diarized = diarize::diarize(
        &samples_i16,
        decoded.sample_rate,
        &config.segmentation_model,
        &config.embedding_model,
        config.max_speakers,
        progress,
    )?;

    if let Some(tx) = progress {
        let _ = tx.send(ProgressEvent::new(TranscriptionPhase::Assembling, 0.0));
    }

    let segments = merge_whisper_with_diarization(&whisper_segments, &diarized);

    if let Some(tx) = progress {
        let _ = tx.send(ProgressEvent::new(TranscriptionPhase::Assembling, 1.0));
    }

    let mut speakers: Vec<String> = segments.iter().map(|s| s.speaker.clone()).collect();
    speakers.sort();
    speakers.dedup();

    Ok(TranscriptionResult { segments, speakers })
}

/// Assign a speaker label to each Whisper segment by maximum temporal overlap
/// with diarized segments, then merge consecutive same-speaker segments.
fn merge_whisper_with_diarization(
    whisper_segments: &[whisper::WhisperSegment],
    diarized: &[diarize::DiarizedSegment],
) -> Vec<Segment> {
    let labeled: Vec<Segment> = whisper_segments
        .iter()
        .map(|ws| {
            let speaker = best_speaker_overlap(ws.start, ws.end, diarized);
            Segment {
                speaker,
                text: ws.text.clone(),
                start_time: ws.start,
                end_time: ws.end,
                words: ws.words.clone(),
            }
        })
        .collect();

    let mut merged: Vec<Segment> = Vec::new();
    for seg in labeled {
        let should_merge = merged
            .last()
            .map(|prev| prev.speaker == seg.speaker)
            .unwrap_or(false);

        if should_merge {
            let current = merged.last_mut().unwrap();
            current.text.push(' ');
            current.text.push_str(&seg.text);
            current.end_time = seg.end_time;
            current.words.extend(seg.words);
        } else {
            merged.push(seg);
        }
    }

    merged
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

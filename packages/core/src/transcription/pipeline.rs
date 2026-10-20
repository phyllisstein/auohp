//! Transcription pipeline: orchestrates audio decoding --> VAD --> Whisper ASR
//! --> speaker diarization into word-timed, speaker-labeled segments.

use super::audio;
use super::config::TranscribeConfig;
use super::diarize;
use super::diarize::segmentation;
use super::types::*;
use super::whisper;
use crate::models;
use anyhow::{Context, Result};
use std::path::Path;

/// Run the transcription pipeline on an audio/video file.
///
/// Returns Whisper segments with per-word DTW timestamps, each labeled with a
/// turn-level speaker (`SPEAKER_00`, ...) when `cfg.diarize.enabled`. Blocking:
/// call from `tokio::task::spawn_blocking`.
pub fn run(input_path: &Path) -> Result<TranscriptionResult> {
    run_with(input_path, &TranscribeConfig::default())
}

/// Run the pipeline under an explicit configuration.
///
/// `cfg` plus a git SHA fully describes a run; the experiment ledger relies on it.
pub fn run_with(input_path: &Path, cfg: &TranscribeConfig) -> Result<TranscriptionResult> {
    let decoded = audio::decode_file_with(input_path, &cfg.audio)
        .with_context(|| format!("failed to decode {}", input_path.display()))?;

    tracing::debug!(
        "Audio: {} Hz / {} ch source -> {} samples at {} Hz mono ({:.1}s){}",
        decoded.source_sample_rate,
        decoded.source_channels,
        decoded.samples.len(),
        decoded.sample_rate,
        decoded.samples.len() as f64 / decoded.sample_rate as f64,
        if decoded.source_sample_rate == decoded.sample_rate && decoded.source_channels == 1 {
            "  [transform chain bypassed]"
        } else {
            ""
        }
    );

    // Load the model after decoding, not before. On Apple Silicon unified memory
    // the decode buffer (~3.8 GB for a 2.6-hour stereo master) and the model
    // (~4.4 GB) share one pool: sequenced peak is ~5.0 GB, overlapped ~8.2 GB.
    // Validate the model path early if failing fast is the goal, not the weights.
    let models_dir = models::models_dir();

    let (whisper_segments, diarized) = std::thread::scope(|s| {
        let whisper_thread = s.spawn(|| {
            let mut whisper_model = whisper::load_model(
                &models_dir.join(whisper::MODEL_FILE),
                &models_dir.join(whisper::VAD_MODEL_FILE),
            )?;
            whisper::transcribe(&mut whisper_model, &decoded.samples, cfg)
        });

        let diarize_thread = s.spawn(|| {
            if cfg.diarize.enabled {
                diarize::diarize(
                    &decoded.samples,
                    decoded.sample_rate,
                    &models_dir.join(segmentation::MODEL_FILE),
                    &models_dir.join(diarize::EMBEDDING_MODEL_FILE),
                    cfg.diarize.max_speakers,
                )
            } else {
                Ok(Vec::new())
            }
        });

        let w_res = whisper_thread.join().unwrap()?;
        let d_res = diarize_thread.join().unwrap()?;
        Ok::<_, anyhow::Error>((w_res, d_res))
    })?;

    let segments: Vec<Segment> = whisper_segments
        .into_iter()
        .map(|s| Segment {
            // Unmatched segments stay `None`: a human still needs to label them.
            speaker: diarize::dominant_speaker(s.start, s.end, &diarized).map(str::to_owned),
            text: s.text,
            start_time: s.start,
            end_time: s.end,
            words: s.words,
        })
        .collect();

    Ok(segments.into())
}

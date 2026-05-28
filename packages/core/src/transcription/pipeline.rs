//! Transcription pipeline: orchestrates audio decoding --> VAD --> Whisper ASR
//! into word-timed segments.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::audio;
use super::types::*;
use super::whisper;

const WHISPER_MODEL_FILE: &str = "ggml-large-v3.bin";
const VAD_MODEL_FILE: &str = "ggml-silero-v6.2.0.bin";
const DEFAULT_MODELS_DIR: &str = "/opt/auohp/models";

/// Run the transcription pipeline on an audio/video file.
///
/// Returns Whisper segments with per-word timestamps from DTW.  Speaker labels
/// are not assigned here---they are filled in later via the manual labeling UI.
///
/// This is blocking (Whisper is CPU-bound). Call from
/// `tokio::task::spawn_blocking` to avoid stalling the async runtime.
pub fn run(input_path: &Path) -> Result<TranscriptionResult> {
    let decoded = audio::decode_file(input_path)
        .with_context(|| format!("failed to decode {}", input_path.display()))?;

    // All models live under $MODELS_DIR, pre-downloaded by download-models.sh.
    let models_dir = PathBuf::from(
        std::env::var("MODELS_DIR").unwrap_or_else(|_| DEFAULT_MODELS_DIR.to_string()),
    );

    let mut whisper_model = whisper::load_model(
        &models_dir.join(WHISPER_MODEL_FILE),
        &models_dir.join(VAD_MODEL_FILE),
    )?;
    let whisper_segments = whisper::transcribe(&mut whisper_model, &decoded.samples)?;

    let segments: Vec<Segment> = whisper_segments
        .iter()
        .map(|s| Segment {
            speaker: None,
            text: s.text.clone(),
            start_time: s.start,
            end_time: s.end,
            words: s.words.clone(),
        })
        .collect();

    Ok(TranscriptionResult { segments })
}

//! Transcription pipeline: orchestrates audio decoding --> Whisper ASR -->
//! wav2vec2 forced alignment into word-timed segments.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::align;
use super::audio;
use super::types::*;
use super::whisper;

const WHISPER_MODEL_FILE: &str = "ggml-large-v3.bin";
const DEFAULT_MODELS_DIR: &str = "/opt/auohp/models";

/// Run the transcription pipeline on an audio/video file.
///
/// Returns Whisper segments with refined per-word timestamps. Speaker labels
/// are not assigned here --- they are filled in later via the manual labeling UI.
///
/// This is blocking (Whisper and wav2vec2 are CPU-bound). Call from
/// `tokio::task::spawn_blocking` to avoid stalling the async runtime.
pub fn run(input_path: &Path) -> Result<TranscriptionResult> {
    let decoded = audio::decode_file(input_path)
        .with_context(|| format!("failed to decode {}", input_path.display()))?;

    // Resolve model path from $MODELS_DIR, matching scripts/download-models.sh.
    let models_dir = std::env::var("MODELS_DIR").unwrap_or_else(|_| DEFAULT_MODELS_DIR.to_string());
    let model_path: PathBuf = [&models_dir, WHISPER_MODEL_FILE].iter().collect();
    let mut whisper_model = whisper::load_model(&model_path)?;
    let mut whisper_segments = whisper::transcribe(&mut whisper_model, &decoded.samples)?;

    // Refine word-level timestamps via wav2vec2 CTC forced alignment.
    // This replaces the proportional approximation from Whisper's timestamp
    // tokens with precise character-level alignment (≈20 ms resolution).
    let mut aligner = align::Aligner::load()?;
    aligner.refine_segments(&mut whisper_segments, &decoded.samples)?;

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

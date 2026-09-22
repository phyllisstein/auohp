use super::{SEGMENTATION_MODEL_FILE, VAD_MODEL_FILE, WHISPER_MODEL_FILE};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A word with its timing from Whisper's DTW alignment.
///
/// `Deserialize` is here so the scoring harness can read archived `result.json`
/// files back. That is what lets a metric change be re-applied to every past run
/// on CPU, instead of re-running them on the GPU.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Word {
    pub word: String,
    pub start: f64,
    pub end: f64,
    pub p: f32,
}

/// A transcription segment---one contiguous block of speech from Whisper.
///
/// `speaker` is always `None` coming out of the pipeline; it will be filled in
/// by the user through the manual labeling UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub speaker: Option<String>,
    pub text: String,
    pub start_time: f64,
    pub end_time: f64,
    pub words: Vec<Word>,
}

/// The output of the transcription pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub segments: Vec<Segment>,
    pub created: DateTime<Utc>,
    pub speakers: Option<Vec<String>>,
    pub models: Option<ModelConfig>,
}

impl TranscriptionResult {
    fn from_segments(segments: Vec<Segment>) -> Self {
        let speakers: Vec<String> = segments
            .iter()
            .map(|s| s.speaker.clone())
            .flatten()
            .fold(HashSet::new(), |mut acc, el| {
                acc.insert(el.clone());
                acc
            })
            .into_iter()
            .collect();

        TranscriptionResult {
            segments,
            speakers: Some(speakers.into()),
            created: Utc::now(),
            models: Some(ModelConfig {
                segmentation_model: SEGMENTATION_MODEL_FILE.into(),
                vad_model: VAD_MODEL_FILE.into(),
                whisper_model: WHISPER_MODEL_FILE.into(),
            }),
        }
    }
}

impl From<Vec<Segment>> for TranscriptionResult {
    fn from(segments: Vec<Segment>) -> Self {
        TranscriptionResult::from_segments(segments)
    }
}

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct ModelConfig {
    pub segmentation_model: String,
    pub vad_model: String,
    pub whisper_model: String,
}

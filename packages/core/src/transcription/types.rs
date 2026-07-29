use serde::{Deserialize, Serialize};

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
}

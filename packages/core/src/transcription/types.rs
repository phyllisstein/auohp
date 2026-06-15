use serde::Serialize;

/// A word with its timing from Whisper's DTW alignment.
#[derive(Debug, Clone, Serialize, PartialEq)]
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
#[derive(Debug, Clone, Serialize)]
pub struct Segment {
    pub speaker: Option<String>,
    pub text: String,
    pub start_time: f64,
    pub end_time: f64,
    pub words: Vec<Word>,
}

/// The output of the transcription pipeline.
#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionResult {
    pub segments: Vec<Segment>,
}

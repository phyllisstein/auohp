use serde::Serialize;

use crate::transcription::whisper::WhisperSegment;

/// A word with its timing from Whisper's DTW alignment.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Word {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

/// A transcription segment---one contiguous block of speech by one speaker.
#[derive(Debug, Clone, Serialize)]
pub struct Segment {
    pub speaker: String,
    pub text: String,
    pub start_time: f64,
    pub end_time: f64,
    pub words: Vec<Word>,
}

/// The output of the transcription pipeline.
#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionResult {
    pub segments: Vec<Segment>,
    pub whisper_segments: Vec<WhisperSegment>,
    pub speakers: Vec<String>,
}

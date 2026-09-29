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
            .filter_map(|s| s.speaker.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        TranscriptionResult {
            segments,
            speakers: Some(speakers),
            created: Utc::now(),
            models: Some(ModelConfig {
                segmentation_model: SEGMENTATION_MODEL_FILE.into(),
                vad_model: VAD_MODEL_FILE.into(),
                whisper_model: WHISPER_MODEL_FILE.into(),
            }),
        }
    }
}

impl FromIterator<Segment> for TranscriptionResult {
    fn from_iter<T: IntoIterator<Item = Segment>>(iter: T) -> Self {
        Self::from(iter.into_iter().collect::<Vec<Segment>>())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcription_result_from_iterator() {
        let segments = vec![
            Segment {
                speaker: Some("SPEAKER_00".into()),
                text: "Hello".into(),
                start_time: 0.0,
                end_time: 1.0,
                words: vec![],
            },
            Segment {
                speaker: Some("SPEAKER_01".into()),
                text: "World".into(),
                start_time: 1.0,
                end_time: 2.0,
                words: vec![],
            },
            Segment {
                speaker: Some("SPEAKER_00".into()),
                text: "Again".into(),
                start_time: 2.0,
                end_time: 3.0,
                words: vec![],
            },
            Segment {
                speaker: None,
                text: "Silence".into(),
                start_time: 3.0,
                end_time: 4.0,
                words: vec![],
            },
        ];

        let result: TranscriptionResult = segments.into_iter().collect();
        assert_eq!(result.segments.len(), 4);
        let speakers = result.speakers.expect("speakers should be populated");
        assert_eq!(speakers.len(), 2);
        assert!(speakers.contains(&"SPEAKER_00".to_string()));
        assert!(speakers.contains(&"SPEAKER_01".to_string()));
        assert!(result.models.is_some());
    }
}

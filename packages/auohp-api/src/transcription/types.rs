use serde::Serialize;
use tokio::sync::broadcast;

/// A word with its timing from Whisper's DTW alignment.
#[derive(Debug, Clone, Serialize)]
pub struct Word {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

/// A transcription segment — one contiguous block of speech by one speaker.
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
    pub speakers: Vec<String>,
}

/// Which phase the pipeline is currently executing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptionPhase {
    Decoding,
    Transcribing,
    Diarizing,
    Assembling,
}

/// Progress update emitted during transcription.
#[derive(Debug, Clone)]
pub struct ProgressEvent {
    pub phase: TranscriptionPhase,
    pub phase_progress: f32,
    pub overall_progress: f32,
}

pub type ProgressTx = broadcast::Sender<ProgressEvent>;
pub type ProgressRx = broadcast::Receiver<ProgressEvent>;

pub fn progress_channel() -> (ProgressTx, ProgressRx) {
    broadcast::channel(64)
}

// Phase weights for overall progress. Whisper inference dominates.
const PHASE_WEIGHTS: [(TranscriptionPhase, f32); 4] = [
    (TranscriptionPhase::Decoding, 0.05),
    (TranscriptionPhase::Transcribing, 0.75),
    (TranscriptionPhase::Diarizing, 0.15),
    (TranscriptionPhase::Assembling, 0.05),
];

impl ProgressEvent {
    pub fn new(phase: TranscriptionPhase, phase_progress: f32) -> Self {
        let mut overall = 0.0f32;
        for &(p, weight) in &PHASE_WEIGHTS {
            if p == phase {
                overall += weight * phase_progress;
                break;
            }
            overall += weight;
        }
        Self {
            phase,
            phase_progress,
            overall_progress: overall.clamp(0.0, 1.0),
        }
    }
}

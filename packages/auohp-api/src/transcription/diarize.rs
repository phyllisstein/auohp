//! Speaker diarization via pyannote-rs (ONNX Runtime).
//!
//! Segments audio into speaker turns and assigns each a label (SPEAKER_01,
//! SPEAKER_02, etc.). Uses pyannote's segmentation-3.0 model for speech
//! detection and wespeaker embeddings for speaker identification.

use anyhow::{Context, Result};
use pyannote_rs::{EmbeddingExtractor, EmbeddingManager};
use std::path::Path;

use super::types::{ProgressEvent, ProgressTx, TranscriptionPhase};

/// A diarized speech segment: a time range attributed to a speaker.
pub struct DiarizedSegment {
    /// Speaker label (e.g. "SPEAKER_01").
    pub speaker: String,
    /// Start time in seconds.
    pub start: f64,
    /// End time in seconds.
    pub end: f64,
}

/// Run speaker diarization on 16-bit PCM audio samples.
///
/// `max_speakers` hints the maximum expected number of distinct speakers
/// (e.g. 3 for a typical AUOHP interview: two interviewers + one interviewee).
pub fn diarize(
    samples_i16: &[i16],
    sample_rate: u32,
    segmentation_model: &Path,
    embedding_model: &Path,
    max_speakers: usize,
    progress: Option<&ProgressTx>,
) -> Result<Vec<DiarizedSegment>> {
    let seg_path = segmentation_model
        .to_str()
        .context("segmentation model path is not valid UTF-8")?;
    let emb_path = embedding_model
        .to_str()
        .context("embedding model path is not valid UTF-8")?;

    // Phase 1: segment audio into speech regions.
    let raw_segments: Vec<pyannote_rs::Segment> =
        pyannote_rs::get_segments(samples_i16, sample_rate, seg_path)
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .filter_map(|r| r.ok())
            .collect();

    let total = raw_segments.len();
    if total == 0 {
        return Ok(Vec::new());
    }

    // Phase 2: extract embeddings and cluster into speakers.
    let mut embedding_extractor = EmbeddingExtractor::new(emb_path)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut manager = EmbeddingManager::new(max_speakers);

    let mut diarized = Vec::with_capacity(total);

    for (i, seg) in raw_segments.iter().enumerate() {
        let embedding: Vec<f32> = embedding_extractor
            .compute(&seg.samples)
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .collect();

        // Try to match to an existing speaker or create a new one.
        let speaker_id = match manager.search_speaker(embedding.clone(), 0.5) {
            Some(id) => id,
            None => {
                // At capacity — force-match to most similar existing speaker.
                manager
                    .get_best_speaker_match(embedding)
                    .map_err(|e| anyhow::anyhow!("{e}"))?
            }
        };

        diarized.push(DiarizedSegment {
            speaker: format!("SPEAKER_{:02}", speaker_id),
            start: seg.start,
            end: seg.end,
        });

        if let Some(tx) = progress {
            let _ = tx.send(ProgressEvent::new(
                TranscriptionPhase::Diarizing,
                (i + 1) as f32 / total as f32,
            ));
        }
    }

    Ok(diarized)
}

/// Convert f32 samples ([-1.0, 1.0]) to i16 for pyannote-rs.
pub fn f32_to_i16(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|&s| (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16)
        .collect()
}

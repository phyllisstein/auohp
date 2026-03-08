//! Speaker diarization via pyannote-rs (ONNX Runtime).
//!
//! Segments audio into speaker turns and assigns each a label (SPEAKER_01,
//! SPEAKER_02, etc.). Uses pyannote's segmentation-3.0 model for speech
//! detection and wespeaker embeddings for speaker identification.

use anyhow::{Context, Result};
use pyannote_rs::{EmbeddingExtractor, EmbeddingManager};
use std::path::Path;
use std::time::Instant;

use super::types::{ProgressEvent, ProgressTx, TranscriptionPhase};

/// A diarized speech segment: a time range attributed to a speaker.
#[derive(Debug, Clone, serde::Serialize)]
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

    // FIXME: replace eprintln!/println! throughout this module with
    // `tracing::{debug, warn, error}` once tracing is wired into the
    // pipeline.  Skipped per-segment errors in particular should be
    // surfaced as structured warnings so callers can decide whether
    // a high error rate is worth aborting the job.

    // Phase 1: segment audio into speech regions.
    eprintln!(
        "[diarize] samples_i16 len={}, sample_rate={}",
        samples_i16.len(),
        sample_rate
    );
    let raw_segments: Vec<pyannote_rs::Segment> =
        pyannote_rs::get_segments(samples_i16, sample_rate, seg_path)
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .filter_map(|r| match r {
                Ok(seg) => Some(seg),
                Err(e) => {
                    eprintln!("[diarize] segment error (skipped): {e}");
                    None
                }
            })
            .collect();

    let total = raw_segments.len();
    if total == 0 {
        eprintln!("No speech segments detected by pyannote-rs.");
        return Ok(Vec::new());
    }

    // Phase 2: extract embeddings and cluster into speakers.
    let mut embedding_extractor = EmbeddingExtractor::new(emb_path)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut manager = EmbeddingManager::new(max_speakers);

    let mut diarized = Vec::with_capacity(total);
    let started_at = Instant::now();

    for (i, seg) in raw_segments.iter().enumerate() {
        // Estimate time remaining by extrapolating from elapsed time and
        // fraction of segments completed.  We guard against i == 0 to avoid
        // a division-by-zero on the very first iteration.
        let diarizing_elapsed_secs = started_at.elapsed().as_secs_f32();
        let diarizing_remaining_secs = if i > 0 {
            diarizing_elapsed_secs / i as f32 * (total - i) as f32
        } else {
            f32::INFINITY
        };
        tracing::debug!(
            segment_number = i + 1,
            total_segments = total,
            diarizing_elapsed_secs = format!("{:.1}", diarizing_elapsed_secs),
            diarizing_remaining_secs = if diarizing_remaining_secs.is_finite() {
                format!("{:.1}", diarizing_remaining_secs)
            } else {
                "?".to_string()
            },
            segment_start_secs = seg.start,
            segment_end_secs = seg.end,
            "diarizing segment"
        );
        // knf-rs uses a 25ms analysis window at 16 kHz (= 400 samples).
        // Segments shorter than one frame produce an empty filterbank array
        // and cause a hard error in compute_fbank, so we skip them here.
        const MIN_SAMPLES: usize = 400;
        if seg.samples.len() < MIN_SAMPLES {
            eprintln!(
                "[diarize] segment {} too short ({} samples < {}), skipping",
                i,
                seg.samples.len(),
                MIN_SAMPLES
            );
            continue;
        }

        let embedding: Vec<f32> = embedding_extractor
            .compute(&seg.samples)
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .collect();

        if embedding.iter().all(|&x| x == 0.0) {
            // Skip silent segments that somehow got through the segmentation model.
            eprintln!("Warning: segment {} has zero embedding and will be skipped", i);
            continue;
        }

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

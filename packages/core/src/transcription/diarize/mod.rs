//! Speaker diarization: pyannote segmentation, wespeaker embeddings, and
//! agglomerative clustering capped at a maximum speaker count.
//!
//! ## Model
//!
//! `wespeaker_en_voxceleb_ECAPA1024.onnx`: ECAPA-TDNN, 1024-dim, trained on
//! VoxCeleb. Input is kaldi-compatible log-mel fbank from `knf-rs`.

mod cluster;
mod embedding;
pub mod segmentation;

pub use cluster::{cluster_embeddings, cosine_distance};
pub use embedding::EMBEDDING_MODEL_FILE;
pub use embedding::extract_segment_embeddings;

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

/// A diarized speech segment: a time range attributed to a speaker.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiarizedSegment {
    /// Speaker label (e.g. "SPEAKER_01").
    pub speaker: String,
    pub start: f64,
    pub end: f64,
}

/// A speech segment and the speaker embedding extracted from it, before
/// clustering has decided which speaker it belongs to.
#[derive(Debug, Clone)]
pub struct SegmentEmbedding {
    pub start: f64,
    pub end: f64,
    pub embedding: Vec<f32>,
}

/// Run speaker diarization on 16 kHz mono f32 PCM samples.
///
/// `max_speakers` caps the number of distinct speaker clusters; AUOHP interviews
/// are usually two-person, but nothing here assumes exactly two.
pub fn diarize(
    samples: &[f32],
    sample_rate: u32,
    segmentation_model: &Path,
    embedding_model: &Path,
    max_speakers: usize,
) -> Result<Vec<DiarizedSegment>> {
    let segment_embeddings =
        extract_segment_embeddings(samples, sample_rate, segmentation_model, embedding_model)?;
    if segment_embeddings.is_empty() {
        return Ok(Vec::new());
    }

    let labels = cluster_embeddings(&segment_embeddings, max_speakers);

    Ok(segment_embeddings
        .iter()
        .zip(labels.iter())
        .map(|(seg, &speaker_id)| DiarizedSegment {
            speaker: format!("SPEAKER_{speaker_id:02}"),
            start: seg.start,
            end: seg.end,
        })
        .collect())
}

/// The speaker who holds the most of `start..end`, by total overlap. `None` when
/// nothing overlaps; callers treat that as "still needs a human label".
///
/// Totals are per speaker, not per segment: one Whisper segment typically spans
/// many diarized runs, and taking the longest single overlap would let one long
/// answer outvote many short runs from the speaker who holds most of the span.
pub fn dominant_speaker(start: f64, end: f64, diarized: &[DiarizedSegment]) -> Option<&str> {
    let mut totals: HashMap<&str, f64> = HashMap::new();
    for d in diarized {
        let overlap = (end.min(d.end) - start.max(d.start)).max(0.0);
        if overlap > 0.0 {
            *totals.entry(d.speaker.as_str()).or_insert(0.0) += overlap;
        }
    }
    totals
        .into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(speaker, _)| speaker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dominant_speaker_aggregation() {
        let diarized = vec![
            DiarizedSegment {
                speaker: "SPEAKER_00".into(),
                start: 0.0,
                end: 2.0,
            },
            DiarizedSegment {
                speaker: "SPEAKER_01".into(),
                start: 2.0,
                end: 5.0,
            },
            DiarizedSegment {
                speaker: "SPEAKER_00".into(),
                start: 5.0,
                end: 8.0,
            },
        ];

        // Range [0.0, 4.0]: SPEAKER_00 has [0.0..2.0] = 2.0s; SPEAKER_01 has [2.0..4.0] = 2.0s.
        // Range [0.0, 8.0]: SPEAKER_00 has 2.0s + 3.0s = 5.0s; SPEAKER_01 has 3.0s.
        assert_eq!(dominant_speaker(0.0, 8.0, &diarized), Some("SPEAKER_00"));
        // Range [2.5, 4.5]: only SPEAKER_01 overlaps.
        assert_eq!(dominant_speaker(2.5, 4.5, &diarized), Some("SPEAKER_01"));
        // Range [10.0, 12.0]: no overlap -> None
        assert_eq!(dominant_speaker(10.0, 12.0, &diarized), None);
    }
}

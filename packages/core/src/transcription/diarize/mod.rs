//! Speaker diarization: segmentation (see [`super::segmentation`]) + wespeaker
//! embeddings + agglomerative clustering, capped at a known maximum speaker
//! count.
//!
//! This is a restoration of a feature dropped in commit `469e9d4` ("Drop
//! diarization"), rebuilt against the current `packages/core` pipeline shape.
//! Segmentation goes through [`super::segmentation::Segmenter`] instead of
//! the buggy `pyannote_rs::get_segments` (see that module's doc comment for
//! why), and the embedding extractor is a direct `ort` + `knf-rs` call
//! instead of going through the `pyannote-rs` crate, for the same reason:
//! `pyannote-rs` doesn't compile against this workspace's `ort` version.
//! Clustering keeps the pre-drop version's overall shape (agglomerative,
//! cosine distance, capped cluster count) but switches linkage method from
//! `Average` to `Complete` --- validating against real interview audio
//! surfaced a real problem with the old choice; see
//! [`cluster::cluster_embeddings`]'s doc comment for the full story.
//!
//! ## Model
//!
//! `wespeaker_en_voxceleb_ECAPA1024.onnx` --- ECAPA-TDNN, 1024-dim, trained on
//! VoxCeleb, from the official WeSpeaker HuggingFace org (the model this
//! project had already upgraded to before diarization was set aside; see
//! commit `8348c36`). Takes log-mel filterbank features (`knf-rs`, the same
//! kaldi-compatible fbank extractor `pyannote-rs` used) and produces an
//! L2-normalizable speaker embedding.

mod cluster;
mod embedding;
pub mod segmentation;

pub use embedding::EMBEDDING_MODEL_FILE;
pub use cluster::{cluster_embeddings, cosine_distance};
pub use embedding::{extract_segment_embeddings, EmbeddingExtractor};

use std::collections::HashMap;
use std::path::Path;
use anyhow::Result;

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

/// Run speaker diarization on 16 kHz mono f32 PCM samples (whisper.cpp's
/// native format --- callers pass `DecodedAudio::samples` directly).
///
/// `max_speakers` caps the number of distinct speaker clusters. AUOHP
/// interviews are near-universally a two-person Q&A (interviewer +
/// interviewee); `4330866` ("Cap diarized speakers at 2") made 2 the
/// deliberate default the last time this ran, which [`super::config::DiarizeConfig`]
/// preserves as a default rather than an architectural limit --- nothing
/// here assumes exactly two.
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

/// The speaker who holds the most of `start..end`, by total overlap with the
/// diarized turns covering that span. `None` when nothing overlaps at all ---
/// callers treat that as "still needs a human label" rather than guessing.
///
/// Aggregating per *speaker* rather than per *segment* is load-bearing, not
/// incidental: diarization emits one `DiarizedSegment` per contiguous speech
/// run from the frame classifier, so a single Whisper segment routinely spans
/// many of them. Taking the longest individual overlapping segment would let
/// one uninterrupted eight-second answer outvote twenty short runs from the
/// speaker who actually holds most of the span.
pub fn dominant_speaker(
    start: f64,
    end: f64,
    diarized: &[DiarizedSegment],
) -> Option<&str> {
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

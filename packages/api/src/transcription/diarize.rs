//! Speaker diarization via pyannote-rs (ONNX Runtime) + agglomerative clustering.
//!
//! Segments audio into speaker turns and assigns each a label (SPEAKER_01,
//! SPEAKER_02, etc.). Uses pyannote's segmentation-3.0 model for speech
//! detection, wespeaker embeddings for speaker identity, and kodama's
//! hierarchical agglomerative clustering for global speaker assignment.

use anyhow::{Context, Result};
use kodama::{linkage, Method};
use pyannote_rs::EmbeddingExtractor;
use std::path::Path;
use std::time::Instant;

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
) -> Result<Vec<DiarizedSegment>> {
    let seg_path = segmentation_model
        .to_str()
        .context("segmentation model path is not valid UTF-8")?;
    let emb_path = embedding_model
        .to_str()
        .context("embedding model path is not valid UTF-8")?;

    // Phase 1: segment audio into speech regions.
    tracing::info!(
        samples = samples_i16.len(),
        sample_rate,
        duration_secs = samples_i16.len() as f64 / sample_rate as f64,
        "starting diarization"
    );
    let raw_segments: Vec<pyannote_rs::Segment> =
        pyannote_rs::get_segments(samples_i16, sample_rate, seg_path)
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .filter_map(|r| match r {
                Ok(seg) => Some(seg),
                Err(e) => {
                    tracing::warn!(error = %e, "skipping pyannote segment");
                    None
                }
            })
            .collect();

    let total = raw_segments.len();
    if total == 0 {
        tracing::warn!("no speech segments detected");
        return Ok(Vec::new());
    }

    // Phase 2: extract an embedding vector for each speech segment.
    let mut embedding_extractor =
        EmbeddingExtractor::new(emb_path).map_err(|e| anyhow::anyhow!("{e}"))?;

    // Each entry pairs a segment's time range with its speaker embedding.
    let mut segment_embeddings: Vec<(f64, f64, Vec<f32>)> = Vec::with_capacity(total);
    let mut skipped_short = 0usize;
    let mut skipped_nonfinite = 0usize;
    let mut skipped_zero = 0usize;
    let started_at = Instant::now();

    for (i, seg) in raw_segments.iter().enumerate() {
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
            "extracting embedding"
        );

        // knf-rs uses a 25ms analysis window at 16 kHz (= 400 samples).
        // Segments shorter than one frame produce an empty filterbank array
        // and cause a hard error in compute_fbank, so we skip them here.
        const MIN_SAMPLES: usize = 400;
        if seg.samples.len() < MIN_SAMPLES {
            skipped_short += 1;
            tracing::debug!(
                segment = i,
                samples = seg.samples.len(),
                min = MIN_SAMPLES,
                "segment too short, skipping"
            );
            continue;
        }

        let embedding: Vec<f32> = embedding_extractor
            .compute(&seg.samples)
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .collect();

        if embedding.iter().any(|x| !x.is_finite()) {
            skipped_nonfinite += 1;
            tracing::warn!(segment = i, "segment has non-finite embedding, skipping");
            continue;
        }
        if embedding.iter().all(|&x| x == 0.0) {
            skipped_zero += 1;
            tracing::warn!(segment = i, "segment has zero embedding, skipping");
            continue;
        }

        segment_embeddings.push((seg.start, seg.end, embedding));
    }

    let n = segment_embeddings.len();
    tracing::info!(
        raw_segments = total,
        kept = n,
        skipped_short,
        skipped_nonfinite,
        skipped_zero,
        "diarization phase 2 complete"
    );
    if n == 0 {
        return Ok(Vec::new());
    }

    // Phase 3: agglomerative clustering of embeddings.
    // Build a condensed cosine-distance matrix (upper triangle, row-major).
    let labels = cluster_embeddings(&segment_embeddings, max_speakers);

    let diarized = segment_embeddings
        .iter()
        .zip(labels.iter())
        .map(|((start, end, _), &speaker_id)| DiarizedSegment {
            speaker: format!("SPEAKER_{:02}", speaker_id),
            start: *start,
            end: *end,
        })
        .collect();

    Ok(diarized)
}

/// Cluster speaker embeddings using hierarchical agglomerative clustering.
///
/// Returns a Vec of speaker IDs (0-indexed), one per input segment, with at
/// most `max_speakers` distinct IDs.
fn cluster_embeddings(
    segment_embeddings: &[(f64, f64, Vec<f32>)],
    max_speakers: usize,
) -> Vec<usize> {
    let n = segment_embeddings.len();

    // Single segment---no clustering needed.
    if n == 1 {
        return vec![0];
    }

    // Build condensed cosine-distance matrix (upper triangle, row-major).
    // For N observations this has N*(N-1)/2 entries.
    let mut condensed: Vec<f64> = Vec::with_capacity(n * (n - 1) / 2);
    for i in 0..n - 1 {
        for j in i + 1..n {
            condensed.push(cosine_distance(
                &segment_embeddings[i].2,
                &segment_embeddings[j].2,
            ));
        }
    }

    let dendrogram = linkage(&mut condensed, n, Method::Average);

    // Cut the dendrogram to produce at most `max_speakers` clusters.
    // The dendrogram has N-1 steps. We want to stop merging when we'd drop
    // below max_speakers clusters. Starting from N clusters, each step
    // reduces the count by 1, so we take the first (N - max_speakers) steps.
    let steps = dendrogram.steps();
    let merges_to_make = n.saturating_sub(max_speakers);

    // Union-Find to track cluster membership.
    // Each observation starts as its own cluster. We apply merges in
    // dendrogram order (lowest dissimilarity first).
    let mut parent: Vec<usize> = (0..2 * n).collect();

    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]]; // path compression
            x = parent[x];
        }
        x
    }

    for (step_idx, step) in steps.iter().enumerate().take(merges_to_make) {
        // kodama labels: 0..N are original observations, N+i is the cluster
        // formed at step i.
        let new_cluster = n + step_idx;
        let a = find(&mut parent, step.cluster1);
        let b = find(&mut parent, step.cluster2);
        parent[a] = new_cluster;
        parent[b] = new_cluster;
    }

    // Map each observation to its final root, then relabel roots as
    // contiguous speaker IDs (0, 1, 2, ...).
    let roots: Vec<usize> = (0..n).map(|i| find(&mut parent, i)).collect();
    let mut label_map: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut next_label = 0usize;
    roots
        .iter()
        .map(|&root| {
            *label_map.entry(root).or_insert_with(|| {
                let l = next_label;
                next_label += 1;
                l
            })
        })
        .collect()
}

/// Cosine distance between two vectors: 1 - cos(a, b).
/// Returns 1.0 (maximally distant) if either vector has zero magnitude.
fn cosine_distance(a: &[f32], b: &[f32]) -> f64 {
    let mut dot = 0.0f64;
    let mut norm_a = 0.0f64;
    let mut norm_b = 0.0f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let x = *x as f64;
        let y = *y as f64;
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        1.0
    } else {
        // Clamp to [0.0, 2.0]: cosine similarity is in [-1, 1], so 1 - sim is
        // in [0, 2].  The clamp catches any floating-point overshoot and
        // converts residual NaN to 1.0 (maximally distant) rather than
        // propagating it into the dendrogram.
        (1.0 - (dot / denom)).clamp(0.0, 2.0)
    }
}

/// Convert f32 samples ([-1.0, 1.0]) to i16 for pyannote-rs.
pub fn f32_to_i16(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|&s| (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16)
        .collect()
}

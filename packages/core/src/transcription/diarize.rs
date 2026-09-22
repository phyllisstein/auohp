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
//! [`cluster_embeddings`]'s doc comment for the full story.
//!
//! ## Model
//!
//! `wespeaker_en_voxceleb_ECAPA1024.onnx` --- ECAPA-TDNN, 1024-dim, trained on
//! VoxCeleb, from the official WeSpeaker HuggingFace org (the model this
//! project had already upgraded to before diarization was set aside; see
//! commit `8348c36`). Takes log-mel filterbank features (`knf-rs`, the same
//! kaldi-compatible fbank extractor `pyannote-rs` used) and produces an
//! L2-normalizable speaker embedding.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use kodama::{linkage, Method};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;

use super::segmentation::Segmenter;

/// Filename of the embedding model under `$MODELS_DIR`, as
/// `scripts/download-models.sh` writes it.
pub const EMBEDDING_MODEL_FILE: &str = "wespeaker_en_voxceleb_ECAPA1024.onnx";

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

/// Minimum samples wespeaker's fbank frontend needs. `knf-rs` uses a 25 ms
/// analysis window at 16 kHz (= 400 samples); anything shorter produces an
/// empty filterbank and a hard error deeper in `compute_fbank`.
const MIN_SAMPLES: usize = 400;

/// Extracts speaker embeddings from short audio segments via the wespeaker
/// ONNX model.
struct EmbeddingExtractor {
    session: Session,
}

impl EmbeddingExtractor {
    fn new(model_path: &Path) -> Result<Self> {
        // See `segmentation::Segmenter::new` for why these are `.map_err`
        // rather than `?`: `ort::Error<SessionBuilder>` isn't `Send + Sync`.
        let session = Session::builder()
            .map_err(|e| anyhow::anyhow!("failed to create session builder: {e}"))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow::anyhow!("failed to set optimization level: {e}"))?
            .with_intra_threads(1)
            .map_err(|e| anyhow::anyhow!("failed to set intra-op threads: {e}"))?
            .commit_from_file(model_path)
            .with_context(|| format!("failed to load {}", model_path.display()))?;
        Ok(Self { session })
    }

    fn compute(&mut self, samples_i16: &[i16]) -> Result<Vec<f32>> {
        let mut samples_f32 = vec![0.0f32; samples_i16.len()];
        knf_rs::convert_integer_to_float_audio(samples_i16, &mut samples_f32);

        // `knf-rs` returns its features as an `ndarray::Array2` from *its*
        // ndarray version (0.16), which is not the same crate instance as
        // the one `ort` rc.13 integrates with (0.17) --- Cargo happily links
        // both, but a value of one crate's `ArrayBase` doesn't implement the
        // other crate's conversion traits. Round-tripping through
        // `(shape, Vec<f32>)` sidesteps the mismatch entirely: `ort::Tensor`
        // accepts any `(shape, Vec<T>)` tuple without needing ndarray at all.
        let features = knf_rs::compute_fbank(&samples_f32)
            .map_err(|e| anyhow::anyhow!("fbank extraction failed: {e}"))?;
        let shape = features.shape().to_vec();
        let data = features.into_raw_vec_and_offset().0;
        let shape = [1i64, shape[0] as i64, shape[1] as i64]; // insert batch dim

        let input =
            ort::value::Tensor::from_array((shape, data)).context("failed to build feats tensor")?;
        let outputs = self
            .session
            .run(ort::inputs!["feats" => input])
            .context("embedding inference failed")?;
        let output = outputs
            .get("embs")
            .context("embedding model has no \"embs\" tensor")?;
        let (_, data) = output
            .try_extract_tensor::<f32>()
            .context("failed to extract embedding")?;

        Ok(data.to_vec())
    }
}

/// Run segmentation + embedding extraction, without clustering. Factored out
/// of [`diarize`] so validation tooling can inspect embeddings directly ---
/// e.g. checking whether same-speaker segments actually land closer together
/// than different-speaker ones is a much more direct diagnostic than reading
/// clustering output when diarization behaves unexpectedly on real audio.
pub fn extract_segment_embeddings(
    samples: &[f32],
    sample_rate: u32,
    segmentation_model: &Path,
    embedding_model: &Path,
) -> Result<Vec<SegmentEmbedding>> {
    let samples_i16 = f32_to_i16(samples);

    let mut segmenter = Segmenter::new(segmentation_model)?;
    let speech_segments = segmenter.segment(&samples_i16, sample_rate)?;

    if speech_segments.is_empty() {
        tracing::warn!("no speech segments detected");
        return Ok(Vec::new());
    }
    tracing::info!(segments = speech_segments.len(), "segmentation complete");

    let mut extractor = EmbeddingExtractor::new(embedding_model)?;

    let mut segment_embeddings: Vec<SegmentEmbedding> =
        Vec::with_capacity(speech_segments.len());
    let mut skipped_short = 0usize;
    let mut skipped_nonfinite = 0usize;

    for seg in &speech_segments {
        let start_idx = ((seg.start * sample_rate as f64) as usize).min(samples_i16.len());
        let end_idx = ((seg.end * sample_rate as f64) as usize).min(samples_i16.len());
        if end_idx <= start_idx {
            continue;
        }
        let seg_samples = &samples_i16[start_idx..end_idx];
        if seg_samples.len() < MIN_SAMPLES {
            skipped_short += 1;
            continue;
        }

        let embedding = extractor.compute(seg_samples)?;
        if embedding.iter().any(|x| !x.is_finite()) {
            skipped_nonfinite += 1;
            continue;
        }

        segment_embeddings.push(SegmentEmbedding {
            start: seg.start,
            end: seg.end,
            embedding,
        });
    }

    tracing::info!(
        raw_segments = speech_segments.len(),
        kept = segment_embeddings.len(),
        skipped_short,
        skipped_nonfinite,
        "diarization embeddings complete"
    );

    Ok(segment_embeddings)
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

/// Cluster speaker embeddings using hierarchical agglomerative clustering.
///
/// Returns a Vec of speaker IDs (0-indexed), one per input segment, with at
/// most `max_speakers` distinct IDs.
///
/// One deliberate change from the pre-drop version: linkage is
/// `Method::Complete`, not `Method::Average`. Validated against
/// `assets/026_original.mp4` (see `examples/diarize_validate.rs`),
/// `Average` collapsed the whole 24-minute interview into one dominant
/// cluster with a single outlier segment split off --- a textbook
/// average-linkage failure mode on imbalanced classes. This interview is
/// exactly that shape: Iris Long (interviewee) accounts for the large
/// majority of speaking time, Sarah Schulman (interviewer) for brief,
/// scattered turns. Average linkage measures a candidate merge by the mean
/// distance to *every* point in a cluster, so as the majority speaker's
/// cluster grows, its average distance to any given minority-speaker point
/// shrinks --- the big cluster keeps absorbing outliers one at a time
/// instead of the minority speaker's segments coalescing into their own
/// cluster first. Complete linkage measures a merge by the *worst-case* (max)
/// pairwise distance instead, so a single stray close pair can't drag two
/// otherwise well-separated clusters together --- it took the same 24-minute
/// clip from ~58% to ~71% dominant-speaker accuracy in that harness.
fn cluster_embeddings(segment_embeddings: &[SegmentEmbedding], max_speakers: usize) -> Vec<usize> {
    let n = segment_embeddings.len();
    if n == 1 {
        return vec![0];
    }

    // `kodama` takes a *condensed* distance matrix: the strict upper triangle
    // flattened row by row, since a distance matrix is symmetric with a zero
    // diagonal. n(n-1)/2 entries instead of n².
    let mut condensed: Vec<f64> = Vec::with_capacity(n * (n - 1) / 2);
    for i in 0..n - 1 {
        for j in i + 1..n {
            condensed.push(cosine_distance(
                &segment_embeddings[i].embedding,
                &segment_embeddings[j].embedding,
            ));
        }
    }

    let dendrogram = linkage(&mut condensed, n, Method::Complete);
    let steps = dendrogram.steps();
    let merges_to_make = n.saturating_sub(max_speakers);

    let mut parent: Vec<usize> = (0..2 * n).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }

    for (step_idx, step) in steps.iter().enumerate().take(merges_to_make) {
        let new_cluster = n + step_idx;
        let a = find(&mut parent, step.cluster1);
        let b = find(&mut parent, step.cluster2);
        parent[a] = new_cluster;
        parent[b] = new_cluster;
    }

    let roots: Vec<usize> = (0..n).map(|i| find(&mut parent, i)).collect();
    let mut label_map: HashMap<usize, usize> = HashMap::new();
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
pub fn dominant_speaker<'a>(
    start: f64,
    end: f64,
    diarized: &'a [DiarizedSegment],
) -> Option<&'a str> {
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

/// Cosine distance between two vectors: 1 - cos(a, b).
///
/// Public so validation tooling can score embeddings against the same metric
/// the clustering step uses, instead of keeping a drifting copy.
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f64 {
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
        (1.0 - (dot / denom)).clamp(0.0, 2.0)
    }
}

/// Convert f32 samples ([-1.0, 1.0]) to i16, the scale both ONNX models expect.
fn f32_to_i16(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|&s| (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16)
        .collect()
}

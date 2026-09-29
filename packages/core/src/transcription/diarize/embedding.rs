use std::path::Path;

use anyhow::{Context, Result};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;

use super::segmentation::Segmenter;
use super::SegmentEmbedding;

/// Filename of the embedding model under `$MODELS_DIR`, as
/// `scripts/download-models.sh` writes it.
pub const EMBEDDING_MODEL_FILE: &str = "wespeaker_en_voxceleb_ECAPA1024.onnx";

/// Minimum samples wespeaker's fbank frontend needs. `knf-rs` uses a 25 ms
/// analysis window at 16 kHz (= 400 samples); anything shorter produces an
/// empty filterbank and a hard error deeper in `compute_fbank`.
const MIN_SAMPLES: usize = 400;

/// Extracts speaker embeddings from short audio segments via the wespeaker
/// ONNX model.
pub struct EmbeddingExtractor {
    session: Session,
}

impl EmbeddingExtractor {
    pub fn new(model_path: &Path) -> Result<Self> {
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

    pub fn compute(&mut self, samples: &[f32]) -> Result<Vec<f32>> {
        // `knf-rs` returns its features as an `ndarray::Array2` from *its*
        // ndarray version (0.16), which is not the same crate instance as
        // the one `ort` rc.13 integrates with (0.17) --- Cargo happily links
        // both, but a value of one crate's `ArrayBase` doesn't implement the
        // other crate's conversion traits. Round-tripping through
        // `(shape, Vec<f32>)` sidesteps the mismatch entirely: `ort::Tensor`
        // accepts any `(shape, Vec<T>)` tuple without needing ndarray at all.
        let features = knf_rs::compute_fbank(samples)
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
    let mut segmenter = Segmenter::new(segmentation_model)?;
    let speech_segments = segmenter.segment(samples, sample_rate)?;

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
        let start_idx = ((seg.start * sample_rate as f64) as usize).min(samples.len());
        let end_idx = ((seg.end * sample_rate as f64) as usize).min(samples.len());
        if end_idx <= start_idx {
            continue;
        }
        let seg_samples = &samples[start_idx..end_idx];
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

//! Native ort-based speaker segmentation (pyannote-segmentation-3.0, ONNX).
//!
//! ## Model
//!
//! `pyannote-segmentation-3.0.onnx`. It classifies non-overlapping 270-sample
//! frames (~16.9 ms at 16 kHz) within a 10 s sliding window as silence (class 0)
//! or speech (any other class). Downloaded by `scripts/download-models.py`.

use std::path::Path;

use anyhow::{Context, Result};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;

/// Filename of the segmentation model under `$MODELS_DIR`, as
/// `scripts/download-models.py` writes it.
pub const MODEL_FILE: &str = "pyannote-segmentation-3.0.onnx";

/// Samples between consecutive output frames for this ONNX export; re-derive
/// from the model's frame rate if the file changes.
const FRAME_STRIDE_SAMPLES: usize = 270;

/// Sample offset of the first output frame within a 10 s window
/// (the model's receptive-field padding).
const FIRST_FRAME_OFFSET_SAMPLES: usize = 721;

/// Window length the model expects, in seconds.
const WINDOW_SECS: usize = 10;

/// A speech region detected by the segmentation model, in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeechSegment {
    pub start: f64,
    pub end: f64,
}

/// Owns the segmentation ONNX session, reused across windows so graph parsing
/// and optimization are paid once per file.
pub struct Segmenter {
    session: Session,
}

impl Segmenter {
    pub fn new(model_path: &Path) -> Result<Self> {
        // `ort::Error<SessionBuilder>` isn't `Send + Sync` (via its custom-operator
        // registration slot), so `?` cannot convert it into `anyhow::Error`;
        // `.map_err` goes through `Display` instead.
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

    /// Run segmentation over f32 audio samples in [-1.0, 1.0], returning every
    /// detected speech region in chronological order.
    pub fn segment(&mut self, samples: &[f32], sample_rate: u32) -> Result<Vec<SpeechSegment>> {
        let window_size = sample_rate as usize * WINDOW_SECS;
        if window_size == 0 {
            anyhow::bail!("sample_rate must be > 0");
        }

        // Zero-pad so the final window is full-length, matching the model's
        // fixed input shape.
        let pad_len = (window_size - (samples.len() % window_size)) % window_size;
        let mut padded = Vec::with_capacity(samples.len() + pad_len);
        padded.extend_from_slice(samples);
        padded.resize(padded.len() + pad_len, 0.0f32);

        let mut segments = Vec::new();
        let mut is_speeching = false;
        let mut speech_start_sample = 0usize;
        // The first classification is a frame after the model's receptive-field
        // padding, not at sample 0.
        let mut frame_offset = FIRST_FRAME_OFFSET_SAMPLES;

        for window_start in (0..padded.len()).step_by(window_size) {
            let window_end = (window_start + window_size).min(padded.len());
            let window = &padded[window_start..window_end];

            // Shape [1, 1, samples]: batch=1, channels=1, raw waveform.
            let input =
                ort::value::Tensor::from_array(([1i64, 1i64, window.len() as i64], window.to_vec()))
                    .context("failed to build segmentation input tensor")?;
            let outputs = self
                .session
                .run(ort::inputs!["input" => input])
                .context("segmentation inference failed")?;
            let output = outputs
                .get("output")
                .context("segmentation model has no \"output\" tensor")?;
            let (shape, data) = output
                .try_extract_tensor::<f32>()
                .context("failed to extract segmentation output")?;

            // Shape is (batch, frames, classes) with batch=1. Checked rather than
            // assumed: a re-exported model would otherwise surface as an
            // out-of-bounds panic several frames deep.
            anyhow::ensure!(
                shape.len() == 3,
                "segmentation output has rank {}, expected 3 (batch, frames, classes)",
                shape.len()
            );
            let n_frames = shape[1] as usize;
            let n_classes = shape[2] as usize;

            for frame in 0..n_frames {
                let row = &data[frame * n_classes..(frame + 1) * n_classes];
                let (class, _) = row
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .expect("row is never empty --- n_classes > 0 by model contract");
                let is_speech_frame = class != 0;

                if is_speech_frame && !is_speeching {
                    speech_start_sample = frame_offset;
                    is_speeching = true;
                } else if !is_speech_frame && is_speeching {
                    segments.push(SpeechSegment {
                        start: speech_start_sample as f64 / sample_rate as f64,
                        end: frame_offset as f64 / sample_rate as f64,
                    });
                    is_speeching = false;
                }
                frame_offset += FRAME_STRIDE_SAMPLES;
            }
        }

        // Flush a run still open at end of recording, closing it at the true
        // sample count rather than the zero-padded length.
        if is_speeching {
            segments.push(SpeechSegment {
                start: speech_start_sample as f64 / sample_rate as f64,
                end: (samples.len().max(speech_start_sample)) as f64 / sample_rate as f64,
            });
        }

        Ok(segments)
    }
}

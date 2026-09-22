//! Native ort-based speaker segmentation (pyannote-segmentation-3.0, ONNX).
//!
//! This restores the segmentation half of AUOHP's diarization pipeline, but
//! deliberately not by depending on the `pyannote-rs` crate. `pyannote-rs`
//! 0.3.4's `get_segments` has two bugs, discovered and diagnosed the last time
//! diarization lived in this codebase (see
//! `git show ecd4471:packages/api/docs/native-segmentation-design.md` for the
//! full writeup that predates this restoration):
//!
//!   1. It is built on `std::iter::from_fn`, returning `None` the instant a
//!      single 10 s window produces zero speech/silence transitions. Standard
//!      iterator consumers (`.collect()`, `for ... in`) treat `None` as
//!      end-of-stream and stop --- so the first window of continuous speech
//!      silently truncates the rest of the recording. On real interview audio
//!      (long uninterrupted answers, not quick back-and-forth) this drops the
//!      majority of the clip.
//!   2. It never flushes a trailing in-progress speech run: if the window
//!      iterator is exhausted while `is_speeching` is still true, that final
//!      segment is lost.
//!
//! Neither bug was ever fixed upstream, and a from-scratch attempt to hit
//! `pyannote-rs` 0.3.4 against this workspace's `ort` version (`2.0.0-rc.13`,
//! pinned by `fastembed`) fails outright: `pyannote-rs`'s `eyre`-based error
//! handling can't convert `ort::Error<SessionBuilder>` because a type nested
//! in `ort`'s operator-registration machinery isn't `Sync`. Re-adding
//! `pyannote-rs` would either reintroduce both bugs (by pinning back to
//! `ort` rc.10, which conflicts with `fastembed` 6's rc.13 requirement) or
//! not compile at all. So: same ONNX model, same frame bookkeeping, our own
//! session and iteration logic, called directly through `ort` --- the crate
//! this project already uses for embeddings, per the "many sharp tools"
//! convention of quarantining each inference task behind a thin adapter
//! rather than depending on someone else's higher-level wrapper.
//!
//! ## Model
//!
//! `pyannote-segmentation-3.0.onnx`, the ONNX export published alongside the
//! `pyannote-rs` v0.1.0 release. It classifies non-overlapping 270-sample
//! frames (~16.9 ms at 16 kHz) within a 10 s sliding window as silence (class
//! 0) or speech (any other class). Downloaded by `scripts/download-models.sh`.

use std::path::Path;

use anyhow::{Context, Result};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;

/// Filename of the segmentation model under `$MODELS_DIR`, as
/// `scripts/download-models.sh` writes it.
pub const MODEL_FILE: &str = "pyannote-segmentation-3.0.onnx";

/// Number of samples between consecutive output frames. Empirically tuned to
/// this specific ONNX export --- see the design doc cited above. If the model
/// file ever changes, these must be re-derived from its published frame rate.
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

/// Owns the segmentation ONNX session. Reused across windows --- `ort`
/// session construction parses the graph and runs optimization passes, so
/// paying that cost once per file (not once per window, as the old chunked
/// workaround did) matters for a multi-hour interview.
pub struct Segmenter {
    session: Session,
}

impl Segmenter {
    pub fn new(model_path: &Path) -> Result<Self> {
        // `ort::Error<SessionBuilder>` embeds the builder itself as context,
        // which (through its custom-operator registration slot) isn't
        // `Send + Sync` --- so it can never satisfy `anyhow::Error`'s `From`
        // bound via `?`. `.map_err` sidesteps that by converting through
        // `Display` instead of relying on the blanket conversion. This is
        // the same trait-bound wall that makes `pyannote-rs` uncompilable
        // against this workspace's `ort` version (see this module's doc
        // comment) --- the difference is we control every call site here.
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

    /// Run segmentation over 16-bit PCM samples, returning every detected
    /// speech region in chronological order.
    ///
    /// Unlike `pyannote_rs::get_segments`, this runs to completion eagerly
    /// and returns a `Vec` rather than a lazy iterator --- every caller in
    /// this codebase collects the whole result anyway, and the eager API
    /// can't be silently truncated by a consumer that treats a momentary
    /// empty batch as end-of-stream.
    pub fn segment(&mut self, samples_i16: &[i16], sample_rate: u32) -> Result<Vec<SpeechSegment>> {
        let window_size = sample_rate as usize * WINDOW_SECS;
        if window_size == 0 {
            anyhow::bail!("sample_rate must be > 0");
        }

        // Zero-pad so the final window is full-length, matching the model's
        // fixed input shape.
        let pad_len = (window_size - (samples_i16.len() % window_size)) % window_size;
        let mut padded = Vec::with_capacity(samples_i16.len() + pad_len);
        padded.extend_from_slice(samples_i16);
        padded.resize(padded.len() + pad_len, 0);

        let mut segments = Vec::new();
        let mut is_speeching = false;
        let mut speech_start_sample = 0usize;
        // Anchored at FIRST_FRAME_OFFSET_SAMPLES: the model's first
        // classification corresponds to a frame that doesn't start at
        // sample 0, it starts after the receptive-field padding.
        let mut frame_offset = FIRST_FRAME_OFFSET_SAMPLES;

        for window_start in (0..padded.len()).step_by(window_size) {
            let window_end = (window_start + window_size).min(padded.len());
            let window = &padded[window_start..window_end];

            let window_f32: Vec<f32> = window.iter().map(|&s| s as f32).collect();
            // Shape [1, 1, samples]: batch=1, channels=1, raw waveform.
            let input =
                ort::value::Tensor::from_array(([1i64, 1i64, window_f32.len() as i64], window_f32))
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

            // Shape is (batch, frames, classes). We only ever run batch=1.
            // Checked rather than assumed: the model file is downloaded from
            // a release URL, so a re-pointed or re-exported model is the
            // realistic failure, and a bare index would surface it as an
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

        // Flush: the recording can end mid-speech. `pyannote_rs::get_segments`
        // drops this final run entirely; we close it out at the true sample
        // count (not the zero-padded length, so we don't report speech into
        // the padding).
        if is_speeching {
            segments.push(SpeechSegment {
                start: speech_start_sample as f64 / sample_rate as f64,
                end: (samples_i16.len().max(speech_start_sample)) as f64 / sample_rate as f64,
            });
        }

        Ok(segments)
    }
}

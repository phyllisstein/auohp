//! Model loading, lifecycle management, and DTW hardware configuration.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use whisper_rs::{
    DtwMode, DtwModelPreset, DtwParameters, WhisperContext, WhisperContextParameters, WhisperState,
};

/// Filename of the Whisper ggml model under `$MODELS_DIR`, as
/// `scripts/download-models.sh` writes it.
pub const MODEL_FILE: &str = "ggml-large-v3.bin";

/// Dedicated memory arena size allocated for Dynamic Time Warping (128 MB).
const DTW_MEM_SIZE: usize = 1024 * 1024 * 128;

/// Loaded Whisper model, ready for repeated inference calls.
pub struct WhisperModel {
    pub(crate) ctx: WhisperContext,
    /// Path to the silero VAD ggml model, stored here so `transcribe` can
    /// reference it without an extra parameter on every call.
    pub(crate) vad_model_path: PathBuf,
}

impl WhisperModel {
    /// Load the Whisper ggml model from `model_path`.
    ///
    /// Both files must already exist---use `scripts/download-models.sh` to
    /// fetch them. The pipeline resolves paths from `$MODELS_DIR` before
    /// calling this function. `vad_model_path` is stored in the returned
    /// `WhisperModel` and referenced on every `transcribe` call.
    pub fn load(model_path: &Path, vad_model_path: &Path) -> Result<Self> {
        tracing::debug!("Whisper: loading model from {}", model_path.display());

        // WhisperContextParameters::default() already sets use_gpu based on whether
        // the `metal` / `cuda` feature compiled in, so this call is belt-and-
        // suspenders---but it's explicit and costs nothing.
        let mut ctx_params = WhisperContextParameters::default();
        ctx_params.use_gpu(true);

        ctx_params.dtw_parameters(DtwParameters {
            mode: DtwMode::ModelPreset {
                model_preset: DtwModelPreset::LargeV3,
            },
            dtw_mem_size: DTW_MEM_SIZE,
        });

        // new_with_params accepts any P: AsRef<Path>.
        let ctx = WhisperContext::new_with_params(model_path, ctx_params)
            .context("failed to load Whisper model")?;

        tracing::debug!("Whisper: model loaded");
        Ok(Self {
            ctx,
            vad_model_path: vad_model_path.to_path_buf(),
        })
    }

    /// Path to the silero VAD model paired with this Whisper model.
    pub fn vad_model_path(&self) -> &Path {
        &self.vad_model_path
    }

    /// Allocate a fresh inference state from the context.
    pub(crate) fn create_state(&self) -> Result<WhisperState> {
        self.ctx
            .create_state()
            .context("failed to create Whisper state")
    }

    /// Return the end-of-text special token id, partitioning regular tokens from control tokens.
    pub(crate) fn token_eot(&self) -> i32 {
        self.ctx.token_eot()
    }
}

/// Load the Whisper ggml model from `model_path`.
///
/// Convenience facade function preserving the original `whisper::load_model` signature.
pub fn load_model(model_path: &Path, vad_model_path: &Path) -> Result<WhisperModel> {
    WhisperModel::load(model_path, vad_model_path)
}

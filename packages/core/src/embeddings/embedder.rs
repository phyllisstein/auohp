//! On-device sentence embeddings via fastembed (ONNX).
//!
//! Wraps nomic-embed-text-v1.5 (768-dim) using fastembed's
//! `UserDefinedEmbeddingModel` API, which loads the ONNX weights and
//! tokenizer files directly from disk rather than relying on fastembed's
//! HuggingFace Hub auto-download.  The five required files are
//! pre-downloaded by `scripts/download-models.sh` into
//! `$MODELS_DIR/nomic-embed-text-v1.5/` (default `/opt/auohp/models`),
//! so no network access occurs at inference time.
//!
//! Public API surface:
//!   - `Embedder`        --- owns and drives the ONNX session directly.
//!   - `EmbedderHandle`  --- async handle backed by a background worker thread;
//!                           use this in request handlers so ONNX inference never
//!                           blocks the async executor.
//!   - `EmbedResult`     --- type alias for the return type of `embed()`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use fastembed::{InitOptionsUserDefined, TextEmbedding, TokenizerFiles, UserDefinedEmbeddingModel};

const DEFAULT_MODELS_DIR: &str = "/opt/auohp/models";
/// Subdirectory within MODELS_DIR holding the five nomic model files.
const NOMIC_MODEL_DIR: &str = "nomic-embed-text-v1.5";

/// Drives the ONNX embedding session directly.
///
/// `TextEmbedding::embed` takes `&mut self` because the ONNX session mutates
/// internal state across calls, so `Embedder::embed` must also take `&mut self`.
/// You cannot call it from two threads at once. For concurrent access use
/// `EmbedderHandle`, which serializes requests through a dedicated blocking
/// thread so the async executor is never stalled.
pub struct Embedder {
    model: TextEmbedding,
    dimensions: usize,
}

impl Embedder {
    /// Load nomic-embed-text-v1.5 (768-dim) from pre-downloaded files.
    ///
    /// Reads five files from `$MODELS_DIR/nomic-embed-text-v1.5/`:
    ///   - `model.onnx`               --- ONNX weights (≈275 MB)
    ///   - `tokenizer.json`           ---
    ///   - `tokenizer_config.json`    --- tokenizer config files
    ///   - `config.json`              ---
    ///   - `special_tokens_map.json`  ---
    ///
    /// All five are fetched by `scripts/download-models.sh`.
    pub fn new() -> Result<Self> {
        let model_dir = PathBuf::from(
            std::env::var("MODELS_DIR").unwrap_or_else(|_| DEFAULT_MODELS_DIR.to_string()),
        )
        .join(NOMIC_MODEL_DIR);

        let read = |name: &str| -> Result<Vec<u8>> {
            std::fs::read(model_dir.join(name))
                .with_context(|| format!("failed to read {}/{}", NOMIC_MODEL_DIR, name))
        };

        let onnx_file = read("model.onnx")?;
        let tokenizer_files = TokenizerFiles {
            tokenizer_file: read("tokenizer.json")?,
            config_file: read("config.json")?,
            special_tokens_map_file: read("special_tokens_map.json")?,
            tokenizer_config_file: read("tokenizer_config.json")?,
        };

        let model = TextEmbedding::try_new_from_user_defined(
            UserDefinedEmbeddingModel::new(onnx_file, tokenizer_files),
            InitOptionsUserDefined::default(),
        )
        .context("failed to initialise embedding model")?;

        Ok(Self {
            model,
            dimensions: 768,
        })
    }

    /// Embed a batch of texts. Returns one `Vec<f32>` per input string.
    ///
    /// Takes `&mut self` because the ONNX session is stateful. Callers that
    /// need to share the embedder across async tasks should go through
    /// `EmbedderHandle` rather than wrapping this in a `Mutex` themselves.
    pub fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        self.model.embed(texts, None).context("embedding failed")
    }

    /// The dimensionality of the embedding vectors (768 for nomic-embed-text-v1.5).
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }
}

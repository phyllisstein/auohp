//! On-device sentence embeddings via fastembed (ONNX).
//!
//! Wraps Qwen3-Embedding-0.6B using fastembed's `UserDefinedEmbeddingModel`
//! API, which loads the ONNX weights and tokenizer files directly from disk
//! rather than relying on fastembed's HuggingFace Hub auto-download.  The
//! required files are pre-downloaded by `scripts/download-models.sh` into
//! `$MODELS_DIR` (default `/opt/auohp/models`), so no network access occurs at
//! inference time.
//!
//! Public API surface:
//!   - `Embedder`        --- owns and drives the ONNX session directly.
//!   - `EmbedderHandle`  --- async handle backed by a background worker thread;
//!                           use this in request handlers so ONNX inference never
//!                           blocks the async executor.
//!   - `EmbedResult`     --- type alias for the return type of `embed()`.

use crate::models;
use anyhow::{Context, Result};
use fastembed::{InitOptionsUserDefined, TextEmbedding, TokenizerFiles, UserDefinedEmbeddingModel};

const QWEN_MODEL_DIR: &str = "Qwen3-Embedding-0.6B";

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
    /// Load Qwen3-Embedding-0.6B (1024-dim) from pre-downloaded files.
    pub fn new() -> Result<Self> {
        let model_dir = models::models_dir();
        tracing::info!("loading embedding model... {:?}", &model_dir);
        let qwen_dir = model_dir.join(QWEN_MODEL_DIR);
        tracing::info!("model path files: {:?}", &qwen_dir);

        let read = |name: &str| -> Result<Vec<u8>> {
            std::fs::read(qwen_dir.join(name))
                .with_context(|| format!("failed to read {}/{}", QWEN_MODEL_DIR, name))
        };

        let onnx_file = read("model.onnx")?;
        let tokenizer_files = TokenizerFiles {
            tokenizer_file: read("tokenizer.json")?,
            config_file: read("config.json")?,
            special_tokens_map_file: read("special_tokens_map.json")?,
            tokenizer_config_file: read("tokenizer_config.json")?,
        };

        let model_definition = UserDefinedEmbeddingModel::new(onnx_file, tokenizer_files);
        let model_definition = model_definition
            .with_external_initializer("model.onnx_data".into(), read("model.onnx_data")?);
        let model = TextEmbedding::try_new_from_user_defined(
            model_definition,
            InitOptionsUserDefined::default(),
        )
        .context("failed to initialise embedding model")?;

        Ok(Self {
            model,
            dimensions: 1024,
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

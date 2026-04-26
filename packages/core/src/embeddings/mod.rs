//! On-device sentence embeddings via fastembed (ONNX).
//!
//! Wraps the nomic-embed-text-v1.5 model (768-dim) for generating vector
//! embeddings of transcript text. The ONNX weights are auto-downloaded
//! from Hugging Face Hub on first use and cached locally.
//!
//! Public API surface:
//!   - `Embedder`        --- owns and drives the ONNX session directly.
//!   - `EmbedderHandle`  --- async handle backed by a background worker thread;
//!                           use this in request handlers so ONNX inference never
//!                           blocks the async executor.
//!   - `EmbedResult`     --- type alias for the return type of `embed()`.

pub mod worker;

pub use worker::{EmbedResult, EmbedderHandle};

use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

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
    /// Load the nomic-embed-text-v1.5 model (768-dim).
    ///
    /// Nomic's v1.5 is a significant upgrade from BGE-small: it supports
    /// 8192-token context (vs. 512), produces 768-dim vectors (vs. 384),
    /// and scores meaningfully higher on MTEB retrieval benchmarks. The
    /// larger vectors and context window improve search relevance for
    /// oral-history transcripts, which tend to be long, conversational
    /// passages.
    pub fn new() -> Result<Self> {
        let model = TextEmbedding::try_new(
            TextInitOptions::new(EmbeddingModel::NomicEmbedTextV15)
                .with_show_download_progress(true),
        )
        .context("failed to load embedding model")?;

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

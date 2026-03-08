//! On-device sentence embeddings via fastembed (ONNX).
//!
//! Wraps the BGE-small-en-v1.5 model (384-dim) for generating vector
//! embeddings of transcript text. The ONNX weights are auto-downloaded
//! from Hugging Face Hub on first use and cached locally.

use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use std::sync::Mutex;

/// Shared embedding model handle.
///
/// `TextEmbedding::embed` takes `&mut self` (the ONNX session is not
/// thread-safe), so we wrap it in a `Mutex`. Embedding is CPU-bound —
/// callers should use `tokio::task::spawn_blocking` and concurrent
/// requests simply queue on the lock.
pub struct Embedder {
    model: Mutex<TextEmbedding>,
    dimensions: usize,
}

impl Embedder {
    /// Load the BGE-small-en-v1.5 model (384-dim).
    pub fn new() -> Result<Self> {
        let model = TextEmbedding::try_new(
            TextInitOptions::new(EmbeddingModel::BGESmallENV15)
                .with_show_download_progress(true),
        )
        .context("failed to load embedding model")?;

        Ok(Self {
            model: Mutex::new(model),
            dimensions: 384,
        })
    }

    /// Embed a batch of texts. Returns one `Vec<f32>` per input string.
    pub fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        // Mutex::lock() returns a Result because the lock can be "poisoned":
        // if a thread panics while holding the lock, Rust marks the Mutex as
        // poisoned to signal that the protected data might be in an
        // inconsistent state.
        //
        // The previous code used .unwrap(), which would panic (and crash the
        // entire server) on a poisoned lock. Instead we use
        // .unwrap_or_else(|e| e.into_inner()) to recover — the ONNX session
        // state is actually fine to reuse after a panic since fastembed
        // doesn't do partial mutation, so we just clear the poison flag and
        // carry on.
        let mut model = self.model.lock().unwrap_or_else(|e| e.into_inner());
        model.embed(texts, None).context("embedding failed")
    }

    /// The dimensionality of the embedding vectors (384 for BGE-small).
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }
}

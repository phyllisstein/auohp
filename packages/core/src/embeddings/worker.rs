//! Background embedding worker.
//!
//! `EmbedderHandle` owns an `Embedder` in a dedicated blocking thread and
//! exposes an async `embed()` method. Multiple callers share one handle;
//! their requests are serialized through an `mpsc` channel so the ONNX
//! session is never accessed from more than one thread at a time---without
//! a `Mutex`.

use anyhow::anyhow;
use tokio::sync::{mpsc, oneshot};

use super::Embedder;

/// The return type of `EmbedderHandle::embed`. One `Vec<f32>` per input text.
pub type EmbedResult = anyhow::Result<Vec<Vec<f32>>>;

/// A single unit of work sent to the embedding worker.
///
/// Not `pub`: only this module and the worker loop need to construct one.
/// `reply` is the "return address" --- the caller keeps the `Receiver` half
/// and the worker fires the result back through the `Sender` half.
struct EmbedRequest {
    texts: Vec<String>,
    reply: oneshot::Sender<EmbedResult>,
}

/// An async handle to the background embedding worker.
///
/// `Clone`-able because `mpsc::Sender` is `Clone`: each clone is another
/// sender pointing at the same worker, so you can hand copies to wherever
/// they're needed without any `Arc` wrapping.
#[derive(Clone)]
pub struct EmbedderHandle {
    tx: mpsc::Sender<EmbedRequest>,
}

impl EmbedderHandle {
    /// Spawn the background worker, taking ownership of `embedder`.
    ///
    /// The `Embedder` is moved into a dedicated OS thread via
    /// `spawn_blocking`. It lives there for the lifetime of the handle ---
    /// nothing outside this module can touch it. When the last
    /// `EmbedderHandle` is dropped, the `mpsc::Sender` is dropped, the
    /// channel closes, `blocking_recv()` returns `None`, and the thread
    /// exits cleanly.
    pub fn new(mut embedder: Embedder) -> Self {
        // Channel capacity: small enough to apply backpressure if the worker
        // falls behind, large enough for normal bursty usage.
        let (tx, mut rx) = mpsc::channel::<EmbedRequest>(32);

        // `spawn_blocking` gives us a dedicated OS thread that is allowed to
        // block. The async executor's worker threads must never block, but
        // ONNX inference is synchronous and CPU-bound, so it has to live here.
        //
        // `blocking_recv()` is the sync counterpart to `.recv().await`: it
        // parks the OS thread until a message arrives. We can't use `.await`
        // here because we're inside a sync closure, not an async block.
        tokio::task::spawn_blocking(move || {
            while let Some(req) = rx.blocking_recv() {
                let result = embedder.embed(&req.texts);
                // `send()` fails only if the caller dropped the Receiver
                // (gave up waiting). That's benign --- discard the result.
                let _ = req.reply.send(result);
            }
        });

        Self { tx }
    }

    /// Embed a batch of texts asynchronously.
    ///
    /// Sends the request to the worker thread and waits for the result.
    /// Returns an error if the worker has stopped or if ONNX inference fails.
    pub async fn embed(&self, texts: Vec<String>) -> EmbedResult {
        // TODO(human): implement this method.
        //
        // Steps:
        //   1. Call `oneshot::channel()` to create a (Sender, Receiver) pair.
        //   2. Build an `EmbedRequest { texts, reply: <sender> }` and send it
        //      through `self.tx`. If the channel is closed (worker died), the
        //      send returns Err --- map it to an `anyhow!` error and use `?`.
        //   3. Await the receiver. `reply_rx.await` returns
        //      `Result<EmbedResult, RecvError>`. Map the `RecvError` to
        //      `anyhow!` the same way, then use `?` to unwrap the outer
        //      Result, leaving the inner `EmbedResult` as the return value.
        todo!()
    }
}

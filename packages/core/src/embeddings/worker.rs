//! Background embedding worker with priority-aware scheduling.
//!
//! `EmbedderHandle` owns an `Embedder` in a dedicated blocking thread and
//! exposes two async submission methods: `embed()` (priority --- search
//! traffic) and `embed_background()` (background --- bulk seeding). The
//! worker drains the priority queue first, so a search request waits at
//! most one in-flight inference instead of an entire seed's worth.
//!
//! Both queues feed a single ONNX session pinned to one thread, so there's
//! no `Mutex` --- ownership is the synchronization primitive.

use anyhow::anyhow;
use tokio::sync::{mpsc, oneshot};

use super::embedder::Embedder;

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

/// Run one request to completion: embed, then ship the result back through
/// its oneshot. Pulled out of the loop so the loop body is purely about
/// *which* request to take next, not what to do with one.
fn handle_request(embedder: &mut Embedder, req: EmbedRequest) {
    let result = embedder.embed(&req.texts);
    // `send()` fails only if the caller dropped the Receiver (gave up
    // waiting). That's benign --- discard.
    let _ = req.reply.send(result);
}

/// An async handle to the background embedding worker.
///
/// `Clone`-able because `mpsc::Sender` is `Clone`: each clone is another
/// sender pointing at the same worker. Both senders are cloned together
/// when the handle is cloned.
#[derive(Clone)]
pub struct EmbedderHandle {
    /// Search traffic. Drained first.
    priority_tx: mpsc::Sender<EmbedRequest>,
    /// Bulk seeding. Drained only when priority is empty.
    background_tx: mpsc::Sender<EmbedRequest>,
}

impl EmbedderHandle {
    /// Spawn the background worker, taking ownership of `embedder`.
    ///
    /// The `Embedder` is moved into a dedicated OS thread via
    /// `spawn_blocking`. It lives there for the lifetime of the handle ---
    /// nothing outside this module can touch it. When *both* senders are
    /// dropped (i.e. the last `EmbedderHandle` is gone), both channels
    /// close, the worker's select sees `None` on both arms, and the thread
    /// exits cleanly.
    ///
    /// Capacity choices:
    ///   - priority: 16. Enough to absorb a small burst of concurrent
    ///     searches; large enough that `send().await` rarely parks under
    ///     normal load.
    ///   - background: 4. Deliberately tight. Seeding tasks `.await` on
    ///     `send`, so a small capacity means seeding self-throttles to the
    ///     worker's drain rate --- predictable memory, no unbounded queue.
    pub fn new(mut embedder: Embedder) -> Self {
        let (priority_tx, mut priority_rx) = mpsc::channel::<EmbedRequest>(16);
        let (background_tx, mut background_rx) = mpsc::channel::<EmbedRequest>(4);

        // We're called from inside the Tokio runtime (main is `#[tokio::main]`).
        // Capture a Handle here so the blocking thread --- which has no
        // implicit runtime context of its own --- can still drive a small
        // `async { select! { ... } }` block via `handle.block_on(...)`.
        let handle = tokio::runtime::Handle::current();

        // `spawn_blocking` gives us a dedicated OS thread that is allowed to
        // block. The async executor's worker threads must never block, but
        // ONNX inference is synchronous and CPU-bound, so it has to live here.
        tokio::task::spawn_blocking(move || {
            loop {
                // TODO(human): priority-aware request selection.
                //
                // Drain `priority_rx` first (use `try_recv` for the fast path
                // when something's already waiting). If nothing is ready,
                // park until either channel produces a request, preferring
                // priority --- `tokio::select!` with `biased;` is the tool.
                //
                // When *both* channels are closed (each `.recv()` resolves to
                // `None`), there's nothing left to do: break out of the loop
                // so the thread can exit.
                //
                // Use `handle.block_on(async { ... })` to drive the select
                // from this sync context, and `handle_request(&mut embedder,
                // req)` to actuall y run the work.
                //
                // ~10 lines.

                let rec = priority_rx.try_recv().map_err(|e| tracing::error!("{e}"));

                todo!("priority-aware select loop")
            }
        });

        Self {
            priority_tx,
            background_tx,
        }
    }

    /// Embed a batch of texts on the **priority** queue. Use this for
    /// interactive traffic (search). Worst-case wait is roughly one
    /// in-flight inference unit.
    pub async fn embed(&self, texts: Vec<String>) -> EmbedResult {
        send_and_wait(&self.priority_tx, texts).await
    }

    /// Embed a batch of texts on the **background** queue. Use this for
    /// bulk work (seeding) where latency doesn't matter but throughput
    /// does. Yields to priority traffic between requests.
    pub async fn embed_background(&self, texts: Vec<String>) -> EmbedResult {
        send_and_wait(&self.background_tx, texts).await
    }
}

/// Shared submit-and-await for both queues. Builds a oneshot, sends the
/// request, awaits the reply.
async fn send_and_wait(tx: &mpsc::Sender<EmbedRequest>, texts: Vec<String>) -> EmbedResult {
    let (reply_tx, reply_rx) = oneshot::channel();
    let request = EmbedRequest {
        texts,
        reply: reply_tx,
    };
    tx.send(request).await.map_err(|e| anyhow!("{e}"))?;
    reply_rx.await.map_err(|e| anyhow!("{e}"))?
}

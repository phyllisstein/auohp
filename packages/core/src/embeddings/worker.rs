//! Background embedding worker with priority-aware scheduling.
//!
//! `EmbedderHandle` owns an `Embedder` in a dedicated blocking thread and
//! exposes two async submission methods: `embed()` (priority --- search
//! traffic) and `embed_background()` (background --- bulk seeding).
//!
//! Two mechanisms keep search responsive while a whole interview is being
//! indexed:
//!   1. The worker drains the priority queue before the background queue, so
//!      a queued search jumps ahead of any pending seed work.
//!   2. Background requests are processed *cooperatively*: the worker slices
//!      a bulk request into small sub-batches and services waiting priority
//!      requests between slices. Without this, one whole-interview
//!      `fastembed` call would pin the thread and search would time out
//!      behind it --- priority queueing alone can't preempt a single
//!      monolithic inference.
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

/// How many texts the worker embeds in one uninterruptible `fastembed` call
/// when servicing a *background* request. This is the preemption granularity:
/// a search request that arrives mid-seed waits at most one sub-batch's
/// inference before it's serviced. Smaller = snappier search, more per-batch
/// overhead; larger = better seeding throughput, longer worst-case search
/// wait. Turn this knob if search latency under load is still too high.
const BACKGROUND_SUB_BATCH: usize = 8;

/// Run one request to completion: embed, then ship the result back through
/// its oneshot. Pulled out of the loop so the loop body is purely about
/// *which* request to take next, not what to do with one.
fn handle_request(embedder: &mut Embedder, req: EmbedRequest) {
    let result = embedder.embed(&req.texts);
    // `send()` fails only if the caller dropped the Receiver (gave up
    // waiting). That's benign --- discard.
    let _ = req.reply.send(result);
}

/// Run one *background* request cooperatively: slice its texts into
/// `BACKGROUND_SUB_BATCH`-sized chunks and, between chunks, fully service any
/// waiting priority (search) requests. The caller still sees a single reply
/// containing every vector in input order --- the slicing is invisible to them.
///
/// This is what makes a whole-interview seed preemptible: without it, the one
/// `embedder.embed(&all_texts)` call pins the worker thread for the entire
/// interview and search requests time out behind it.
fn handle_background_request(
    embedder: &mut Embedder,
    priority_rx: &mut mpsc::Receiver<EmbedRequest>,
    req: EmbedRequest,
) {
    // Destructure up front: `texts` and `reply` are now independent locals, so
    // the loop can borrow `texts` (via `.chunks()`) while still being free to
    // move `reply` into a `send()` --- no partial-move-of-`req` borrow clash.
    let EmbedRequest { texts, reply } = req;

    // Accumulates one Vec<f32> per input text, in order. Pre-size to avoid
    // reallocations as we extend it sub-batch by sub-batch.
    let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(texts.len());

    for slice in texts.chunks(BACKGROUND_SUB_BATCH) {
        // Drain priority *before* committing the thread to this slice: any
        // search that queued during the previous slice's inference gets
        // serviced now, so it waits at most one sub-batch. `try_recv` is
        // non-blocking --- it empties whatever's currently queued and then
        // returns `Err(Empty)`, ending the loop, rather than parking (which we
        // can't do here: this is a synchronous context, no `.await`).
        while let Ok(p) = priority_rx.try_recv() {
            handle_request(embedder, p);
        }

        match embedder.embed(slice) {
            // Extend in slice order so `vectors[i]` stays aligned with
            // `texts[i]` --- the caller zips these back against `uids`.
            Ok(v) => vectors.extend(v),
            // A failed slice dooms the whole request: send the error and bail
            // rather than replying with a partial result. `return` also means
            // the fall-through `reply.send` below never runs --- which matters,
            // since sending here has already moved `reply`.
            Err(e) => {
                let _ = reply.send(Err(e));
                return;
            }
        }
    }

    // One reply for the whole request: all vectors, in input order.
    let _ = reply.send(Ok(vectors));
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
        tokio::task::spawn_blocking(move || loop {
            // Which queue did the next request come from? The loop needs to
            // know: priority requests run straight through, but background
            // requests run *cooperatively* (yielding to priority between
            // sub-batches), so they dispatch to a different handler.
            enum Work {
                Priority(EmbedRequest),
                Background(EmbedRequest),
            }

            // Fast path: if a priority request is already sitting in the
            // channel, take it without paying for a `block_on`/`select!` at
            // all. This is the common case under load: search traffic
            // arriving faster than the worker can drain it.
            let work = if let Ok(req) = priority_rx.try_recv() {
                Some(Work::Priority(req))
            } else {
                // Nothing queued on priority right now --- park until
                // *either* channel has something. `biased` turns off
                // `select!`'s default (fair, randomized) arm choice: when
                // both arms are ready in the same poll, the first-listed
                // arm always wins, so priority preempts background here.
                //
                // `.map(...)` on the `Option<EmbedRequest>` each arm yields
                // tags the origin without disturbing the `None`-means-closed
                // signal we match on below.
                handle.block_on(async {
                    tokio::select! {
                        biased;
                        req = priority_rx.recv() => req.map(Work::Priority),
                        req = background_rx.recv() => req.map(Work::Background),
                    }
                })
            };

            match work {
                Some(Work::Priority(req)) => handle_request(&mut embedder, req),
                Some(Work::Background(req)) => {
                    handle_background_request(&mut embedder, &mut priority_rx, req)
                }
                // `recv()` only returns `None` once a channel is both
                // closed and drained. Because both senders live inside one
                // `EmbedderHandle` and are dropped together, they close
                // together too, so seeing `None` here means there is
                // nothing left on either side.
                None => break,
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

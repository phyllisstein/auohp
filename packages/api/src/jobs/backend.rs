//! The storage seam.
//!
//! [`QueueBackend`] is the trait a durable implementation would later satisfy.
//! [`InProcessBackend`] is the only implementation today: a tokio mpsc channel
//! and a pool of tasks draining it.
//!
//! The trait is narrow on purpose. Loco's `QueueProvider` has fourteen methods,
//! most of which exist to serve its CLI (`dump`, `import`, `cancel_jobs`,
//! `clear_by_status`, `clear_jobs_older_than`, `retry_failed`, `requeue`,
//! `ping`, `describe`). Every one of those is a method a new backend must
//! implement or stub, and none of them is reachable from application code. We
//! take four --- enqueue, start, shutdown, describe --- which is the minimum
//! that supports the actual lifecycle, and let an operational surface grow
//! later against a real requirement.

use std::sync::Arc;

use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use crate::jobs::handle::{JobId, StateStore};
use crate::jobs::registry::Registry;
use crate::jobs::worker::JobContext;

/// One unit of queued work in its erased, on-the-wire form.
///
/// `name` routes it to a handler; `args` is the payload. Note that this is
/// already the shape a durable backend would persist --- a row of (id, name,
/// json) --- which is not an accident. Designing the in-process envelope to be
/// serializable-shaped is what keeps the durable backend a drop-in rather than
/// a rewrite, even though nothing serializes it today.
#[derive(Clone, Debug)]
pub struct Envelope {
    pub id: JobId,
    pub name: String,
    pub args: JsonValue,
}

/// Storage and execution strategy for queued jobs.
///
/// `async fn` in trait would make this non-object-safe, and we need
/// `Arc<dyn QueueBackend>` for the `Queue` newtype to work. So the one
/// genuinely async method (`enqueue`) is written in its desugared form: a
/// method returning a boxed future. This is what `#[async_trait]` would
/// generate; writing it by hand costs one line of noise and saves a proc-macro
/// dependency plus its compile time.
///
/// In practice `enqueue` on the in-process backend never actually awaits --- it
/// uses the synchronous `try_send`. The async signature is kept because a
/// durable backend's enqueue *is* an I/O round trip, and retrofitting async
/// onto this method later would break every call site. Paying for the future
/// now is what makes the later swap invisible.
pub trait QueueBackend: Send + Sync {
    /// Accept a job for execution. Returns once the job is durably accepted ---
    /// for the in-process backend, that means "in the channel".
    fn enqueue(
        &self,
        envelope: Envelope,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + '_>>;

    /// Start processing. Called once, after every worker is registered.
    ///
    /// Takes `&self` and spawns internally rather than consuming `self`,
    /// because the same `Arc<dyn QueueBackend>` is simultaneously held by the
    /// GraphQL schema for enqueueing.
    fn start(&self, ctx: JobContext, registry: Arc<Registry>) -> anyhow::Result<()>;

    /// Stop accepting work and wait for in-flight jobs to finish.
    ///
    /// See [`InProcessBackend::shutdown`] for what "wait" costs here.
    fn shutdown(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + '_>>;

    /// Short human-readable description, for the startup log line.
    fn describe(&self) -> String;
}

/// In-process queue: an mpsc channel plus N draining tasks.
///
/// The whole backend is a bounded `tokio::sync::mpsc` channel. Bounded, not
/// unbounded, and that choice carries the module's most important operational
/// property: backpressure. An unbounded channel converts a queue overload into
/// unbounded memory growth and eventually an OOM kill, which takes the HTTP
/// server with it. A bounded one converts the same overload into a visible
/// `Err` on enqueue, which a GraphQL mutation can turn into a "server busy"
/// response. Given that each job here can hold a whole interview's transcript
/// in its args, that ceiling matters.
pub struct InProcessBackend {
    /// Producer side, cloned into the schema via the `Queue` handle.
    tx: mpsc::Sender<Envelope>,

    /// Consumer side, taken exactly once by `start`.
    ///
    /// `Mutex<Option<T>>` is the idiom for "this value moves out, once, through
    /// a shared reference". `start` takes `&self` (it must --- the backend is
    /// behind an `Arc`), but the receiver must be *moved* into the pool task
    /// since an mpsc receiver is not clonable. `Option::take` through a mutex
    /// is what turns a shared borrow into a one-time move, and the `None` left
    /// behind is what makes a second `start` detectably wrong instead of
    /// silently spawning a second pool.
    rx: std::sync::Mutex<Option<mpsc::Receiver<Envelope>>>,

    /// Cancellation signal, cloned into every worker task.
    ///
    /// A `CancellationToken` rather than a broadcast channel or an `AtomicBool`
    /// because it is simultaneously awaitable (usable as a `select!` branch, so
    /// an idle worker wakes immediately) and pollable (`is_cancelled`, for a
    /// cheap check between jobs). An `AtomicBool` gives only the second, which
    /// means an idle worker would not notice shutdown until its next poll tick.
    token: CancellationToken,

    /// Tracks spawned worker tasks so shutdown can await all of them.
    ///
    /// `TaskTracker` over a `Vec<JoinHandle>` because it can be awaited through
    /// a shared reference, which is what `shutdown(&self)` has. Collecting
    /// handles into a `Vec` would need the same `Mutex<Option<..>>` dance as
    /// the receiver, for no gain.
    tracker: TaskTracker,

    /// Number of concurrent draining tasks.
    concurrency: usize,

    /// Channel capacity, retained for `describe`.
    capacity: usize,

    /// Where worker tasks publish job state.
    state: Arc<dyn StateStore>,
}

impl InProcessBackend {
    /// Build a backend with `concurrency` workers and a `capacity`-slot channel.
    ///
    /// On choosing `concurrency` for this workload, see the spike doc: the
    /// answer is *not* "number of cores". Both real jobs here funnel into
    /// `EmbedderHandle`, which owns a single ONNX thread, so raising this does
    /// not buy embedding parallelism --- it only buys the ability to run an
    /// unrelated job while one is embedding. Two is a reasonable default.
    pub fn new(
        concurrency: usize,
        capacity: usize,
        state: Arc<dyn StateStore>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(capacity);

        Self {
            tx,
            rx: std::sync::Mutex::new(Some(rx)),
            token: CancellationToken::new(),
            tracker: TaskTracker::new(),
            concurrency,
            capacity,
            state,
        }
    }

    /// The loop each worker task runs.
    ///
    /// Written as a free async function taking owned arguments rather than a
    /// method, because it is spawned: a `tokio::spawn`ed future must be
    /// `'static`, and a method borrowing `&self` is not. Owning its inputs is
    /// what satisfies that, and it makes the task's full dependency set
    /// legible in the signature.
    async fn run_worker(
        index: usize,
        rx: Arc<tokio::sync::Mutex<mpsc::Receiver<Envelope>>>,
        ctx: JobContext,
        registry: Arc<Registry>,
        state: Arc<dyn StateStore>,
        token: CancellationToken,
    ) {
        tracing::debug!(worker = index, "job worker started");

        loop {
            // Take the next envelope, or stop.
            //
            // The receiver is shared behind an async mutex so that N workers
            // can share one channel --- mpsc is multi-producer, single-consumer,
            // so the consumer end is the thing that needs guarding. The lock is
            // held only across `recv`, never across the job body, so workers
            // contend for microseconds and then run concurrently for minutes.
            //
            // A `tokio::sync::Mutex` (not `std`) is required precisely because
            // the guard is held across an `.await`: a std guard is not `Send`,
            // so holding one across a yield point would make the whole task
            // non-`Send` and refuse to spawn.
            let envelope = {
                let mut guard = rx.lock().await;

                tokio::select! {
                    // `biased` polls branches top-down instead of randomly, so
                    // cancellation always wins a tie against an available job.
                    // Without it, a saturated queue could keep feeding this
                    // worker jobs indefinitely after shutdown was requested.
                    biased;

                    () = token.cancelled() => {
                        tracing::debug!(worker = index, "cancellation received, stopping");
                        break;
                    }

                    maybe = guard.recv() => match maybe {
                        Some(envelope) => envelope,
                        // Every sender dropped and the buffer is drained. This
                        // is the clean end-of-stream, distinct from
                        // cancellation: it means no more work can ever arrive.
                        None => {
                            tracing::debug!(worker = index, "queue closed, stopping");
                            break;
                        }
                    },
                }
            };

            let Envelope { id, name, args } = envelope;

            let Some(handler) = registry.get(&name) else {
                // Unroutable. `Queue::enqueue` rejects unknown names up front,
                // so reaching this means the registry and the enqueue path
                // disagree --- worth an error, but not worth stopping for.
                tracing::error!(job_id = %id, job = %name, "no handler registered for job");
                state.set_failed(&id, format!("no handler registered for {name:?}"));
                continue;
            };

            tracing::info!(job_id = %id, job = %name, worker = index, "job started");
            state.set_progress(&id, 0.0, "starting");

            // The handler already converts panics into `Err` (see
            // `registry::erase`), so this loop only has to deal with a Result.
            match handler(ctx.clone(), id.clone(), args).await {
                Ok(()) => {
                    tracing::info!(job_id = %id, job = %name, "job completed");
                    state.set_completed(&id);
                }
                Err(error) => {
                    // `{error:#}` renders anyhow's full context chain rather
                    // than just the outermost message.
                    tracing::error!(job_id = %id, job = %name, error = %error, "job failed");
                    state.set_failed(&id, format!("{error:#}"));
                }
            }
        }

        tracing::debug!(worker = index, "job worker stopped");
    }
}

impl QueueBackend for InProcessBackend {
    fn enqueue(
        &self,
        envelope: Envelope,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + '_>> {
        Box::pin(async move {
            let id = envelope.id.clone();
            let name = envelope.name.clone();

            // `try_send`, not `send`. `send` would await a free slot, which
            // turns queue saturation into a stalled HTTP request holding a
            // connection open --- the caller waits instead of being told no.
            // Failing fast lets the resolver return a real error while the
            // request is still cheap to abandon.
            self.tx.try_send(envelope).map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => anyhow::anyhow!(
                    "job queue is full ({} slots); refusing job {name:?}",
                    self.capacity
                ),
                mpsc::error::TrySendError::Closed(_) => {
                    anyhow::anyhow!("job queue is shut down; refusing job {name:?}")
                }
            })?;

            // Mark queued only after the send succeeds, so a rejected job never
            // leaves a phantom entry in the state store.
            self.state.set_queued(&id);
            tracing::debug!(job_id = %id, job = %name, "job enqueued");

            Ok(())
        })
    }

    fn start(&self, ctx: JobContext, registry: Arc<Registry>) -> anyhow::Result<()> {
        let rx = self
            .rx
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .ok_or_else(|| anyhow::anyhow!("job queue has already been started"))?;

        // Re-wrap the single receiver so all N workers can share it.
        let rx = Arc::new(tokio::sync::Mutex::new(rx));

        for index in 0..self.concurrency {
            // `TaskTracker::spawn` both spawns and registers the task, so
            // `wait()` later covers every worker with no bookkeeping.
            self.tracker.spawn(Self::run_worker(
                index,
                Arc::clone(&rx),
                ctx.clone(),
                Arc::clone(&registry),
                Arc::clone(&self.state),
                self.token.clone(),
            ));
        }

        // Closing the tracker means "no further tasks will be added". A
        // `TaskTracker` that is never closed makes `wait()` hang forever, since
        // it cannot know more work is not coming --- this line is load-bearing
        // for shutdown, not tidiness.
        self.tracker.close();

        tracing::info!(
            workers = self.concurrency,
            capacity = self.capacity,
            registered = registry.len(),
            "background job pool started"
        );

        Ok(())
    }

    fn shutdown(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + '_>> {
        Box::pin(async move {
            tracing::info!("shutting down background job pool");

            // Deliberate ordering, and the semantics here are the honest cost
            // of an in-process queue.
            //
            // Cancelling the token stops workers at their *next* dequeue. It
            // does not interrupt a job already running --- there is no safe way
            // to interrupt a `spawn_blocking` closure mid-inference --- so
            // in-flight jobs run to completion and we wait for them.
            //
            // Jobs still sitting in the channel are *dropped*. They are not
            // persisted anywhere, so a restart does not recover them. That is
            // the durability tradeoff stated plainly; the mitigation (making
            // every job idempotent and re-derivable from graph state) is
            // discussed in the spike doc.
            self.token.cancel();

            self.tracker.wait().await;

            tracing::info!("background job pool stopped");

            Ok(())
        })
    }

    fn describe(&self) -> String {
        format!(
            "in-process queue ({} workers, {} slots, non-durable)",
            self.concurrency, self.capacity
        )
    }
}

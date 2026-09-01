//! Type erasure: turning many differently-typed [`Worker`]s into one uniform
//! callable the worker loop can invoke without knowing anything about them.
//!
//! This is the single most interesting piece of machinery in the module, and
//! it is worth reading slowly, because it is the trick that makes a
//! heterogeneous job system possible at all.
//!
//! The problem: `TranscribeWorker` performs `TranscribeArgs`, `EmbedWorker`
//! performs `EmbedArgs`. These are different types, so `Vec<Worker<_>>` is not
//! a thing you can write --- the trait is generic, and a generic trait is not
//! one type but a family of them. Yet the worker loop must hold all of them in
//! one collection and dispatch by a runtime string.
//!
//! The solution: wrap each typed worker in a closure that accepts the *erased*
//! form of its argument (`serde_json::Value`) and does the downcast itself, at
//! the boundary, where the concrete type is still statically known. Once
//! wrapped, every worker has the identical signature `(JobId, Value) -> Future`
//! and they become storable together. The generic parameter is consumed at
//! wrap time; it does not survive into the collection.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use futures_util::FutureExt;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::jobs::handle::JobId;
use crate::jobs::worker::{JobContext, Worker};

/// A type-erased job body.
///
/// Reading this type outside-in: a heap-allocated closure, callable many times
/// from many threads (`Fn + Send + Sync`), which returns a heap-allocated
/// future that is itself `Send`.
///
/// `Pin<Box<dyn Future>>` is mandatory rather than stylistic. `dyn Future` is
/// unsized, so it must be boxed to be returned. And it must be *pinned* because
/// a future produced by an `async` block is typically self-referential --- it
/// holds borrows across await points that point into its own storage --- so
/// moving it after polling begins would dangle those. `Pin` is the type-level
/// promise that the move will not happen. This is the same shape `async_trait`
/// generates; we write it out once, by hand, in exactly the one place it is
/// unavoidable, instead of taking the macro across the whole trait.
pub type JobHandler = Arc<
    dyn Fn(JobContext, JobId, JsonValue) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>
        + Send
        + Sync,
>;

/// Wrap one typed worker into a [`JobHandler`].
///
/// This function is where `Args` disappears. Inside the closure the compiler
/// still knows the concrete type --- that is what lets `from_value::<Args>`
/// monomorphize --- but the closure's *signature* mentions only `JsonValue`. So
/// the type parameter is real at compile time and absent at runtime: erasure
/// without any dynamic typing.
///
/// The worker is moved into an `Arc` because the returned closure is `Fn`, not
/// `FnOnce`: it may be invoked concurrently by several worker tasks for several
/// jobs at once. Each invocation clones the `Arc` (a refcount bump) so the
/// spawned future owns a handle that outlives the call.
pub fn erase<Args, W>(worker: W) -> JobHandler
where
    Args: Serialize + for<'de> Deserialize<'de> + Send + 'static,
    W: Worker<Args>,
{
    let worker = Arc::new(worker);

    Arc::new(move |ctx: JobContext, job_id: JobId, data: JsonValue| {
        let worker = Arc::clone(&worker);

        Box::pin(async move {
            // Deserialize at the boundary. A malformed payload is a job
            // failure, not a queue failure --- one bad enqueue must not take
            // down the worker loop, so this becomes an `Err` like any other.
            let args: Args = serde_json::from_value(data)
                .map_err(|e| anyhow::anyhow!("failed to deserialize job arguments: {e}"))?;

            // Catch panics at the job boundary.
            //
            // Without this, a `panic!` or an `unwrap` on `None` inside a job
            // body unwinds through the worker task. Tokio would contain it to
            // that one task, but the task *is* the worker loop --- so the pool
            // would silently lose a worker on every panicking job, degrading to
            // zero throughput with nothing in the logs to say why. Converting
            // the panic to an `Err` keeps the loop alive and the job's failure
            // visible in the state store.
            //
            // `AssertUnwindSafe` is required because the future closes over
            // `&JobContext` and friends, which the compiler cannot prove remain
            // logically consistent if unwound through. The assertion is sound
            // here: on panic we abandon the job entirely and touch none of its
            // partial state, so no half-updated invariant escapes. Loco makes
            // the same call at the same boundary.
            let outcome = std::panic::AssertUnwindSafe(worker.perform(&ctx, &job_id, args))
                .catch_unwind()
                .await;

            match outcome {
                Ok(result) => result,
                Err(panic) => {
                    // A panic payload is `Box<dyn Any>`; the common cases are a
                    // `String` (from a formatted `panic!`) or a `&'static str`
                    // (from a literal one). Try both before giving up.
                    let message = panic
                        .downcast_ref::<String>()
                        .map(String::as_str)
                        .or_else(|| panic.downcast_ref::<&str>().copied())
                        .unwrap_or("job panicked with a non-string payload");

                    tracing::error!(job_id = %job_id, panic = message, "job panicked");

                    Err(anyhow::anyhow!("job panicked: {message}"))
                }
            }
        })
    })
}

/// Name-to-handler map, built once at startup and then read-only.
///
/// Loco keeps its registry behind a `tokio::sync::Mutex` because workers may be
/// registered after the queue is running. We require registration to finish
/// before the pool starts, which means the map can be frozen into an `Arc` and
/// shared with zero locking on the hot path --- every dispatch is a plain
/// `HashMap` read. The cost is that dynamic registration is impossible; the
/// benefit is no lock in the dispatch loop and no "registered too late" failure
/// mode. For a fixed set of compiled-in job types, that is the right trade.
#[derive(Default)]
pub struct Registry {
    handlers: HashMap<String, JobHandler>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a typed worker under its `NAME`.
    ///
    /// Takes and returns `self` so registrations chain at the call site. A
    /// duplicate name is a programming error --- two workers silently sharing a
    /// name would make dispatch depend on registration order --- so it panics
    /// at startup rather than misrouting jobs at runtime.
    #[must_use]
    pub fn register<Args, W>(mut self, worker: W) -> Self
    where
        Args: Serialize + for<'de> Deserialize<'de> + Send + 'static,
        W: Worker<Args>,
    {
        let name = W::NAME.to_string();

        if self.handlers.contains_key(&name) {
            panic!("a worker is already registered under the name {name:?}");
        }

        tracing::debug!(worker = %name, "registering background worker");
        self.handlers.insert(name, erase(worker));

        self
    }

    /// Look up a handler by the name a job was enqueued under.
    pub fn get(&self, name: &str) -> Option<&JobHandler> {
        self.handlers.get(name)
    }

    /// Whether any worker is registered under `name`. Used by the queue to
    /// reject an unroutable job at enqueue time rather than at dequeue time.
    pub fn contains(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }

    /// How many workers are registered. Used in the startup log line.
    pub fn len(&self) -> usize {
        self.handlers.len()
    }
}

//! The typed half of the job system: [`Worker`] and the [`JobContext`] it runs
//! against.

use std::sync::Arc;

use auohp_core::EmbedderHandle;
use serde::{Deserialize, Serialize};

use crate::jobs::handle::{JobId, StateStore};
use crate::neo4j::Db;

/// Everything a running job is allowed to reach for.
///
/// This is the deliberate anti-`AppContext`. Loco hands workers the entire
/// framework context --- database, config, mailer, storage, cache, and the queue
/// itself --- which makes every worker's dependency surface unknowable from its
/// signature. We hand over exactly the three things a job in this codebase
/// needs, so adding a fourth is a visible, reviewable edit here.
///
/// Both fields are `Arc`-shaped and therefore cheap to clone per job. Note what
/// is *absent*: no `Queue`. A job cannot enqueue another job. That is a
/// restriction we can lift later by adding a field, but starting without it
/// keeps the dependency graph acyclic --- `Queue` holds a `Registry` holds
/// handlers that close over `JobContext`; putting a `Queue` back inside
/// `JobContext` closes that loop and forces an `Arc<Weak<..>>` dance.
#[derive(Clone)]
pub struct JobContext {
    /// Neo4j connection pool handle. Already `Arc<Graph>` --- see `neo4j::Db`.
    pub db: Db,

    /// The embedding worker handle. Note this is *not* an `Embedder`: it is a
    /// handle to a dedicated OS thread that owns the ONNX session, with its own
    /// priority/background queues. The job system must therefore never try to
    /// parallelize embedding by running N jobs at once --- see the spike doc's
    /// "two schedulers" section. Jobs submit to this handle and await; the
    /// serialization happens inside it.
    ///
    /// `Option` purely so tests can build a context without loading a real
    /// ONNX model from disk. In `main` this is always `Some`. A job that needs
    /// the embedder calls [`JobContext::embedder`], which turns the absence
    /// into an ordinary job failure rather than a panic --- so the testing
    /// affordance cannot silently become a production nil-deref.
    pub embedder: Option<Arc<EmbedderHandle>>,

    /// Where a job reports its own progress. Injected rather than global so
    /// tests can substitute an inert store.
    pub state: Arc<dyn StateStore>,
}

impl JobContext {
    /// The embedder, or a job-level error if this context has none.
    ///
    /// Returning `Result` rather than unwrapping is what keeps the `Option`
    /// above honest: a job that needs an embedder and finds none fails that one
    /// job with a clear message, instead of panicking a worker task.
    pub fn embedder(&self) -> anyhow::Result<&Arc<EmbedderHandle>> {
        self.embedder
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("this job requires an embedder, but none is configured"))
    }

    /// Convenience for a job body that wants to publish progress without
    /// threading `job_id` and `state` through every helper it calls.
    ///
    /// `fraction` is `0.0..=1.0`. We clamp rather than assert, since a job
    /// computing `done / total` hands over a NaN when `total` is zero, and a
    /// panic in a progress call would fail an otherwise-healthy job.
    ///
    /// `f32::clamp` panics if either bound is NaN, but not if *self* is NaN ---
    /// a NaN input propagates through and would land in the store. The explicit
    /// `is_finite` guard is what actually rejects it.
    pub fn report_progress(&self, job_id: &JobId, fraction: f32, message: &str) {
        let fraction = if fraction.is_finite() {
            fraction.clamp(0.0, 1.0)
        } else {
            0.0
        };

        self.state.set_progress(job_id, fraction, message);
    }
}

/// A typed unit of background work.
///
/// The generic parameter is on the *trait*, not the method, which is what lets
/// one type implement `Worker` for several argument types if it ever needs to.
/// More importantly it is what makes `Args` available to the erasing closure in
/// `registry::erase`, which must name the concrete type in order to call
/// `serde_json::from_value::<Args>`.
///
/// The bound soup on `Args` is doing real work, so it is worth unpacking:
///   - `Serialize` --- needed at *enqueue* time, to turn the caller's struct
///     into the `serde_json::Value` that crosses the erased boundary.
///   - `for<'de> Deserialize<'de>` --- a higher-ranked trait bound, needed at
///     *dequeue* time. It says "deserializable from a borrow of any lifetime",
///     which is the standard way to spell `DeserializeOwned` inline. It rules
///     out argument types that borrow from the input buffer, which is exactly
///     right: the buffer is a temporary inside the handler.
///   - `Send + 'static` --- the args cross a `tokio::spawn` boundary.
///
/// `async_trait` is not used here. Rust 2024 supports `async fn` in traits
/// natively; the cost is that the trait is no longer object-safe (the returned
/// future is an opaque per-impl type with no fixed size). That does not matter
/// because we never store a `dyn Worker` --- we store a `JobHandler`, which is
/// the erased form. Loco needed `async_trait` because it targets an older
/// edition; we get to skip the `Box::pin` on every call.
pub trait Worker<Args>: Send + Sync + 'static
where
    Args: Serialize + for<'de> Deserialize<'de> + Send + 'static,
{
    /// The stable name this worker is registered and enqueued under.
    ///
    /// Loco derives this from `std::any::type_name` via `heck`. That is clever
    /// and wrong for anything durable: renaming or moving the struct silently
    /// orphans every already-queued job of that name. We require an explicit
    /// constant so the wire name is a deliberate, greppable decision --- this
    /// costs one line per worker and removes a whole class of migration bug.
    const NAME: &'static str;

    /// Run the job.
    ///
    /// Returning `Err` marks the job failed. Implementations should treat this
    /// as "this attempt failed", not "this job is impossible" --- retry policy
    /// is the backend's business, not the worker's.
    fn perform(
        &self,
        ctx: &JobContext,
        job_id: &JobId,
        args: Args,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
}

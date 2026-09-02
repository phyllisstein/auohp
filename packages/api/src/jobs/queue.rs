//! The enqueue-side handle that GraphQL resolvers hold.
//!
//! # Why this type exists rather than passing `SqliteStorage` around
//!
//! `SqliteStorage<T, C, F>` is generic over its argument type, its codec, and
//! its fetcher. Putting it directly into async-graphql's `.data()` would mean
//! resolvers naming that whole three-parameter type, and it would mean a
//! second job kind forces a second `.data()` entry with a different concrete
//! type. [`JobQueue`] is a thin façade that hides those parameters and exposes
//! the two operations resolvers actually want: enqueue, and read status back.
//!
//! # Why it holds a pool and a config rather than a built storage
//!
//! The second half of that argument only works if the storage type stays out of
//! the façade as well, so [`JobQueue`] holds `{ pool, config }` and builds a
//! `SqliteStorage` view per call instead of keeping one as a field.
//!
//! The reason is that async-graphql's `.data()` is a `TypeId` map. Job kinds
//! are distinguished by the storage's first type parameter, so
//! `SqliteStorage<A>` and `SqliteStorage<B>` are *different keys*: a façade
//! that stored one would need one context entry per job kind, which is the
//! exact problem the previous paragraph says the façade exists to prevent.
//! `SqlitePool` and [`StorageConfig`] are not generic, so the context stays at
//! one entry no matter how many job kinds there are.
//!
//! Building per call is affordable because it is not really "building"
//! anything: `SqliteStorage::new_with_config` wraps a pool handle, and
//! `SqlitePool` is an `Arc` internally, so the cost is a refcount bump rather
//! than a connection. `open_pool`'s own documentation already describes this
//! arrangement --- "the enqueue side and the worker side each build their own
//! `SqliteStorage` view over this one pool" --- and the tests have always
//! worked this way (see `probe_storage` in `tests.rs`).
//!
//! Note what this does *not* do: [`enqueue_embed`](JobQueue::enqueue_embed) is
//! still concrete, and adding a job kind still means adding methods. What the
//! shape buys is that nothing structural stands in the way --- a generic
//! `enqueue<T>` becomes a possible later step rather than a blocked one,
//! because the storage type is now constructed where `T` is known instead of
//! being frozen into a field.
//!
//! # Task ids and a sharp edge in apalis-sqlite
//!
//! `Task::parts.task_id` is an `Option<TaskId<Ulid>>`, and `push_tasks` in
//! apalis-sqlite treats `None` as "generate one" ---
//! `.unwrap_or(Ulid::new().to_string())`. That generated id is written to the
//! row and then **dropped on the floor**: the push future resolves to `()`, so
//! a caller that lets apalis assign the id has no way to learn it.
//!
//! Since the whole point of the mutation change is to hand the client a
//! pollable handle, we assign the `TaskId` ourselves via `TaskBuilder` and
//! return the same value we stored. This is the kind of detail that only shows
//! up by reading the backend's source, and it is worth flagging as an
//! ergonomics gap rather than a bug.

use std::time::Duration;

use apalis::prelude::{TaskBuilder, TaskSink};
use apalis_sqlite::{CompactType, SqliteStorage, TaskBuilderExt};
use apalis_codec::json::JsonCodec;
use apalis_core::task::task_id::TaskId;
use sqlx::SqlitePool;
use ulid::Ulid;

use crate::jobs::embed::EmbedInterview;
use crate::jobs::status::{JobStatus, read_status};
use crate::jobs::storage::StorageConfig;

/// The concrete storage type for the embedding queue.
///
/// Spelled out once here so nothing else in the crate has to. `JsonCodec<Vec<u8>>`
/// is what `SqliteStorage::new` selects by default; `SqliteFetcher` is the
/// polling fetcher (as opposed to the update-hook-driven `HookCallbackListener`).
pub type EmbedStorage =
    SqliteStorage<EmbedInterview, JsonCodec<CompactType>, apalis_sqlite::fetcher::SqliteFetcher>;

/// How many times a failed embedding job is retried before it is left alone.
///
/// This is the *storage-level* attempt ceiling, written into the row's
/// `max_attempts` column. It is deliberately distinct from the Tower
/// `RetryPolicy` on the worker: see the note in `worker_for` below --- the two
/// operate at different layers and both matter.
pub const MAX_ATTEMPTS: u32 = 3;

/// Handle held by GraphQL resolvers, injected via async-graphql's `.data()`.
///
/// `Clone` is cheap: `SqlitePool` is internally an `Arc`, so cloning a
/// `JobQueue` clones a reference-counted pool handle, not a connection, and
/// `StorageConfig` is a `String` and a `Duration`.
#[derive(Clone)]
pub struct JobQueue {
    pool: SqlitePool,
    config: StorageConfig,
}

impl JobQueue {
    /// Build a queue handle over an already-migrated pool.
    ///
    /// Takes the config by reference and clones it, because callers
    /// (`main.rs`) keep their own copy to hand to the worker.
    pub fn new(pool: SqlitePool, config: &StorageConfig) -> Self {
        Self {
            pool,
            config: config.clone(),
        }
    }

    /// Enqueue an embedding job to run as soon as a worker is free.
    ///
    /// Returns the task id, which the client polls with the `jobStatus` query.
    pub async fn enqueue_embed(&self, interview_uid: &str) -> anyhow::Result<String> {
        self.enqueue_embed_after(interview_uid, Duration::ZERO)
            .await
    }

    /// Enqueue an embedding job that must not run until `delay` has elapsed.
    ///
    /// This is the scheduling capability the hand-rolled in-process queue could
    /// not express. It is not implemented with a timer or a sleeping task: the
    /// delay is written into the row's `run_at` column as an absolute unix
    /// timestamp, and every fetch query carries
    /// `AND (run_at IS NULL OR run_at <= strftime('%s','now'))`.
    ///
    /// That distinction is the whole point. A `tokio::time::sleep` before
    /// enqueueing lives in process memory and dies with the process; a future
    /// `run_at` is a fact in the database, so a job scheduled for an hour from
    /// now still fires even if the server restarts twice in between. Scheduling
    /// and durability are the same mechanism here, not two features.
    pub async fn enqueue_embed_after(
        &self,
        interview_uid: &str,
        delay: Duration,
    ) -> anyhow::Result<String> {
        // Assign the id up front so we can return it --- see the module docs.
        let task_id: TaskId<Ulid> = TaskId::new(Ulid::new());
        let id_string = task_id.to_string();

        let mut builder = TaskBuilder::new(EmbedInterview::new(interview_uid))
            .with_task_id(task_id)
            // Written to the row's `max_attempts`. The orphan-reclaim path and
            // the fetch query both consult it, so this survives restarts in a
            // way an in-memory retry counter would not.
            .max_attempts(MAX_ATTEMPTS);

        if !delay.is_zero() {
            builder = builder.run_after(delay);
        }

        let task = builder.build();

        // `push_task` comes from the `TaskSink` trait, which is blanket-implemented
        // for any `Backend` that is also a `futures::Sink`. That is a neat bit of
        // layering: the durable-write path is expressed as a Sink impl, and the
        // ergonomic `push`/`push_bulk`/`push_task` surface is derived from it
        // generically rather than reimplemented per backend.
        //
        // It takes `&mut self`, which is why the storage is built here as a
        // local rather than held as a field: a local is trivially `mut`, where a
        // field would need either `&mut self` on this method or a clone to
        // dodge it. Constructing it costs a refcount bump on the pool --- see
        // the module docs for why that, not thrift, is what makes this cheap.
        let mut storage: EmbedStorage = SqliteStorage::new_with_config(
            &self.pool,
            &self.config.to_apalis_config(EmbedInterview::QUEUE),
        );
        storage
            .push_task(task)
            .await
            .map_err(|e| anyhow::anyhow!("failed to enqueue embedding job: {e}"))?;

        tracing::info!(
            job_id = %id_string,
            interview_uid,
            delay_secs = delay.as_secs(),
            "embedding job enqueued"
        );

        Ok(id_string)
    }

    /// Read a task's current state back out of the queue.
    pub async fn status(&self, job_id: &str) -> anyhow::Result<Option<JobStatus>> {
        read_status(&self.pool, job_id).await
    }
}

/// Build the worker that drains the embedding queue.
///
/// # The middleware stack, and whether Tower is real leverage here
///
/// Everything below `.backend(...)` is a `tower::Layer`, stacked in the order
/// written, with later layers wrapping earlier ones. Two of the three are
/// literally Tower's own types re-exported --- `apalis::layers::limit` is
/// `pub use tower::limit::{...}` and `TimeoutLayer` is `pub use
/// tower::timeout::TimeoutLayer` --- and `retry` wraps `tower::retry::Policy`
/// with apalis-aware policies. That is the substance behind the feature table
/// reading `retry = ["tower/retry"]`.
///
/// The practical consequence for this project is that the queue's middleware
/// and the HTTP server's middleware are the same abstraction and the same
/// compiled crate: cargo resolves a single `tower 0.5.3` for both axum and
/// apalis, with no duplicate in the lockfile.
///
/// # Two retry mechanisms, deliberately
///
/// `.retry(RetryPolicy::retries(n))` retries *in process, immediately*, without
/// the task ever leaving the worker. The row's `max_attempts` retries *across*
/// process lifetimes, because a `Failed` row with attempts remaining is
/// re-selected by the fetch query.
///
/// They cover different failures. A transient Neo4j blip is best absorbed by
/// the in-process retry --- no database round trip, no re-fetch. A worker that
/// is killed mid-job can only be recovered by the durable path, because there
/// is no process left to hold a retry counter. Configuring both is not
/// redundancy; it is covering both halves.
///
/// # Concurrency
///
/// No `.concurrency(n)` layer, and that is a considered omission rather than an
/// oversight. Every embedding job funnels into `EmbedderHandle`, which owns a
/// single ONNX session on a single dedicated thread. Raising worker concurrency
/// would let more jobs sit `Running` simultaneously while they queue behind
/// that one thread --- more rows locked, more apparent parallelism, exactly the
/// same throughput. A Tower `ConcurrencyLimitLayer` would be measuring the
/// wrong resource.
/// # Return type
///
/// This returns an opaque `Future` rather than the `Worker` itself, and that
/// is a concession to how gnarly `Worker`'s type is: it carries five type
/// parameters, one of which is the fully-materialized middleware `Stack<...>`
/// produced by the builder chain. Naming it at a module boundary would mean
/// transcribing that stack --- and re-transcribing it every time a layer is
/// added or reordered.
///
/// `impl Future` erases all of it. The worker is constructed and immediately
/// converted into its run-future here, where the concrete types are still in
/// scope and inference can do the work. This is the same reason axum handlers
/// return `impl IntoResponse` rather than their real future types.
///
/// # Ownership
///
/// Takes the pool and config **by value**, not by reference. That is forced by
/// `tokio::spawn`, which requires a `'static` future: a returned
/// `impl Future` that borrowed its arguments would carry those lifetimes, and
/// the borrow checker rejects spawning it. Taking ownership up front moves the
/// borrow problem to the call site, where a `SqlitePool` clone is a refcount
/// bump and a `StorageConfig` clone is two small fields.
///
/// # Shutdown
///
/// The signal is bounded by `IntoFuture`, not `Future`, so callers can pass a
/// [`ShutdownSignal`](crate::jobs::ShutdownSignal) from
/// [`Workers`](crate::jobs::Workers) as-is. Everything awaitable is
/// `IntoFuture` --- every `Future` gets a blanket impl --- so this loosens the
/// signature without asking anything more of a caller who has a plain future.
pub fn run_worker<S>(
    pool: SqlitePool,
    config: StorageConfig,
    deps: crate::jobs::embed::EmbedDeps,
    shutdown: S,
) -> impl std::future::Future<Output = anyhow::Result<()>> + Send
where
    // `IntoFuture` rather than `Future` so callers may pass a `ShutdownSignal`
    // (or anything else awaitable) directly. `S::IntoFuture: Send` is the
    // bound that actually matters --- it is the type that ends up held across
    // an await inside the returned future, and `tokio::spawn` needs that
    // future to be `Send`. Naming `S` rather than writing `impl IntoFuture` in
    // argument position is forced: an argument-position `impl Trait` has no
    // name, so its associated types cannot be constrained in a where clause.
    S: std::future::IntoFuture<Output = ()> + Send + 'static,
    S::IntoFuture: Send,
{
    use apalis::layers::retry::RetryPolicy;
    use apalis::prelude::{WorkerBuilder, WorkerBuilderExt};

    // Same queue name as the enqueue side, derived from the same const rather
    // than repeated as a literal --- see `EmbedInterview::QUEUE`.
    let storage: EmbedStorage =
        SqliteStorage::new_with_config(&pool, &config.to_apalis_config(EmbedInterview::QUEUE));

    let worker = WorkerBuilder::new("auohp-embedder")
        .backend(storage)
        // Injected into the handler through the `Data<EmbedDeps>` extractor.
        .data(deps)
        // Panic containment. Without this a panicking task body unwinds
        // through the worker; with it the panic becomes an `AbortError`, which
        // `RetryPolicy` recognizes as terminal and does not retry. The
        // hand-rolled spike had to write this `catch_unwind` itself.
        .catch_panic()
        // In-process retries. Added before tracing so that tracing wraps it
        // and each attempt gets its own span, rather than all attempts
        // collapsing into a single one.
        .retry(RetryPolicy::retries(MAX_ATTEMPTS as usize))
        .enable_tracing()
        .build(crate::jobs::embed::embed_interview);

    async move {
        // `run_until` is what lets the worker participate in the same shutdown
        // path as the HTTP server: it drains until the signal future resolves,
        // then stops accepting new tasks. Tasks already in flight are allowed
        // to finish; tasks still `Pending` stay in the database, which is
        // exactly the property the in-process queue could not offer.
        //
        // The signal must resolve to `Result<(), Err>` where `Err: Into<WorkerError>`,
        // not to `()`. Rather than push that constraint onto every caller, we
        // adapt here: our callers hand us a plain `Future<Output = ()>` and
        // this wrapper supplies the `Ok`. `WorkerError` is named as the error
        // type only to satisfy inference --- the `Err` branch is unreachable.
        let signal = async move {
            // `.await` on a non-`Future` desugars through `IntoFuture`, so this
            // reads the same whether the caller handed us a `ShutdownSignal` or
            // a plain future.
            shutdown.await;
            Ok::<(), apalis_core::error::WorkerError>(())
        };

        worker
            .run_until(signal)
            .await
            .map_err(|e| anyhow::anyhow!("embedding worker failed: {e}"))
    }
}

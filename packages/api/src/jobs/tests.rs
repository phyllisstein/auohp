//! Tests for the apalis-backed queue.
//!
//! # What is and is not exercised here
//!
//! These tests drive the **real** `SqliteStorage`, the real migrations, the
//! real Tower middleware stack, and the real fetch/lock/ack SQL. What they do
//! not drive is Neo4j or the ONNX embedder, because neither is available in a
//! unit-test process. So the *job bodies* below are stand-ins whose observable
//! behaviour (fail twice then succeed; count invocations) is what the queue
//! semantics are asserted against.
//!
//! That split is deliberate and is the honest way to test a queue: the thing
//! under test is the delivery guarantee, not the payload. `embed_interview`'s
//! own logic is a straight-line read-embed-write with no branching worth a
//! mock harness.
//!
//! # Why every test uses a file, not `:memory:`
//!
//! `sqlite::memory:` gives each *connection* a private database. Since
//! `open_pool` pins to one connection that would technically work --- but a
//! durability test that never touches a filesystem is not testing durability.
//! The restart test in particular must survive the pool being dropped and
//! reopened, which only a file can do.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use apalis::prelude::{Data, TaskBuilder, TaskSink, WorkerContext};
use apalis_codec::json::JsonCodec;
use apalis_core::backend::codec::Codec;
use apalis_core::error::BoxDynError;
use apalis_core::task::status::Status;
use apalis_core::task::task_id::TaskId;
use apalis_sqlite::{CompactType, SqliteStorage, TaskBuilderExt};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ulid::Ulid;

use crate::jobs::status::read_status;
use crate::jobs::storage::{StorageConfig, open_pool};

/// A temporary directory that deletes itself on drop.
///
/// Hand-rolled rather than pulling in `tempfile`: the spike is measuring
/// dependency cost, so adding a dev-dependency to test a dependency-cost claim
/// would be a small act of self-sabotage.
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!("auohp-jobs-{tag}-{}", Ulid::new()));
        std::fs::create_dir_all(&path).expect("could not create temp dir");
        Self(path)
    }

    fn db_url(&self) -> String {
        format!("sqlite://{}", self.0.join("jobs.db").display())
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Test job arguments. Distinct from `EmbedInterview` so these tests exercise
/// the queue without dragging in Neo4j types.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Probe {
    label: String,
}

impl Probe {
    /// The probe's own queue, distinct from the real embedding queue.
    ///
    /// Naming it separately is the point: these tests and the production
    /// embedding queue can address the same database file without colliding,
    /// because every fetch query filters on `job_type`. This is the same
    /// partitioning `EmbedInterview::QUEUE` relies on, exercised here with a
    /// second job kind.
    const QUEUE: &str = "auohp-test-probe";
}

/// Storage view over the probe job type.
type ProbeStorage =
    SqliteStorage<Probe, JsonCodec<CompactType>, apalis_sqlite::fetcher::SqliteFetcher>;

/// Build a storage view over the probe queue.
///
/// Both the push side and the worker side of every test below go through this
/// one helper, so they cannot disagree about the queue name --- the same
/// property `EmbedInterview::QUEUE` gives the production code.
fn probe_storage(pool: &SqlitePool, config: &StorageConfig) -> ProbeStorage {
    SqliteStorage::new_with_config(pool, &config.to_apalis_config(Probe::QUEUE))
}

fn config_for(dir: &TempDir) -> StorageConfig {
    StorageConfig {
        url: dir.db_url(),
        orphan_reclaim_after: Duration::from_secs(1),
    }
}

/// Push a task with a caller-chosen id, returning that id.
async fn push_probe(
    storage: &mut ProbeStorage,
    label: &str,
    max_attempts: u32,
    delay: Duration,
) -> String {
    let task_id: TaskId<Ulid> = TaskId::new(Ulid::new());
    let id = task_id.to_string();

    let mut builder = TaskBuilder::new(Probe {
        label: label.to_string(),
    })
    .with_task_id(task_id)
    .max_attempts(max_attempts);

    if !delay.is_zero() {
        builder = builder.run_after(delay);
    }

    storage.push_task(builder.build()).await.expect("push failed");
    id
}

// ---------------------------------------------------------------------------
// Storage and durability
// ---------------------------------------------------------------------------

/// The database file is created on first run, with no install step.
#[tokio::test]
async fn creates_database_file_on_first_run() {
    let dir = TempDir::new("create");
    let config = config_for(&dir);

    let db_path = dir.0.join("jobs.db");
    assert!(!db_path.exists(), "precondition: file should not exist yet");

    let pool = open_pool(&config).await.expect("open_pool failed");

    assert!(
        db_path.exists(),
        "open_pool should have created the database file"
    );

    // Migrations ran: the Jobs table exists and is queryable.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM Jobs")
        .fetch_one(&pool)
        .await
        .expect("Jobs table should exist after setup");
    assert_eq!(count, 0);

    pool.close().await;
}

/// Running setup twice is safe --- the migration path is idempotent, which is
/// what lets `main` call it unconditionally on every boot.
#[tokio::test]
async fn setup_is_idempotent() {
    let dir = TempDir::new("idempotent");
    let config = config_for(&dir);

    let pool = open_pool(&config).await.expect("first open failed");
    pool.close().await;

    let pool = open_pool(&config).await.expect("second open failed");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM Jobs")
        .fetch_one(&pool)
        .await
        .expect("Jobs table should still exist");
    assert_eq!(count, 0);
    pool.close().await;
}

/// **The durability claim, demonstrated rather than asserted.**
///
/// Enqueue a job, drop the entire pool (simulating process death), reopen the
/// database from scratch, and confirm the job is still there and still
/// runnable. This is the property the in-process channel queue cannot have at
/// any price: its jobs live in process memory.
#[tokio::test]
async fn queued_jobs_survive_a_restart() {
    let dir = TempDir::new("restart");
    let config = config_for(&dir);

    // ── "First process" ──────────────────────────────────────────────────
    let id = {
        let pool = open_pool(&config).await.expect("open failed");
        let mut storage = probe_storage(&pool, &config);
        let id = push_probe(&mut storage, "survives", 3, Duration::ZERO).await;

        let status = read_status(&pool, &id)
            .await
            .expect("status read failed")
            .expect("job should exist");
        assert_eq!(status.status, Status::Pending);

        // Close the pool. Everything in memory about this queue is now gone.
        pool.close().await;
        id
    };

    // ── "Second process" ─────────────────────────────────────────────────
    let pool = open_pool(&config).await.expect("reopen failed");

    let status = read_status(&pool, &id)
        .await
        .expect("status read failed")
        .expect("job should have survived the restart");

    assert_eq!(
        status.status,
        Status::Pending,
        "a job queued before the restart should still be pending after it"
    );
    assert_eq!(status.attempts, 0);

    pool.close().await;
}

/// An unknown id reads as `None`, not an error.
#[tokio::test]
async fn unknown_job_id_reads_as_none() {
    let dir = TempDir::new("unknown");
    let config = config_for(&dir);
    let pool = open_pool(&config).await.expect("open failed");

    let status = read_status(&pool, &Ulid::new().to_string())
        .await
        .expect("read should succeed");
    assert!(status.is_none());

    pool.close().await;
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

/// A job pushed to the queue is actually picked up and run by a worker.
#[tokio::test]
async fn worker_executes_a_queued_job() {
    use apalis::prelude::{WorkerBuilder, WorkerBuilderExt};

    let dir = TempDir::new("execute");
    let config = config_for(&dir);
    let pool = open_pool(&config).await.expect("open failed");

    let mut storage = probe_storage(&pool, &config);
    let id = push_probe(&mut storage, "run-me", 3, Duration::ZERO).await;

    let ran = Arc::new(AtomicUsize::new(0));

    async fn handler(
        _task: Probe,
        counter: Data<Arc<AtomicUsize>>,
        worker: WorkerContext,
    ) -> Result<(), BoxDynError> {
        counter.fetch_add(1, Ordering::SeqCst);
        let _ = worker;
        Ok(())
    }

    let worker = WorkerBuilder::new("probe-worker")
        .backend(probe_storage(&pool, &config))
        .data(Arc::clone(&ran))
        .catch_panic()
        .build(handler);

    // A hard timeout so a hang fails the test instead of stalling CI.
    //
    // The result of `run()` is deliberately not unwrapped. `worker.stop()`
    // from inside a handler flips the worker's state, but the run-future does
    // not necessarily resolve promptly afterwards --- it may stay parked on
    // its poll interval. What this test asserts is the observable outcome (the
    // handler ran; the row reached Done), not how quickly the future settles.
    // Run the worker in the background and poll storage for the ack instead of
    // stopping the worker from inside its own handler.
    let handle = tokio::spawn(worker.run());

    let mut status = read_status(&pool, &id).await.unwrap().expect("job exists");
    for _ in 0..100 {
        if status.is_terminal() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        status = read_status(&pool, &id).await.unwrap().expect("job exists");
    }
    handle.abort();

    assert_eq!(ran.load(Ordering::SeqCst), 1, "handler should have run once");
    assert_eq!(status.status, Status::Done);
    assert!(status.is_terminal());

    pool.close().await;
}

/// **Retry, demonstrated.**
///
/// The handler fails its first two invocations and succeeds on the third. If
/// the retry layer were absent the job would be observed exactly once and the
/// counter would read 1; observing three invocations and a terminal `Done` is
/// the retry policy doing real work.
#[tokio::test]
async fn failed_jobs_are_retried_until_they_succeed() {
    use apalis::layers::retry::RetryPolicy;
    use apalis::prelude::{WorkerBuilder, WorkerBuilderExt};

    let dir = TempDir::new("retry");
    let config = config_for(&dir);
    let pool = open_pool(&config).await.expect("open failed");

    let mut storage = probe_storage(&pool, &config);
    let id = push_probe(&mut storage, "flaky", 5, Duration::ZERO).await;

    let attempts = Arc::new(AtomicUsize::new(0));

    async fn flaky(
        _task: Probe,
        counter: Data<Arc<AtomicUsize>>,
        worker: WorkerContext,
    ) -> Result<(), BoxDynError> {
        // `fetch_add` returns the *previous* value, so this is 0, 1, 2, ...
        let seen = counter.fetch_add(1, Ordering::SeqCst);
        if seen < 2 {
            return Err(format!("synthetic failure #{}", seen + 1).into());
        }
        worker.stop()?;
        Ok(())
    }

    let worker = WorkerBuilder::new("retry-worker")
        .backend(probe_storage(&pool, &config))
        .data(Arc::clone(&attempts))
        .catch_panic()
        .retry(RetryPolicy::retries(5))
        .enable_tracing()
        .build(flaky);

    tokio::time::timeout(Duration::from_secs(30), worker.run())
        .await
        .expect("worker timed out")
        .expect("worker errored");

    assert_eq!(
        attempts.load(Ordering::SeqCst),
        3,
        "handler should have been invoked twice unsuccessfully then once successfully"
    );

    let status = read_status(&pool, &id)
        .await
        .expect("status read failed")
        .expect("job should exist");
    assert_eq!(
        status.status,
        Status::Done,
        "the job should end Done, not Failed --- retries recovered it"
    );

    pool.close().await;
}

/// A job that fails every time stops at its attempt ceiling rather than
/// spinning forever, and lands in a terminal state.
#[tokio::test]
async fn permanently_failing_jobs_stop_at_the_attempt_ceiling() {
    use apalis::layers::retry::RetryPolicy;
    use apalis::prelude::{WorkerBuilder, WorkerBuilderExt};

    let dir = TempDir::new("giveup");
    let config = config_for(&dir);
    let pool = open_pool(&config).await.expect("open failed");

    let mut storage = probe_storage(&pool, &config);
    let id = push_probe(&mut storage, "doomed", 2, Duration::ZERO).await;

    let attempts = Arc::new(AtomicUsize::new(0));

    async fn always_fails(
        _task: Probe,
        counter: Data<Arc<AtomicUsize>>,
    ) -> Result<(), BoxDynError> {
        counter.fetch_add(1, Ordering::SeqCst);
        Err("this job never succeeds".into())
    }

    let worker = WorkerBuilder::new("giveup-worker")
        .backend(probe_storage(&pool, &config))
        .data(Arc::clone(&attempts))
        .catch_panic()
        .retry(RetryPolicy::retries(2))
        .build(always_fails);

    // No `worker.stop()` is reachable from a handler that always errors, so
    // this worker is bounded by the timeout rather than by completion. That is
    // the point: we want to observe where the job lands, then stop looking.
    let _ = tokio::time::timeout(Duration::from_secs(10), worker.run()).await;

    let status = read_status(&pool, &id)
        .await
        .expect("status read failed")
        .expect("job should exist");

    assert!(
        matches!(status.status, Status::Failed | Status::Killed),
        "a permanently failing job should end Failed or Killed, got {:?}",
        status.status
    );
    assert!(
        attempts.load(Ordering::SeqCst) >= 2,
        "the handler should have been retried at least to its ceiling"
    );

    pool.close().await;
}

// ---------------------------------------------------------------------------
// Scheduling
// ---------------------------------------------------------------------------

/// **Scheduling, demonstrated at the storage level.**
///
/// A delayed job's `run_at` is a future timestamp, and --- crucially --- it is
/// a *column*, not a timer. Asserting on the row rather than on elapsed
/// wall-clock is the stronger claim: it shows the delay is persisted state
/// that a restart preserves, not a sleeping task that a restart discards.
#[tokio::test]
async fn delayed_jobs_are_scheduled_into_the_future() {
    let dir = TempDir::new("schedule");
    let config = config_for(&dir);
    let pool = open_pool(&config).await.expect("open failed");

    let mut storage = probe_storage(&pool, &config);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let immediate = push_probe(&mut storage, "now", 3, Duration::ZERO).await;
    let delayed = push_probe(&mut storage, "later", 3, Duration::from_secs(3600)).await;

    let immediate_status = read_status(&pool, &immediate)
        .await
        .unwrap()
        .expect("immediate job should exist");
    let delayed_status = read_status(&pool, &delayed)
        .await
        .unwrap()
        .expect("delayed job should exist");

    assert!(
        immediate_status.run_at <= now + 2,
        "an undelayed job should be runnable now, run_at={} now={now}",
        immediate_status.run_at
    );
    assert!(
        delayed_status.run_at >= now + 3500,
        "a job delayed an hour should have run_at about an hour out, run_at={} now={now}",
        delayed_status.run_at
    );

    pool.close().await;
}

/// A scheduled job is genuinely *not* dispatched before it comes due, and *is*
/// dispatched once it does.
///
/// This is the behavioural half of the previous test: the row says the job is
/// deferred, and here the fetch query actually honours it. The job is delayed
/// by two seconds; a worker that ran it immediately would fail the first
/// assertion.
#[tokio::test]
async fn scheduled_jobs_do_not_run_before_they_are_due() {
    use apalis::prelude::{WorkerBuilder, WorkerBuilderExt};

    let dir = TempDir::new("due");
    let config = config_for(&dir);
    let pool = open_pool(&config).await.expect("open failed");

    let mut storage = probe_storage(&pool, &config);
    let id = push_probe(&mut storage, "deferred", 3, Duration::from_secs(2)).await;

    let ran = Arc::new(AtomicUsize::new(0));

    async fn handler(
        _task: Probe,
        counter: Data<Arc<AtomicUsize>>,
        worker: WorkerContext,
    ) -> Result<(), BoxDynError> {
        counter.fetch_add(1, Ordering::SeqCst);
        let _ = worker;
        Ok(())
    }

    let worker = WorkerBuilder::new("due-worker")
        .backend(probe_storage(&pool, &config))
        .data(Arc::clone(&ran))
        .catch_panic()
        .build(handler);

    let handle = tokio::spawn(worker.run());

    // Well before the job is due, nothing should have run.
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert_eq!(
        ran.load(Ordering::SeqCst),
        0,
        "a job scheduled 2s out must not run within the first second"
    );

    // Now give it time to come due, be picked up, and be acked. The worker is
    // stopped from outside rather than from inside the handler --- see the note
    // in `worker_executes_a_queued_job`.
    let mut status = read_status(&pool, &id).await.unwrap().expect("job exists");
    for _ in 0..150 {
        if status.is_terminal() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        status = read_status(&pool, &id).await.unwrap().expect("job exists");
    }
    handle.abort();

    assert_eq!(
        ran.load(Ordering::SeqCst),
        1,
        "the job should have run once it came due"
    );
    assert_eq!(status.status, Status::Done);

    pool.close().await;
}

// ---------------------------------------------------------------------------
// Argument round-tripping
// ---------------------------------------------------------------------------

/// The real job argument survives the codec round trip.
///
/// This is the bound that separates a durable queue from an in-process one:
/// `EmbedInterview` must go to bytes and back. Asserting it here means a field
/// added to that struct without a `Serialize` impl fails at test time rather
/// than at runtime on a queued job.
#[test]
fn embed_interview_round_trips_through_the_codec() {
    use crate::jobs::embed::EmbedInterview;

    let original = EmbedInterview::new("interview-uid-123");

    let encoded: CompactType =
        <JsonCodec<CompactType> as Codec<EmbedInterview>>::encode(&original)
            .expect("encode failed");
    let decoded: EmbedInterview =
        <JsonCodec<CompactType> as Codec<EmbedInterview>>::decode(&encoded)
            .expect("decode failed");

    assert_eq!(decoded.interview_uid, original.interview_uid);
}

/// The enqueue-side façade returns an id that the status-side can actually
/// find --- i.e. the id we hand the client is the id in the row.
///
/// This guards the sharp edge described in `jobs::queue`: apalis-sqlite will
/// happily invent an id and discard it, so a regression here would silently
/// hand clients unpollable ids.
#[tokio::test]
async fn enqueued_job_id_is_resolvable() {
    use crate::jobs::JobQueue;

    let dir = TempDir::new("roundtrip");
    let config = config_for(&dir);
    let pool = open_pool(&config).await.expect("open failed");

    let queue = JobQueue::new(pool.clone(), &config);
    let job_id = queue
        .enqueue_embed("some-interview-uid")
        .await
        .expect("enqueue failed");

    let status = queue
        .status(&job_id)
        .await
        .expect("status read failed")
        .expect("the id returned by enqueue must resolve to a real row");

    assert_eq!(status.id, job_id);
    assert_eq!(status.status, Status::Pending);
    assert_eq!(status.max_attempts, crate::jobs::queue::MAX_ATTEMPTS as i64);

    pool.close().await;
}

/// `is_terminal` distinguishes a job that will be retried from one that is
/// genuinely finished --- the distinction `check_status` cannot express.
#[test]
fn failed_is_terminal_only_once_attempts_are_exhausted() {
    use crate::jobs::status::JobStatus;

    let retrying = JobStatus {
        id: "x".into(),
        status: Status::Failed,
        attempts: 1,
        max_attempts: 3,
        run_at: 0,
        last_result: None,
    };
    assert!(
        !retrying.is_terminal(),
        "a failed job with attempts remaining is not finished"
    );

    let exhausted = JobStatus {
        attempts: 3,
        ..retrying
    };
    assert!(
        exhausted.is_terminal(),
        "a failed job at its ceiling is finished"
    );
}

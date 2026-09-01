//! Tests for queue behaviour.
//!
//! These exercise the queue mechanics --- enqueue/dequeue, concurrency,
//! shutdown draining, failure and panic isolation --- without touching Neo4j or
//! the ONNX embedder, so the whole file runs in CI with no services up.
//!
//! That is possible because `JobContext` takes its dependencies as fields
//! rather than reaching for globals: a test supplies `embedder: None`, and
//! `Graph::connect` is lazy enough (it opens no socket until the first query)
//! that an unconnected pool is fine for workers that never query. Constructor
//! injection is what buys this --- had `JobContext` built its own dependencies,
//! none of these tests could exist without a live Neo4j and a model on disk.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::jobs::backend::{Envelope, InProcessBackend, QueueBackend};
use crate::jobs::handle::{InMemoryStateStore, JobId, JobState, StateStore};
use crate::jobs::queue::{Queue, QueueError};
use crate::jobs::registry::{Registry, erase};
use crate::jobs::worker::{JobContext, Worker};

// ---------------------------------------------------------------------------
// Test doubles
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CountArgs {
    n: usize,
}

/// Records how many times it ran, and how many ran concurrently at the peak.
///
/// The counters are `Arc<AtomicUsize>` shared with the test body: the worker
/// itself is moved into the registry and erased, so the test can no longer
/// reach it by value. Sharing an atomic is how the assertion gets its data back
/// across that boundary.
struct CountingWorker {
    runs: Arc<AtomicUsize>,
    in_flight: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
    delay: Duration,
}

impl Worker<CountArgs> for CountingWorker {
    const NAME: &'static str = "counting";

    async fn perform(
        &self,
        _ctx: &JobContext,
        _job_id: &JobId,
        _args: CountArgs,
    ) -> anyhow::Result<()> {
        let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;

        // `fetch_max` in a single atomic op rather than a load-compare-store,
        // which would race: two workers could both read the old max and both
        // write a value lower than the true peak.
        self.peak.fetch_max(now, Ordering::SeqCst);

        tokio::time::sleep(self.delay).await;

        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        self.runs.fetch_add(1, Ordering::SeqCst);

        Ok(())
    }
}

struct FailingWorker;

impl Worker<CountArgs> for FailingWorker {
    const NAME: &'static str = "failing";

    async fn perform(
        &self,
        _ctx: &JobContext,
        _job_id: &JobId,
        _args: CountArgs,
    ) -> anyhow::Result<()> {
        anyhow::bail!("deliberate failure")
    }
}

struct PanickingWorker;

impl Worker<CountArgs> for PanickingWorker {
    const NAME: &'static str = "panicking";

    async fn perform(
        &self,
        _ctx: &JobContext,
        _job_id: &JobId,
        _args: CountArgs,
    ) -> anyhow::Result<()> {
        panic!("deliberate panic");
    }
}

/// A `JobContext` whose db and embedder are never touched.
///
/// `embedder: None` is what lets these tests run in CI without loading a real
/// ONNX model; no worker under test asks for it. The `db` pool is real but
/// unconnected --- `Graph::connect` opens no socket until the first query (see
/// `neo4j::connect`, which is why that function follows it with an explicit
/// ping), and no worker under test issues one.
fn test_context(state: Arc<dyn StateStore>) -> anyhow::Result<JobContext> {
    let config = neo4rs::ConfigBuilder::default()
        .uri("neo4j://127.0.0.1:7687")
        .user("neo4j")
        .password("neo4j")
        .db("neo4j")
        .build()?;

    Ok(JobContext {
        db: Arc::new(neo4rs::Graph::connect(config)?),
        embedder: None,
        state,
    })
}

fn envelope(name: &str, n: usize) -> Envelope {
    Envelope {
        id: JobId::generate(),
        name: name.to_string(),
        args: serde_json::json!({ "n": n }),
    }
}

// ---------------------------------------------------------------------------
// State store
// ---------------------------------------------------------------------------

#[test]
fn state_store_records_lifecycle() {
    let store = InMemoryStateStore::with_defaults();
    let id = JobId::generate();

    assert_eq!(store.get(&id), None, "unknown job should read as None");

    store.set_queued(&id);
    assert_eq!(store.get(&id), Some(JobState::Queued));

    store.set_progress(&id, 0.25, "working");
    assert!(matches!(
        store.get(&id),
        Some(JobState::Running { progress, .. }) if (progress - 0.25).abs() < f32::EPSILON
    ));

    store.set_completed(&id);
    assert_eq!(store.get(&id), Some(JobState::Completed));
}

#[test]
fn state_store_evicts_past_capacity() {
    // Capacity 2, TTL long enough not to interfere.
    let store = InMemoryStateStore::new(Duration::from_secs(3600), 2);

    let ids: Vec<JobId> = (0..5).map(|_| JobId::generate()).collect();

    for id in &ids {
        store.set_queued(id);
    }

    let live = ids.iter().filter(|id| store.get(id).is_some()).count();

    assert_eq!(live, 2, "capacity bound should hold after eviction");

    // The survivors should be the most recent, since eviction is oldest-first.
    assert!(
        store.get(&ids[4]).is_some(),
        "most recently touched entry must survive"
    );
}

#[test]
fn state_store_evicts_past_ttl() {
    let store = InMemoryStateStore::new(Duration::from_millis(1), 1000);
    let old = JobId::generate();

    store.set_queued(&old);
    std::thread::sleep(Duration::from_millis(20));

    // Eviction is triggered by writes, not by a timer, so a second write is
    // what sweeps the expired entry.
    store.set_queued(&JobId::generate());

    assert_eq!(store.get(&old), None, "expired entry should be swept");
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

#[test]
fn registry_registers_and_finds_by_name() {
    let registry = Registry::new().register(FailingWorker);

    assert!(registry.contains("failing"));
    assert!(!registry.contains("nonexistent"));
    assert_eq!(registry.len(), 1);
}

#[test]
#[should_panic(expected = "already registered")]
fn registry_rejects_duplicate_names() {
    let _ = Registry::new().register(FailingWorker).register(FailingWorker);
}

#[tokio::test]
async fn erased_handler_converts_panic_to_error() {
    // Erasure is testable without a running pool: build the handler and call
    // it directly. This is the isolation guarantee the worker loop depends on.
    let handler = erase(PanickingWorker);
    let state: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::with_defaults());

    let Ok(ctx) = test_context(Arc::clone(&state)) else {
        eprintln!("skipping: could not build a test context");
        return;
    };

    let result = handler(ctx, JobId::generate(), serde_json::json!({ "n": 1 })).await;

    let err = result.expect_err("a panicking job must surface as Err, not unwind");
    assert!(
        err.to_string().contains("deliberate panic"),
        "panic message should be preserved, got: {err}"
    );
}

#[tokio::test]
async fn erased_handler_rejects_malformed_arguments() {
    let handler = erase(FailingWorker);
    let state: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::with_defaults());

    let Ok(ctx) = test_context(Arc::clone(&state)) else {
        eprintln!("skipping: could not build a test context");
        return;
    };

    // `n` should be a number; a string cannot deserialize into usize.
    let result = handler(ctx, JobId::generate(), serde_json::json!({ "n": "nope" })).await;

    let err = result.expect_err("malformed args must fail the job");
    assert!(
        err.to_string().contains("deserialize"),
        "expected a deserialization error, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Backend
// ---------------------------------------------------------------------------

#[tokio::test]
async fn enqueue_rejects_when_full() {
    let state: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::with_defaults());

    // Capacity 1 and never started, so nothing drains it.
    let backend = InProcessBackend::new(1, 1, Arc::clone(&state));

    backend
        .enqueue(envelope("counting", 1))
        .await
        .expect("first enqueue should fit");

    let err = backend
        .enqueue(envelope("counting", 2))
        .await
        .expect_err("second enqueue must be refused, not awaited");

    assert!(
        err.to_string().contains("full"),
        "expected a capacity error, got: {err}"
    );
}

#[tokio::test]
async fn enqueue_marks_job_queued() {
    let state: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::with_defaults());
    let backend = InProcessBackend::new(1, 4, Arc::clone(&state));

    let env = envelope("counting", 1);
    let id = env.id.clone();

    backend.enqueue(env).await.expect("enqueue");

    assert_eq!(state.get(&id), Some(JobState::Queued));
}

#[tokio::test]
async fn start_is_rejected_twice() {
    let state: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::with_defaults());
    let backend = InProcessBackend::new(1, 4, Arc::clone(&state));
    let registry = Arc::new(Registry::new().register(FailingWorker));

    let Ok(ctx) = test_context(Arc::clone(&state)) else {
        eprintln!("skipping: could not build a test context");
        return;
    };

    backend
        .start(ctx.clone(), Arc::clone(&registry))
        .expect("first start should succeed");

    let err = backend
        .start(ctx, registry)
        .expect_err("second start must be refused");

    assert!(
        err.to_string().contains("already been started"),
        "got: {err}"
    );

    backend.shutdown().await.expect("shutdown");
}

// ---------------------------------------------------------------------------
// Queue facade
// ---------------------------------------------------------------------------

#[tokio::test]
async fn queue_rejects_an_unregistered_worker() {
    // The fail-fast path: an unroutable job is refused at enqueue, while the
    // caller is still there to be told, rather than dying in a worker later.
    let state: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::with_defaults());
    let backend = Arc::new(InProcessBackend::new(1, 4, Arc::clone(&state)));

    // Registry deliberately empty.
    let queue = Queue::new(backend, Arc::new(Registry::new()), Arc::clone(&state));

    // `expect_err` is unavailable here: it requires `Debug` on the Ok type, and
    // `JobHandle` holds an `Arc<dyn StateStore>`, which cannot derive it. A
    // match costs one more line and avoids contorting the production type to
    // suit the test.
    match queue.enqueue::<_, FailingWorker>(CountArgs { n: 1 }).await {
        Err(QueueError::UnknownJob(name)) => assert_eq!(name, "failing"),
        Err(other) => panic!("expected UnknownJob, got: {other}"),
        Ok(_) => panic!("an unregistered worker must be refused"),
    }
}

#[tokio::test]
async fn job_handle_observes_state_through_to_completion() {
    // Exercises the handle a mutation actually returns: enqueue, then watch the
    // same id move to Completed without ever touching the store directly.
    let state: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::with_defaults());
    let backend = Arc::new(InProcessBackend::new(1, 4, Arc::clone(&state)));

    let registry = Arc::new(Registry::new().register(CountingWorker {
        runs: Arc::new(AtomicUsize::new(0)),
        in_flight: Arc::new(AtomicUsize::new(0)),
        peak: Arc::new(AtomicUsize::new(0)),
        delay: Duration::from_millis(5),
    }));

    // The annotation is load-bearing: unsizing `Arc<InProcessBackend>` to
    // `Arc<dyn QueueBackend>` is a coercion, and a coercion needs a target type
    // to aim at. `Arc::clone(&backend)` would infer the concrete type and fail.
    let backend: Arc<dyn QueueBackend> = backend;

    let queue = Queue::new(backend, Arc::clone(&registry), Arc::clone(&state));

    let ctx = test_context(Arc::clone(&state)).expect("context");
    queue.start(ctx).expect("start");

    let handle = queue
        .enqueue::<_, CountingWorker>(CountArgs { n: 1 })
        .await
        .expect("enqueue");

    assert_eq!(handle.state(), Some(JobState::Queued));

    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(
        handle.state(),
        Some(JobState::Completed),
        "the handle should observe the job reaching completion"
    );

    // And the same state is reachable by id, which is what the GraphQL
    // resolver does.
    assert_eq!(queue.job_state(&handle.id), Some(JobState::Completed));

    queue.shutdown().await.expect("shutdown");
}

// ---------------------------------------------------------------------------
// End-to-end pool behaviour
// ---------------------------------------------------------------------------

#[tokio::test]
async fn runs_queued_jobs_to_completion() {
    let state: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::with_defaults());
    let runs = Arc::new(AtomicUsize::new(0));

    let registry = Arc::new(Registry::new().register(CountingWorker {
        runs: Arc::clone(&runs),
        in_flight: Arc::new(AtomicUsize::new(0)),
        peak: Arc::new(AtomicUsize::new(0)),
        delay: Duration::from_millis(5),
    }));

    let backend = InProcessBackend::new(2, 16, Arc::clone(&state));
    let ctx = test_context(Arc::clone(&state)).expect("context");

    backend.start(ctx, registry).expect("start");

    let mut ids = Vec::new();
    for n in 0..8 {
        let env = envelope("counting", n);
        ids.push(env.id.clone());
        backend.enqueue(env).await.expect("enqueue");
    }

    // Shutdown waits for in-flight work, which after cancellation means the
    // jobs already dequeued. Give the pool time to drain first so this asserts
    // on completion rather than on the drop behaviour.
    tokio::time::sleep(Duration::from_millis(300)).await;
    backend.shutdown().await.expect("shutdown");

    assert_eq!(runs.load(Ordering::SeqCst), 8, "every job should have run");

    for id in &ids {
        assert_eq!(
            state.get(id),
            Some(JobState::Completed),
            "job {id} should be marked completed"
        );
    }
}

#[tokio::test]
async fn workers_run_concurrently() {
    let state: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::with_defaults());
    let peak = Arc::new(AtomicUsize::new(0));

    let registry = Arc::new(Registry::new().register(CountingWorker {
        runs: Arc::new(AtomicUsize::new(0)),
        in_flight: Arc::new(AtomicUsize::new(0)),
        peak: Arc::clone(&peak),
        // Long enough that jobs must overlap if the pool is genuinely parallel.
        delay: Duration::from_millis(100),
    }));

    let backend = InProcessBackend::new(3, 16, Arc::clone(&state));
    let ctx = test_context(Arc::clone(&state)).expect("context");

    backend.start(ctx, registry).expect("start");

    for n in 0..6 {
        backend.enqueue(envelope("counting", n)).await.expect("enqueue");
    }

    tokio::time::sleep(Duration::from_millis(400)).await;
    backend.shutdown().await.expect("shutdown");

    let observed = peak.load(Ordering::SeqCst);

    assert!(
        observed > 1,
        "expected overlapping execution with 3 workers, peak concurrency was {observed}"
    );
    assert!(
        observed <= 3,
        "concurrency must not exceed the worker count, saw {observed}"
    );
}

#[tokio::test]
async fn failed_job_is_recorded_and_pool_survives() {
    let state: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::with_defaults());
    let runs = Arc::new(AtomicUsize::new(0));

    let registry = Arc::new(
        Registry::new()
            .register(FailingWorker)
            .register(PanickingWorker)
            .register(CountingWorker {
                runs: Arc::clone(&runs),
                in_flight: Arc::new(AtomicUsize::new(0)),
                peak: Arc::new(AtomicUsize::new(0)),
                delay: Duration::from_millis(1),
            }),
    );

    let backend = InProcessBackend::new(1, 16, Arc::clone(&state));
    let ctx = test_context(Arc::clone(&state)).expect("context");

    backend.start(ctx, registry).expect("start");

    let failing = envelope("failing", 1);
    let failing_id = failing.id.clone();
    backend.enqueue(failing).await.expect("enqueue failing");

    let panicking = envelope("panicking", 1);
    let panicking_id = panicking.id.clone();
    backend.enqueue(panicking).await.expect("enqueue panicking");

    // Queued last, on the same single worker: if either of the two above had
    // killed the worker task, this would never run. That is the real assertion.
    backend.enqueue(envelope("counting", 1)).await.expect("enqueue counting");

    tokio::time::sleep(Duration::from_millis(300)).await;
    backend.shutdown().await.expect("shutdown");

    assert!(
        matches!(state.get(&failing_id), Some(JobState::Failed { .. })),
        "a returned Err should be recorded as Failed"
    );
    assert!(
        matches!(state.get(&panicking_id), Some(JobState::Failed { .. })),
        "a panic should be recorded as Failed"
    );
    assert_eq!(
        runs.load(Ordering::SeqCst),
        1,
        "the worker must survive both and run the following job"
    );
}

#[tokio::test]
async fn shutdown_waits_for_in_flight_work() {
    let state: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::with_defaults());
    let runs = Arc::new(AtomicUsize::new(0));

    let registry = Arc::new(Registry::new().register(CountingWorker {
        runs: Arc::clone(&runs),
        in_flight: Arc::new(AtomicUsize::new(0)),
        peak: Arc::new(AtomicUsize::new(0)),
        delay: Duration::from_millis(200),
    }));

    let backend = InProcessBackend::new(1, 16, Arc::clone(&state));
    let ctx = test_context(Arc::clone(&state)).expect("context");

    backend.start(ctx, registry).expect("start");
    backend.enqueue(envelope("counting", 1)).await.expect("enqueue");

    // Let the worker pick the job up, then shut down mid-flight.
    tokio::time::sleep(Duration::from_millis(20)).await;

    let began = std::time::Instant::now();
    backend.shutdown().await.expect("shutdown");
    let waited = began.elapsed();

    assert_eq!(
        runs.load(Ordering::SeqCst),
        1,
        "shutdown must let an in-flight job finish rather than abandoning it"
    );
    assert!(
        waited >= Duration::from_millis(100),
        "shutdown returned in {waited:?}, too fast to have awaited the job"
    );
}

#[tokio::test]
async fn shutdown_drops_undequeued_jobs() {
    // The durability tradeoff, asserted rather than merely documented: work
    // still sitting in the channel at shutdown is lost.
    let state: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::with_defaults());
    let runs = Arc::new(AtomicUsize::new(0));

    let registry = Arc::new(Registry::new().register(CountingWorker {
        runs: Arc::clone(&runs),
        in_flight: Arc::new(AtomicUsize::new(0)),
        peak: Arc::new(AtomicUsize::new(0)),
        delay: Duration::from_millis(150),
    }));

    let backend = InProcessBackend::new(1, 64, Arc::clone(&state));
    let ctx = test_context(Arc::clone(&state)).expect("context");

    backend.start(ctx, registry).expect("start");

    for n in 0..20 {
        backend.enqueue(envelope("counting", n)).await.expect("enqueue");
    }

    tokio::time::sleep(Duration::from_millis(20)).await;
    backend.shutdown().await.expect("shutdown");

    let completed = runs.load(Ordering::SeqCst);

    assert!(
        completed < 20,
        "expected queued jobs to be dropped at shutdown, but all {completed} ran"
    );
}

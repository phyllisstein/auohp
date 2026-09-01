//! Job identity and observable state.
//!
//! There is no queue table here --- Neo4j is deliberately not the queue --- so
//! "where do I look up job status" needs its own answer. That answer is a
//! [`StateStore`]: an in-memory, bounded, TTL-evicted map from [`JobId`] to
//! [`JobState`], written by the worker loop and read by a GraphQL query.
//!
//! This is an honest tradeoff, not a hidden one. The state store is exactly as
//! durable as the queue it describes --- both die with the process. See the
//! spike doc's shutdown section.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A job's public identifier.
///
/// A newtype rather than a bare `String` so that a job id cannot be silently
/// passed where an interview uid or a statement uid is expected. All three are
/// nanoids of similar shape, which is exactly the situation where the compiler
/// should be helping.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobId(String);

impl JobId {
    /// Mint a fresh id, using the same alphabet as every other uid in the
    /// codebase (see `crate::uid`).
    pub fn generate() -> Self {
        Self(crate::uid::generate())
    }

    /// Rebuild an id from a client-supplied string, for status lookups.
    pub fn from_string(raw: String) -> Self {
        Self(raw)
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Where a job is in its lifecycle.
///
/// Loco's `JobStatus` also carries `Cancelled`. We omit it: cancellation of an
/// already-running in-process job would require the job body to cooperatively
/// poll a token, and nothing in this codebase does that yet. Adding a variant
/// later is additive; pretending to support cancellation we cannot deliver is
/// not.
#[derive(Clone, Debug, PartialEq)]
pub enum JobState {
    /// Accepted, not yet picked up by a worker.
    Queued,

    /// A worker is running it. `progress` is `0.0..=1.0` and `message` is a
    /// human-facing phase label ("transcribing", "embedding 400/1200").
    Running { progress: f32, message: String },

    /// Finished successfully.
    Completed,

    /// The worker returned `Err`, or panicked. The string is the rendered
    /// error --- we keep a message rather than the error value because
    /// `anyhow::Error` is neither `Clone` nor `PartialEq`, and this state is
    /// read many times by status queries.
    Failed { error: String },
}

/// Observable job state, decoupled from any particular storage.
///
/// This is a second trait boundary, and it exists for the same reason as
/// [`crate::jobs::QueueBackend`]: the in-memory implementation is a starting
/// point, not a commitment. A durable backend would supply a `StateStore` that
/// reads the same rows its queue writes, and no caller changes.
///
/// Methods take `&self`, not `&mut self` --- interior mutability lives in the
/// implementation. That is what lets this be shared as `Arc<dyn StateStore>`
/// across every worker task without an outer lock.
pub trait StateStore: Send + Sync {
    /// Record that a job has been accepted.
    fn set_queued(&self, id: &JobId);

    /// Record progress. Called potentially thousands of times per job, so
    /// implementations should be cheap and must not block on I/O.
    fn set_progress(&self, id: &JobId, fraction: f32, message: &str);

    /// Record terminal success.
    fn set_completed(&self, id: &JobId);

    /// Record terminal failure.
    fn set_failed(&self, id: &JobId, error: String);

    /// Read current state. `None` means unknown --- either never enqueued, or
    /// evicted after its TTL.
    fn get(&self, id: &JobId) -> Option<JobState>;
}

/// The entry actually stored, pairing state with the time it last changed so
/// that eviction has something to sort on.
struct Entry {
    state: JobState,
    touched: Instant,
}

/// Bounded, TTL-evicted in-memory [`StateStore`].
///
/// Two independent bounds, because either alone is insufficient. The TTL keeps
/// a low-traffic server from holding a completed job's state forever; the
/// capacity cap keeps a burst of enqueues from growing the map without limit
/// before any TTL expires. Loco needs neither because its rows live in a
/// database with its own retention story.
pub struct InMemoryStateStore {
    /// `Mutex<HashMap>` rather than a sharded or lock-free map. The critical
    /// section is a single hash lookup and a small write, and the contention
    /// ceiling is `worker_count + concurrent status queries` --- single digits.
    /// A `RwLock` would not help: every writer here takes the write path, and
    /// progress updates are the dominant traffic.
    inner: Mutex<HashMap<JobId, Entry>>,
    ttl: Duration,
    capacity: usize,
}

impl InMemoryStateStore {
    pub fn new(ttl: Duration, capacity: usize) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            ttl,
            capacity,
        }
    }

    /// Sensible defaults: keep a job's state for an hour, cap at 10k jobs.
    pub fn with_defaults() -> Self {
        Self::new(Duration::from_secs(60 * 60), 10_000)
    }

    /// Insert or overwrite one entry, then evict.
    ///
    /// Every mutating trait method funnels through here so the eviction policy
    /// has exactly one implementation.
    ///
    /// On lock poisoning we recover with `into_inner` rather than propagating.
    /// A poisoned lock means some other thread panicked *while holding it*, but
    /// the only thing under this lock is a `HashMap` of plain data --- there is
    /// no broken invariant to protect. This mirrors the treatment of the
    /// embedder mutex in `auohp-core`.
    fn put(&self, id: &JobId, state: JobState) {
        let mut map = self.inner.lock().unwrap_or_else(|e| e.into_inner());

        map.insert(
            id.clone(),
            Entry {
                state,
                touched: Instant::now(),
            },
        );

        Self::evict(&mut map, self.ttl, self.capacity);
    }

    /// Drop expired entries, then oldest-first until under capacity.
    ///
    /// Taking `&mut HashMap` rather than `&self` is what makes this callable
    /// from inside `put`'s critical section without re-locking --- the caller
    /// already proved exclusive access by holding the guard, and passing the
    /// `&mut` transfers that proof. An associated function rather than a method
    /// because it needs no `self`, only the two policy numbers.
    fn evict(map: &mut HashMap<JobId, Entry>, ttl: Duration, capacity: usize) {
        let now = Instant::now();

        // `retain` is a single pass with in-place removal --- notably better
        // than collecting doomed keys into a Vec and looping to remove them,
        // which is the shape this wants to be written in.
        map.retain(|_, entry| now.duration_since(entry.touched) < ttl);

        if map.len() <= capacity {
            return;
        }

        // Over capacity even after TTL eviction. Sort what is left by age and
        // drop the oldest. This is O(n log n) on a path that only runs when the
        // map is genuinely full, which for the default 10k cap is rare enough
        // to prefer simple code over a proper LRU intrusive list.
        let excess = map.len() - capacity;
        let mut by_age: Vec<(JobId, Instant)> =
            map.iter().map(|(k, v)| (k.clone(), v.touched)).collect();

        by_age.sort_by_key(|(_, touched)| *touched);

        for (id, _) in by_age.into_iter().take(excess) {
            map.remove(&id);
        }
    }
}

impl Default for InMemoryStateStore {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl StateStore for InMemoryStateStore {
    fn set_queued(&self, id: &JobId) {
        self.put(id, JobState::Queued);
    }

    fn set_progress(&self, id: &JobId, fraction: f32, message: &str) {
        self.put(
            id,
            JobState::Running {
                progress: fraction,
                message: message.to_string(),
            },
        );
    }

    fn set_completed(&self, id: &JobId) {
        self.put(id, JobState::Completed);
    }

    fn set_failed(&self, id: &JobId, error: String) {
        self.put(id, JobState::Failed { error });
    }

    fn get(&self, id: &JobId) -> Option<JobState> {
        let map = self.inner.lock().unwrap_or_else(|e| e.into_inner());

        map.get(id).map(|entry| entry.state.clone())
    }
}

/// What [`crate::jobs::Queue::enqueue`] hands back to a caller.
///
/// Deliberately not a `JoinHandle`. A `JoinHandle` would tie the caller's
/// ability to observe a job to the lifetime of one particular tokio task, which
/// is precisely the coupling the queue abstraction exists to break --- with a
/// durable backend the job may not run in this process at all. An id plus a
/// state store is a lookup, and a lookup works from anywhere.
#[derive(Clone)]
pub struct JobHandle {
    pub id: JobId,
    state: Arc<dyn StateStore>,
}

impl JobHandle {
    pub fn new(id: JobId, state: Arc<dyn StateStore>) -> Self {
        Self { id, state }
    }

    /// Current state, or `None` if unknown or evicted.
    pub fn state(&self) -> Option<JobState> {
        self.state.get(&self.id)
    }
}

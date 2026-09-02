//! Where the queue lives on disk, and what "durable" actually buys.
//!
//! # Location
//!
//! The path comes from `JOBS_DATABASE_URL`, defaulting to
//! `sqlite://./data/jobs.db`. Two deliberate choices are encoded here:
//!
//! 1. It is a *file*, not `:memory:`. An in-memory SQLite database would make
//!    every test fast and would also silently reproduce the exact failure mode
//!    apalis is being evaluated to fix --- a queue that evaporates on restart.
//!    Tests opt into `:memory:` explicitly; production does not get to.
//!
//! 2. It is a separate file from Neo4j's store, and that is the honest
//!    operational cost of this design. A deployment that previously had one
//!    stateful thing to back up now has two, and they can disagree: the graph
//!    can commit an interview while the queue file is lost, or vice versa.
//!    See `docs/spikes/apalis-sqlite.md` for why that is survivable here
//!    (the job is idempotent and re-derivable from graph state) but is still a
//!    real cost.
//!
//! # Creation
//!
//! `create_if_missing(true)` on the connect options makes the file appear on
//! first run, and `SqliteStorage::setup` runs the embedded migrations that
//! create the `Jobs` and `Workers` tables. Both are idempotent, so this is
//! safe to run unconditionally at every boot --- there is no separate
//! "install" step an operator can forget.
//!
//! # Restart semantics --- the actual durability claim
//!
//! This is the part worth being precise about, because it is the main reason
//! apalis is under consideration at all.
//!
//! A task's row carries `status`, `run_at`, `lock_by`, and `lock_at`. When a
//! worker picks a task up it does *not* delete the row; it flips the row to
//! `Running` and stamps `lock_by` with its worker id. So a process that dies
//! mid-job leaves behind a row that is still `Running`, still owned by a
//! worker id that no longer exists.
//!
//! Recovery is not automatic-on-startup; it is automatic-on-*heartbeat*. Each
//! live worker's `Backend::heartbeat` stream interleaves two things: a
//! keep-alive that bumps `Workers.last_seen`, and `reenqueue_orphaned`, which
//! runs (paraphrasing `queries/backend/reenqueue_orphaned.sql`) "any row that
//! is Running or Queued, whose owning worker has not been seen for longer than
//! the keep-alive window, goes back to Pending with attempts incremented".
//!
//! Three consequences follow, and all three are load-bearing:
//!
//! - A queued-but-never-started job survives a restart trivially: it is still
//!   `Pending` with a `run_at` in the past, so the next poll picks it up.
//! - An interrupted in-flight job is recovered, but only after the orphan
//!   window elapses --- not instantly. Set that window with
//!   `Config::set_reenqueue_orphaned_after`.
//! - Recovery *re-runs the job body*. That makes idempotency a correctness
//!   requirement, not a nicety. Our embedding job satisfies it because writing
//!   a vector onto a Statement node is `MATCH ... SET`-shaped: doing it twice
//!   is indistinguishable from doing it once.

use std::time::Duration;

use anyhow::Context as _;
use apalis_sqlite::{Config, SqliteStorage};
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;

/// Default on-disk location for the queue.
///
/// Relative to the process working directory, which in the Docker image is the
/// app root; mount a volume at `./data` to make the queue outlive the
/// container.
pub const DEFAULT_JOBS_URL: &str = "sqlite://./data/jobs.db";

/// How long a worker may be silent before its in-flight tasks are considered
/// stranded and returned to `Pending`.
///
/// This is the restart-recovery latency. Too low and a merely-busy worker has
/// its own live task stolen out from under it (the task then runs twice ---
/// safe here, wasteful anywhere); too high and a crashed process's work sits
/// idle. Thirty seconds is comfortably longer than the keep-alive interval and
/// short enough that a crash-loop still makes progress.
pub const ORPHAN_RECLAIM_AFTER: Duration = Duration::from_secs(30);

/// Tuning knobs for the queue, resolved once at startup.
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// sqlx connection URL, e.g. `sqlite://./data/jobs.db`.
    pub url: String,
    /// How long before a silent worker's tasks are reclaimed.
    pub orphan_reclaim_after: Duration,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            url: std::env::var("JOBS_DATABASE_URL")
                .unwrap_or_else(|_| DEFAULT_JOBS_URL.to_string()),
            orphan_reclaim_after: ORPHAN_RECLAIM_AFTER,
        }
    }
}

impl StorageConfig {
    /// An ephemeral, per-connection database for tests.
    ///
    /// Note the pool implications: `:memory:` gives each *connection* its own
    /// private database, so a multi-connection pool would appear to lose rows
    /// at random. `open_pool` pins the pool to a single connection for exactly
    /// this reason.
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self {
            url: "sqlite::memory:".to_string(),
            // Tests that exercise orphan recovery want it to trigger promptly.
            orphan_reclaim_after: Duration::from_secs(1),
        }
    }

    /// Translate into apalis's own `Config` for one named queue.
    ///
    /// `Config::new` takes the queue name; the rest is our policy layered on
    /// top. This is the only place the two vocabularies meet.
    ///
    /// The queue name is a parameter rather than a constant because a
    /// `StorageConfig` describes the *database* --- where it lives, how long an
    /// orphan window is --- and those settings are shared by every job kind,
    /// while the queue name distinguishes one job kind from another within that
    /// one file. Callers pass the owning job type's `QUEUE` const (see
    /// [`crate::jobs::embed::EmbedInterview::QUEUE`]), which is what keeps the
    /// enqueue and worker sides naming the same queue.
    pub fn to_apalis_config(&self, queue: &str) -> Config {
        Config::new(queue).set_reenqueue_orphaned_after(self.orphan_reclaim_after)
    }
}

/// Open (creating if necessary) the SQLite pool backing the queue, and run
/// apalis's migrations against it.
///
/// Returns a pool rather than a `SqliteStorage` because the pool is the shared
/// resource: the enqueue side and the worker side each build their own
/// `SqliteStorage` *view* over this one pool. `SqliteStorage` is generic over
/// its argument type `T`, so those two views are genuinely different Rust
/// types even when they address the same rows --- sharing the pool rather than
/// the storage handle is what keeps that from being a problem.
pub async fn open_pool(config: &StorageConfig) -> anyhow::Result<SqlitePool> {
    // `from_str` parses the `sqlite://` URL; `create_if_missing` is what makes
    // first-run work without an install step. Without it sqlx returns error
    // code 14 (SQLITE_CANTOPEN) rather than creating the file.
    let options: SqliteConnectOptions = config
        .url
        .parse::<SqliteConnectOptions>()
        .with_context(|| format!("invalid JOBS_DATABASE_URL: {}", config.url))?
        .create_if_missing(true);

    // Ensure the parent directory exists. sqlx will create the *file* but not
    // the directory holding it, and the default path (`./data/jobs.db`) has a
    // directory component that will not exist on a fresh checkout.
    if let Some(path) = options.get_filename().parent()
        && !path.as_os_str().is_empty()
    {
        std::fs::create_dir_all(path)
            .with_context(|| format!("could not create jobs directory {}", path.display()))?;
    }

    // max_connections(1) deserves its own justification, because it looks like
    // a bottleneck and mostly is not.
    //
    // SQLite serializes writers at the database level regardless of pool size,
    // so extra connections buy no write concurrency --- they buy
    // `SQLITE_BUSY` errors. And for `sqlite::memory:` a second connection is
    // outright wrong: each connection would get a *separate* empty database,
    // so the migrations run on one and the queries on another.
    //
    // The read side is not hurt because WAL mode (set by
    // `SqliteStorage::setup`) lets readers proceed against a snapshot while a
    // writer holds the write lock.
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .with_context(|| format!("could not open jobs database at {}", config.url))?;

    // Idempotent: creates the Jobs/Workers tables and sets WAL + NORMAL
    // synchronous pragmas. Safe to run on every boot.
    SqliteStorage::setup(&pool)
        .await
        .context("apalis migrations failed")?;

    Ok(pool)
}

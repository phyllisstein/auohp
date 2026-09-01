//! Reading task state back out of the queue.
//!
//! # Why this queries the table directly instead of using `WaitForCompletion`
//!
//! apalis-sqlite does implement `WaitForCompletion`, whose `check_status`
//! looks like exactly the right tool. It is not, and the reason is visible in
//! the SQL it runs (`queries/backend/fetch_completed_tasks.sql`): the WHERE
//! clause admits only
//!
//! ```text
//! status = 'Done' OR (status = 'Failed' AND attempts >= max_attempts) OR status = 'Killed'
//! ```
//!
//! In other words `check_status` reports *terminal* tasks only. A task that is
//! `Pending`, `Queued`, `Running`, or `Failed`-but-retrying returns no row at
//! all, which is indistinguishable from an id that never existed. For a
//! progress-polling endpoint --- whose whole job is to say "still working" ---
//! that is precisely the wrong half of the state space.
//!
//! It also assumes the stored `last_result` deserializes into
//! `Result<O, String>` and will `unwrap()` if it does not.
//!
//! So we read the row ourselves. The schema is stable across apalis-sqlite's
//! migrations and is public API in the practical sense (its migrations ship in
//! the crate), but this is a coupling worth naming: it is the price of wanting
//! richer status than the trait exposes.
//!
//! # Runtime queries, not `sqlx::query!`
//!
//! Deliberately `sqlx::query_as` with a runtime string rather than the
//! compile-time-checked `query!` macro. `query!` requires either a live
//! database at build time or a checked-in `.sqlx` offline cache, which would
//! impose a build-time dependency on a schema that belongs to a third-party
//! crate. The rows here are three columns wide; the checking is not worth the
//! build-system entanglement.

use apalis_core::task::status::Status;
use sqlx::{Row, SqlitePool};

/// A task's current state, as read from the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobStatus {
    /// The task id, echoing back what was asked for.
    pub id: String,
    /// Lifecycle state.
    pub status: Status,
    /// How many times the body has been started.
    pub attempts: i64,
    /// Attempt ceiling from the row --- the durable half of the retry policy.
    pub max_attempts: i64,
    /// Earliest time the task may run, as a unix timestamp. In the future for
    /// a scheduled task that has not come due.
    pub run_at: i64,
    /// The last result blob apalis wrote, if any. Holds the error message for
    /// a failed task.
    pub last_result: Option<String>,
}

impl JobStatus {
    /// Whether the task has reached a state it will not leave on its own.
    ///
    /// Note that `Failed` is *not* terminal while attempts remain --- the
    /// fetch query will pick such a row up again. Encoding that here keeps the
    /// distinction in one place rather than scattering
    /// `attempts >= max_attempts` comparisons through the resolvers.
    pub fn is_terminal(&self) -> bool {
        match self.status {
            Status::Done | Status::Killed => true,
            Status::Failed => self.attempts >= self.max_attempts,
            Status::Pending | Status::Queued | Status::Running => false,
            // `Status` is `#[non_exhaustive]`, so a wildcard arm is required
            // for this match to compile against future variants.
            _ => false,
        }
    }
}

/// Fetch one task's state by id. `Ok(None)` means no such task.
pub async fn read_status(pool: &SqlitePool, job_id: &str) -> anyhow::Result<Option<JobStatus>> {
    let row = sqlx::query(
        "SELECT id, status, attempts, max_attempts, run_at, last_result
         FROM Jobs
         WHERE id = ?1",
    )
    .bind(job_id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let raw_status: String = row.try_get("status")?;

    Ok(Some(JobStatus {
        id: row.try_get("id")?,
        // apalis stores the status as its `Display` string, and `FromStr` is
        // the exact inverse --- so this round-trips through the same
        // representation the worker writes, rather than a parallel mapping of
        // ours that could drift.
        status: raw_status
            .parse::<Status>()
            .map_err(|e| anyhow::anyhow!("unrecognized job status {raw_status:?}: {e}"))?,
        attempts: row.try_get("attempts")?,
        max_attempts: row.try_get("max_attempts")?,
        run_at: row.try_get("run_at")?,
        last_result: row.try_get("last_result")?,
    }))
}

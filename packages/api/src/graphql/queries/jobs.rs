//! GraphQL surface for background job status.
//!
//! This is the read side of the `embeddingJobId` that `seedInterview` returns:
//! the client holds an opaque id and polls `jobStatus(jobId:)` until the
//! result stops being in-flight.
//!
//! It is deliberately equivalent in capability to what the hand-rolled spike
//! exposed --- lifecycle state, attempt counts, an error message --- plus two
//! fields that only exist because the queue is durable: `runAt` (when a
//! *scheduled* task becomes eligible) and a `maxAttempts` that survives a
//! process restart.

use async_graphql::{Context, Enum, Object, SimpleObject};

use crate::jobs::JobQueue;
use crate::jobs::status::JobStatus;
use apalis_core::task::status::Status;

/// Lifecycle state of a background job.
///
/// A local mirror of `apalis_core::task::status::Status` rather than a direct
/// re-export, for two reasons. First, `Status` is `#[non_exhaustive]`, so a
/// new upstream variant would otherwise silently widen this crate's public
/// GraphQL schema. Second, deriving async-graphql's `Enum` requires the type
/// to be local (the orphan rule). Mapping explicitly makes the schema's
/// stability our decision rather than a dependency's.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum JobState {
    /// Accepted and waiting. For a scheduled job, `runAt` may still be future.
    Pending,
    /// Claimed by a worker but not yet started.
    Queued,
    /// Executing now.
    Running,
    /// Finished successfully.
    Done,
    /// The last attempt failed. Check `attempts` against `maxAttempts` to tell
    /// a job that will be retried from one that has given up.
    Failed,
    /// Abandoned --- attempts exhausted, or aborted by a panic.
    Killed,
}

impl From<Status> for JobState {
    fn from(value: Status) -> Self {
        match value {
            Status::Pending => Self::Pending,
            Status::Queued => Self::Queued,
            Status::Running => Self::Running,
            Status::Done => Self::Done,
            Status::Failed => Self::Failed,
            Status::Killed => Self::Killed,
            // Required because `Status` is `#[non_exhaustive]`. Treating an
            // unknown future variant as Pending is the conservative choice: it
            // reads as "not finished", so a polling client keeps polling
            // rather than concluding success.
            _ => Self::Pending,
        }
    }
}

/// A background job's current state.
#[derive(SimpleObject)]
pub struct JobStatusPayload {
    /// The job id that was polled.
    pub id: String,
    /// Lifecycle state.
    pub state: JobState,
    /// How many times execution has been attempted.
    pub attempts: i32,
    /// Attempt ceiling before the job is abandoned.
    pub max_attempts: i32,
    /// Unix timestamp of the earliest moment this job may run. In the future
    /// for a delayed job that has not yet come due.
    pub run_at: i32,
    /// True once the job will not change state on its own.
    pub is_terminal: bool,
    /// The last recorded result. Carries the error text for a failed job.
    pub last_result: Option<String>,
}

impl From<JobStatus> for JobStatusPayload {
    fn from(value: JobStatus) -> Self {
        Self {
            is_terminal: value.is_terminal(),
            id: value.id,
            state: value.status.into(),
            // Narrowing casts. These columns are SQLite INTEGERs (i64) but
            // GraphQL's Int is 32-bit. Attempt counts are single digits and
            // `run_at` is a unix timestamp that stays in range until 2038, so
            // this is safe today --- flagged rather than hidden.
            attempts: value.attempts as i32,
            max_attempts: value.max_attempts as i32,
            run_at: value.run_at as i32,
            last_result: value.last_result,
        }
    }
}

#[derive(Default)]
pub struct JobsQuery;

#[Object]
impl JobsQuery {
    /// Look up a background job by the id returned when it was enqueued.
    ///
    /// Returns `null` for an unknown id. Note that "unknown" and "long since
    /// vacuumed" are indistinguishable here --- a client that polls a very old
    /// id gets `null`, not an error.
    async fn job_status(
        &self,
        ctx: &Context<'_>,
        job_id: String,
    ) -> async_graphql::Result<Option<JobStatusPayload>> {
        let queue = ctx.data::<JobQueue>()?;

        let status = queue
            .status(&job_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("failed to read job status: {e}")))?;

        Ok(status.map(Into::into))
    }
}

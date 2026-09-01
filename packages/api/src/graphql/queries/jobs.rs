//! Job status queries.
//!
//! This is the answer to "where does job state get observed, given there is no
//! queue table": a resolver that reads the in-memory state store. The client
//! holds the id returned by the enqueueing mutation and polls this.

use async_graphql::{Context, Enum, Object, SimpleObject};

use crate::jobs::handle::{JobId, JobState};
use crate::jobs::Queue;

/// Lifecycle phase of a job, flattened for GraphQL.
///
/// The Rust `JobState` is a sum type carrying per-variant payloads, which
/// GraphQL has no direct equivalent for --- a GraphQL enum is a bare tag. So
/// the wire type splits into a tag plus nullable fields, and `progress` /
/// `message` / `error` are non-null only for the phases that define them. This
/// is a genuine narrowing: the Rust type makes "completed with an error string"
/// unrepresentable, and the GraphQL projection does not.
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum JobPhase {
    Queued,
    Running,
    Completed,
    Failed,
}

/// A job's observable status.
#[derive(SimpleObject)]
pub struct JobStatus {
    /// The id returned when the job was enqueued.
    pub id: String,

    pub phase: JobPhase,

    /// Fraction complete, `0.0..=1.0`. Only set while running.
    pub progress: Option<f64>,

    /// Human-facing phase label. Only set while running.
    pub message: Option<String>,

    /// Rendered error. Only set on failure.
    pub error: Option<String>,
}

impl JobStatus {
    /// Project the internal state onto the wire type.
    fn from_state(id: &JobId, state: JobState) -> Self {
        let (phase, progress, message, error) = match state {
            JobState::Queued => (JobPhase::Queued, None, None, None),
            JobState::Running { progress, message } => (
                JobPhase::Running,
                Some(f64::from(progress)),
                Some(message),
                None,
            ),
            JobState::Completed => (JobPhase::Completed, None, None, None),
            JobState::Failed { error } => (JobPhase::Failed, None, None, Some(error)),
        };

        Self {
            id: id.to_string(),
            phase,
            progress,
            message,
            error,
        }
    }
}

#[derive(Default)]
pub struct JobsQuery;

#[Object]
impl JobsQuery {
    /// Status of one background job.
    ///
    /// Returns `null` when the id is unknown --- which conflates "never
    /// existed" with "completed long enough ago to have been evicted from the
    /// state store". A durable backend would distinguish them; this one cannot.
    /// Clients should treat a `null` on a previously-seen id as "finished,
    /// outcome no longer retained" rather than as an error.
    async fn job(&self, ctx: &Context<'_>, id: String) -> async_graphql::Result<Option<JobStatus>> {
        let queue = ctx.data::<Queue>()?;
        let id = JobId::from_string(id);

        Ok(queue
            .job_state(&id)
            .map(|state| JobStatus::from_state(&id, state)))
    }
}

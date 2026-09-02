//! What every background job kind has in common.
//!
//! # Why a trait rather than a second set of methods
//!
//! [`JobQueue`](crate::jobs::JobQueue) originally exposed only
//! `enqueue_embed`/`enqueue_embed_after`, with no generic operation underneath
//! them. That was a missing layer rather than a design: a second job kind would
//! have meant a second near-verbatim pair of methods, differing only in the
//! type they construct and the queue they name.
//!
//! [`QueuedJob`] supplies the layer. It carries the two things the enqueue path
//! needs from a job type --- the queue it belongs to, and the bounds that let
//! apalis store it --- so `enqueue<T>` can be written once.
//!
//! # The bounds, and where they come from
//!
//! None of these are invented here; each is the minimum apalis already demands,
//! collected into one name so call sites do not restate them:
//!
//! - `Serialize + DeserializeOwned` --- `JsonCodec` encodes the job's arguments
//!   into the row's payload column and decodes them again in the worker. This
//!   is the bound that physically separates a durable queue from an in-process
//!   one: a `tokio::mpsc` moves any `Send` value, a persistent queue can only
//!   move values that survive a round trip through bytes.
//! - `Send + 'static` --- the task crosses a `tokio::spawn` boundary.
//! - `Unpin` --- required by `SqliteStorage`'s `Backend` impl
//!   (`Args: Send + 'static + Unpin`), because the fetch side holds tasks in a
//!   stream it moves out of.
//! - `Sync` --- required by its `Sink` impl (`Args: Send + Sync + 'static`),
//!   which is the write path `push_task` is derived from.
//!
//! The last two are worth noting together: the read path wants `Unpin`, the
//! write path wants `Sync`, and neither wants both. This trait has to be the
//! union of what every path demands, because it now stands in for all of them.
//!
//! That union is also the honest cost of generalizing. With a concrete job type
//! the compiler checked each of these against the real struct and never had to
//! name them --- `EmbedInterview` is a `String` in a wrapper, so all four hold
//! trivially. Writing `enqueue<T>` is what forces every silently-satisfied
//! requirement to become an explicit bound.
//!
//! # Why `QUEUE` is on the job type
//!
//! The enqueue side and the worker side each build their own storage view, and
//! both must name the same queue: apalis filters `WHERE job_type = ?2`, so a
//! mismatch means the worker polls for rows that do not exist. Jobs would sit
//! `Pending` forever with no error on either side, and nothing in the type
//! system compares two string literals.
//!
//! Hanging the name off the job type makes agreement structural rather than
//! conventional --- both sides write `T::QUEUE`, so there is no second literal
//! to get wrong.

use serde::Serialize;
use serde::de::DeserializeOwned;

/// A job kind that [`JobQueue`](crate::jobs::JobQueue) can enqueue.
///
/// Implementors are argument structs --- the data a job needs, not the code
/// that runs it. The handler lives with the worker; this describes only what is
/// written to the row.
pub trait QueuedJob: Serialize + DeserializeOwned + Send + Sync + Unpin + 'static {
    /// The apalis queue this job kind is filed under.
    ///
    /// apalis partitions the `Jobs` table by a `job_type` column and every
    /// fetch query filters on it, so distinct job kinds share one database file
    /// without seeing each other's work. Two job kinds must not share a name
    /// unless they are genuinely the same job.
    const QUEUE: &'static str;
}

//! Background job processing, backed by apalis + SQLite.
//!
//! # Why a job queue at all
//!
//! Embedding a whole interview takes minutes. It cannot happen inside the
//! `seedInterview` mutation, so something has to carry the work past the end
//! of the HTTP response. The previous implementation used a bare
//! `tokio::spawn`, which is the smallest possible version of that idea and
//! also the least accountable one: the handle is dropped, so nothing can
//! report on the task, nothing retries it, and a process restart loses it
//! silently.
//!
//! # The shape apalis imposes
//!
//! apalis is not a "job framework" in the Sidekiq sense. Its central claim is
//! narrower and more interesting: **a worker is a `tower::Service`**, and a
//! task is its request type. Concretely, a handler like
//!
//! ```ignore
//! async fn embed_interview(args: EmbedInterview, data: Data<Deps>) -> Result<(), BoxDynError>
//! ```
//!
//! is lifted by `task_fn` into `Service<Task<EmbedInterview, SqliteContext, Ulid>>`.
//! Once it is a `Service`, every cross-cutting concern is an ordinary
//! `tower::Layer` --- and that is why apalis's own feature table reads
//! `retry = ["tower/retry"]`, `timeout = ["tower/timeout"]`,
//! `limit = ["tower/limit"]`. It is not reimplementing those; it is reusing
//! the ones this project already compiles for axum.
//!
//! # Module layout
//!
//! - [`embed`] --- the one real job: re-read an interview's statements from
//!   Neo4j and write embeddings back.
//! - [`status`] --- reading task state back out of the SQLite `Jobs` table,
//!   which is what makes a job id pollable from GraphQL.
//! - [`queue`] --- the enqueue-side handle that resolvers hold via `.data()`.
//! - [`storage`] --- where the database file lives and how it is created.

pub mod embed;
pub mod queue;
pub mod status;
pub mod storage;

#[cfg(test)]
mod tests;

pub use queue::JobQueue;
pub use storage::{StorageConfig, open_pool};

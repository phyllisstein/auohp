//! The caller-facing handle.
//!
//! [`Queue`] is what a GraphQL resolver holds. It is a newtype over
//! `Arc<dyn QueueBackend>` plus the registry and state store, and every method
//! forwards. That indirection is the entire point: resolvers name `Queue`, not
//! `InProcessBackend`, so swapping the backend touches this file's constructor
//! and nothing else in the codebase.

use std::sync::Arc;

use serde::Serialize;

use crate::jobs::backend::{Envelope, QueueBackend};
use crate::jobs::handle::{JobHandle, JobId, JobState, StateStore};
use crate::jobs::registry::Registry;
use crate::jobs::worker::{JobContext, Worker};

/// What can go wrong at enqueue time.
///
/// A typed error rather than bare `anyhow`, because the caller genuinely needs
/// to distinguish these: `Full` is a retryable "try again shortly" that should
/// surface to a client as backpressure, while `UnknownJob` is a programming
/// error that no retry will fix. Collapsing both into one opaque error would
/// force resolvers to string-match to tell them apart.
#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    /// No worker is registered under this name.
    #[error("no worker is registered for job {0:?}")]
    UnknownJob(String),

    /// Arguments could not be serialized into the erased envelope form.
    #[error("failed to serialize job arguments: {0}")]
    Serialize(#[from] serde_json::Error),

    /// The queue is at capacity, or shut down.
    #[error("{0}")]
    Full(String),
}

/// A cloneable handle to the background job system.
///
/// Cheap to clone --- three `Arc` bumps --- which is what makes it suitable for
/// `.data()` injection into async-graphql, where each request's context holds
/// its own view of it.
#[derive(Clone)]
pub struct Queue {
    backend: Arc<dyn QueueBackend>,
    registry: Arc<Registry>,
    state: Arc<dyn StateStore>,
}

impl Queue {
    /// Assemble a queue from its three parts.
    ///
    /// The registry arrives already populated and frozen in an `Arc`; see
    /// `Registry`'s note on why registration closes before startup.
    pub fn new(
        backend: Arc<dyn QueueBackend>,
        registry: Arc<Registry>,
        state: Arc<dyn StateStore>,
    ) -> Self {
        Self {
            backend,
            registry,
            state,
        }
    }

    /// Enqueue a job for `W`, returning a handle for observing it.
    ///
    /// The turbofish at the call site (`queue.enqueue::<_, EmbedWorker>(args)`)
    /// is what selects the worker. `Args` is then inferred from the value
    /// passed, so only the worker type usually needs naming.
    ///
    /// Note this takes the worker only as a *type*, never a value: the instance
    /// lives in the registry, put there at startup. Enqueueing therefore cannot
    /// accidentally construct a second, differently-configured worker --- the
    /// type is used purely to recover `W::NAME` and the `Args` type.
    pub async fn enqueue<Args, W>(&self, args: Args) -> Result<JobHandle, QueueError>
    where
        Args: Serialize + for<'de> serde::Deserialize<'de> + Send + 'static,
        W: Worker<Args>,
    {
        let name = W::NAME;

        // Fail fast on an unroutable job. Without this the job would be
        // accepted, sit in the channel, and only fail when a worker picked it
        // up --- by which time the caller is long gone and the failure is a log
        // line instead of a response.
        if !self.registry.contains(name) {
            return Err(QueueError::UnknownJob(name.to_string()));
        }

        let envelope = Envelope {
            id: JobId::generate(),
            name: name.to_string(),
            args: serde_json::to_value(args)?,
        };

        let id = envelope.id.clone();

        self.backend
            .enqueue(envelope)
            .await
            .map_err(|e| QueueError::Full(e.to_string()))?;

        Ok(JobHandle::new(id, Arc::clone(&self.state)))
    }

    /// Look up a job's state by id, for a status query.
    ///
    /// `None` means unknown: never enqueued, or evicted after its TTL. A caller
    /// cannot distinguish those, which is a real limitation of a memory-backed
    /// store and is called out in the spike doc.
    pub fn job_state(&self, id: &JobId) -> Option<JobState> {
        self.state.get(id)
    }

    /// Start the worker pool. Called once from `main`, after registration.
    pub fn start(&self, ctx: JobContext) -> anyhow::Result<()> {
        self.backend.start(ctx, Arc::clone(&self.registry))
    }

    /// Stop accepting jobs and wait for in-flight work to finish.
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        self.backend.shutdown().await
    }

    /// Description of the active backend, for the startup log.
    pub fn describe(&self) -> String {
        self.backend.describe()
    }
}

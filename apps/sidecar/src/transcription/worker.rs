//! Worker: the thin bridge between the registry's spawned future and
//! `auohp_core::transcription::run`.
//!
//! `pipeline::run` is sync and CPU-bound, so it goes onto tokio's
//! blocking pool via `spawn_blocking`. We `select!` its `JoinHandle`
//! against the cancel token: if cancel fires first, we abandon the
//! result.
//!
//! FIXME (coarse cancellation): the spawn_blocking thread keeps running
//! to completion even after we abandon its handle. True mid-decode
//! cancellation requires plumbing the `CancellationToken` into
//! whisper-rs's `set_abort_callback_safe` from inside auohp-core's
//! whisper module. Until then, cancel takes effect at the *next* job
//! boundary, not in the middle of one.

use std::path::PathBuf;

use thiserror::Error;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use auohp_core::transcription::{TranscriptionResult, run as run_pipeline};

use super::registry::{Event, JobId};
use super::source::TranscribeSource;

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("cancelled")]
    Cancelled,
    #[error("transcription pipeline failed: {0}")]
    Pipeline(#[from] anyhow::Error),
    #[error("blocking task panicked")]
    Panicked,
}

pub async fn run(
    source: TranscribeSource,
    cancel: CancellationToken,
    events: broadcast::Sender<Event>,
    id: JobId,
) -> Result<TranscriptionResult, WorkerError> {
    // Cheap pre-flight check: if cancel fired between submit and the
    // worker actually getting polled, bail out without touching disk.
    if cancel.is_cancelled() {
        return Err(WorkerError::Cancelled);
    }

    // No wildcard arm: when Vimeo (or another variant) lands, the
    // compiler will refuse to build until this match is updated, which
    // is exactly the refactoring foothold we want for a same-crate enum.
    // (`#[non_exhaustive]` only suppresses exhaustiveness checking for
    // *external* crates; inside the defining crate, the match sees the
    // real variant set.)
    let path: PathBuf = match source {
        TranscribeSource::Local { path, .. } => path,
    };

    let _ = events.send(Event::Stage {
        id: id.clone(),
        message: "decoding audio".into(),
    });

    // spawn_blocking returns a JoinHandle<T> that we await; it's a
    // future, so it composes with select!. The closure itself runs on
    // a dedicated thread from tokio's blocking pool, leaving the async
    // workers free to make progress on other tasks.
    let blocking = tokio::task::spawn_blocking(move || run_pipeline(&path));

    // `biased;` makes select! poll arms in source order rather than
    // pseudo-random. We want cancel to win ties so that hammering
    // cancel during a fast-finishing job reliably reports "cancelled"
    // rather than racing the completion arm.
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(WorkerError::Cancelled),
        joined = blocking => match joined {
            Ok(Ok(transcription)) => Ok(transcription),
            Ok(Err(e)) => Err(WorkerError::Pipeline(e)),
            Err(_) => Err(WorkerError::Panicked),
        }
    }
}

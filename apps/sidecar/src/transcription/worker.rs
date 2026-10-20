//! Runs the blocking `auohp_core` pipeline, abandoning its result if the cancel
//! token fires first.
//!
//! FIXME: the blocking thread still runs to completion after cancel. Mid-decode
//! cancellation needs the token plumbed into whisper-rs's
//! `set_abort_callback_safe` inside auohp-core.

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
    if cancel.is_cancelled() {
        return Err(WorkerError::Cancelled);
    }

    let path: PathBuf = match source {
        TranscribeSource::Local { path, .. } => path,
    };

    let _ = events.send(Event::Stage {
        id: id.clone(),
        message: "decoding audio".into(),
    });

    let blocking = tokio::task::spawn_blocking(move || run_pipeline(&path));

    // `biased` so cancel wins ties against a just-finished job.
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

//! Single-slot job registry: at most one transcription runs at a time.
//!
//! The slot holds handles to the live job (cancel token, event sender, id,
//! source). A wrapper task spawned at submit time emits the terminal `Event`
//! and clears the slot.

use std::sync::Arc;

use serde::Serialize;
use thiserror::Error;
use tokio::sync::{Mutex, broadcast};
use tokio_util::sync::CancellationToken;

use auohp_core::transcription::TranscriptionResult;

use super::source::TranscribeSource;
use super::worker::{self, WorkerError};

/// `{interview_id}:{nanoid}`.
pub type JobId = String;

fn mint_id(source: &TranscribeSource) -> JobId {
    let suffix = nanoid::nanoid!(8);
    format!("{}:{}", source.interview_id(), suffix)
}

const EVENT_BUFFER: usize = 64;

/// Wire-format payloads. `Done` carries the full `TranscriptionResult`, which
/// `broadcast` clones per receiver.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Started {
        id: JobId,
    },
    Stage {
        id: JobId,
        message: String,
    },
    Done {
        id: JobId,
        result: TranscriptionResult,
    },
    Cancelled {
        id: JobId,
    },
    Failed {
        id: JobId,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Status {
    Idle,
    Running { id: JobId, source: TranscribeSource },
}

#[derive(Debug, Error, Serialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum SubmitError {
    #[error("another job is already running: {id}")]
    Busy { id: JobId },
}

#[derive(Debug, Error, Serialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum CancelError {
    #[error("no job is currently running")]
    NotRunning,
    #[error("id mismatch: requested {requested}, running {running}")]
    IdMismatch { requested: JobId, running: JobId },
}

struct Running {
    id: JobId,
    source: TranscribeSource,
    cancel: CancellationToken,
    events: broadcast::Sender<Event>,
}

pub struct SubmitOutcome {
    pub id: JobId,
    pub events: broadcast::Receiver<Event>,
}

pub struct Registry {
    slot: Mutex<Option<Running>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            slot: Mutex::new(None),
        }
    }

    pub async fn status(&self) -> Status {
        match self.slot.lock().await.as_ref() {
            Some(r) => Status::Running {
                id: r.id.clone(),
                source: r.source.clone(),
            },
            None => Status::Idle,
        }
    }

    /// A fresh subscriber if a job is running.
    pub async fn subscribe(&self) -> Option<broadcast::Receiver<Event>> {
        self.slot
            .lock()
            .await
            .as_ref()
            .map(|r| r.events.subscribe())
    }

    /// Fires the cancel token and returns without waiting for the worker.
    pub async fn cancel(&self, id: &str) -> Result<(), CancelError> {
        let slot = self.slot.lock().await;
        let running = slot.as_ref().ok_or(CancelError::NotRunning)?;
        if running.id != id {
            return Err(CancelError::IdMismatch {
                requested: id.into(),
                running: running.id.clone(),
            });
        }
        running.cancel.cancel();
        Ok(())
    }

    /// Reserves the slot and spawns the worker plus cleanup wrapper. Takes
    /// `Arc<Self>` so the `'static` wrapper can clear the slot afterwards.
    pub async fn submit(
        self: &Arc<Self>,
        source: TranscribeSource,
    ) -> Result<SubmitOutcome, SubmitError> {
        let mut slot = self.slot.lock().await;
        if let Some(running) = slot.as_ref() {
            return Err(SubmitError::Busy {
                id: running.id.clone(),
            });
        }

        let id = mint_id(&source);
        let cancel = CancellationToken::new();
        let (events, _) = broadcast::channel::<Event>(EVENT_BUFFER);

        // Subscribe before the lock drops so the caller can't miss `Started`;
        // broadcast receivers get no history.
        let outcome_rx = events.subscribe();

        slot.replace(Running {
            id: id.clone(),
            source: source.clone(),
            cancel: cancel.clone(),
            events: events.clone(),
        });
        drop(slot);

        // Runs the worker, emits the terminal event, clears the slot.
        let registry = Arc::clone(self);
        let id_for_worker = id.clone();
        let source_for_worker = source.clone();
        let events_for_worker = events.clone();
        let cancel_for_worker = cancel.clone();

        tokio::spawn(async move {
            let _ = events_for_worker.send(Event::Started {
                id: id_for_worker.clone(),
            });

            let result = worker::run(
                source_for_worker,
                cancel_for_worker,
                events_for_worker.clone(),
                id_for_worker.clone(),
            )
            .await;

            let terminal = match result {
                Ok(transcription) => Event::Done {
                    id: id_for_worker.clone(),
                    result: transcription,
                },
                Err(e) if matches!(e, WorkerError::Cancelled) => Event::Cancelled {
                    id: id_for_worker.clone(),
                },
                Err(e) => Event::Failed {
                    id: id_for_worker.clone(),
                    message: e.to_string(),
                },
            };
            let _ = events_for_worker.send(terminal);

            let mut slot = registry.slot.lock().await;
            if slot.as_ref().is_some_and(|r| r.id == id_for_worker) {
                slot.take();
            }
        });

        Ok(SubmitOutcome {
            id,
            events: outcome_rx,
        })
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

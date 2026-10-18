//! Single-slot job registry: at most one transcription runs at a time.
//!
//! The slot holds a `Running` --- a bundle of *handles* to the live
//! future (cancel token, broadcast sender, id, source metadata). The
//! future itself runs ownerlessly on tokio's task pool; we never keep
//! its `JoinHandle`. Cleanup happens in a wrapper future spawned at
//! submit time, which emits a terminal `Event` and clears the slot.

use std::sync::Arc;

use serde::Serialize;
use thiserror::Error;
use tokio::sync::{Mutex, broadcast};
use tokio_util::sync::CancellationToken;

use auohp_core::transcription::TranscriptionResult;

use super::source::TranscribeSource;
use super::worker::{self, WorkerError};

/// `{interview_id}:{nanoid}`. A `String` newtype would be tidier but
/// adds wiring (Display, Serialize, FromStr) without buying anything
/// at this size; promote it later if pattern-matching on the prefix
/// becomes interesting.
pub type JobId = String;

fn mint_id(source: &TranscribeSource) -> JobId {
    let suffix = nanoid::nanoid!(8);
    format!("{}:{}", source.interview_id(), suffix)
}

/// Channel buffer for progress events. 64 is large enough for a normal
/// interview's segment cadence with slack for slow subscribers; lossy
/// behavior on overflow lives at the producer side once we wire
/// `set_segment_callback_safe_lossy` in the whisper bridge.
const EVENT_BUFFER: usize = 64;

/// Wire-format payloads. `serde(tag = "kind")` produces self-describing
/// JSON so the webview can `switch (event.kind)` without sniffing.
///
/// `Done` carries the `TranscriptionResult` inline. broadcast::Sender
/// stores `T` and clones to each receiver on `recv`, so a multi-MB
/// transcript means 2--3 clones (Tauri bridge + maybe SSE + the channel
/// itself). Acceptable at this scale; revisit if memory pressure
/// shows up in profiling.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Started {
        id: JobId,
    },
    /// Coarse-grained progress marker. Per-segment streaming arrives
    /// when the worker plumbs `CancellationToken` and `broadcast::Sender`
    /// into auohp-core's whisper module via callbacks.
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

/// The handle bundle the registry stores while a job is active.
/// Field-private: external callers reach the wires through `Registry`
/// methods (`subscribe`, `cancel`, `status`).
struct Running {
    id: JobId,
    source: TranscribeSource,
    cancel: CancellationToken,
    events: broadcast::Sender<Event>,
}

/// Returned from `submit` so the caller (Tauri command or HTTP handler)
/// can wire its own forwarder onto the event stream without re-locking
/// the slot.
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

    /// Snapshot of the slot for the `/status` route and the matching
    /// Tauri command. Cheap-to-clone variants only; no live channel
    /// handles leak out.
    pub async fn status(&self) -> Status {
        match self.slot.lock().await.as_ref() {
            Some(r) => Status::Running {
                id: r.id.clone(),
                source: r.source.clone(),
            },
            None => Status::Idle,
        }
    }

    /// Returns a fresh subscriber if a job is running, else `None`.
    /// Each subscriber is independent --- the SSE handler and the
    /// Tauri-event bridge each call this and own their own receiver.
    pub async fn subscribe(&self) -> Option<broadcast::Receiver<Event>> {
        self.slot
            .lock()
            .await
            .as_ref()
            .map(|r| r.events.subscribe())
    }

    /// Flip the cancel token. Returns immediately; the worker observes
    /// the token at its next poll point and exits cooperatively.
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

    /// Reserve the slot, mint an id, spawn the worker + cleanup wrapper,
    /// and hand back the id and a fresh event receiver.
    ///
    /// `Arc<Self>` is required because the cleanup wrapper future has
    /// `'static` lifetime and needs to lock the slot after the worker
    /// returns. The arc gives it that ownership without borrowing.
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

        // Hand the caller their subscriber *before* the slot lock drops,
        // so the very first event (`Started`, sent below from the worker)
        // can't race past them. broadcast subscribers see all messages
        // sent after `subscribe()` was called; they don't see history.
        let outcome_rx = events.subscribe();

        slot.replace(Running {
            id: id.clone(),
            source: source.clone(),
            cancel: cancel.clone(),
            events: events.clone(),
        });
        drop(slot);

        // Wrapper: runs the worker, emits the terminal event, and
        // clears the slot. This is what makes the slot's "occupied"
        // state line up with the future's "still running" state.
        let registry = Arc::clone(self);
        let id_for_worker = id.clone();
        let source_for_worker = source.clone();
        let events_for_worker = events.clone();
        let cancel_for_worker = cancel.clone();

        // "Submit is always called from inside a task already running on
        // Tokio's global runtime"
        tokio::spawn(async move {
            // The first event subscribers see. Sent from inside the
            // wrapper rather than `submit` because we want it to come
            // *after* the receiver has been handed back, so its arrival
            // is unambiguous evidence the worker has started.
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

            // Vacate the slot, but only if we still own it. The id check
            // is paranoia for now (reject-until-cancelled means the slot
            // can't change underneath us), but it's the kind of guard
            // that earns its keep when invariants drift.
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

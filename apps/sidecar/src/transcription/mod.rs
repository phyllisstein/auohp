//! Transcription job system.
//!
//! Single-slot, in-memory pipeline: one job runs at a time, new submits
//! while a job is active are rejected. The registry holds *handles* to the
//! running future (cancel token, broadcast sender, id, source) --- not a
//! "job record." The work itself is the future spawned at submit time.
//!
//! See `registry.rs` for the slot mechanics, `worker.rs` for the
//! pipeline-invocation wrapper, and `source.rs` for the input enum that
//! lets local files and (future) remote URLs converge on one submit path.

pub mod registry;
pub mod source;
pub mod worker;

pub use registry::{CancelError, Event, JobId, Registry, Status, SubmitError, SubmitOutcome};
pub use source::TranscribeSource;

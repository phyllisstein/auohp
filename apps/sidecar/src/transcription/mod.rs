//! Transcription job system.
//!
//! Single-slot, in-memory pipeline: one job at a time; submits while a job is
//! active are rejected.

pub mod registry;
pub mod source;
pub mod worker;

pub use registry::{CancelError, Event, JobId, Registry, Status, SubmitError, SubmitOutcome};
pub use source::TranscribeSource;

//! The convergence type: where a transcription's input bytes come from.
//!
//! Both the Tauri command path (local files) and the HTTP route path
//! (currently local files; later, remote URLs like Vimeo) build a
//! `TranscribeSource` and hand it to `Registry::submit`. The submit
//! function does not care which entry point produced the value.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// `#[non_exhaustive]` so adding `Vimeo { url: Url }` later is non-breaking
/// for the desktop crate's external consumers (other crates in the
/// workspace, or the Tauri webview through the IPC boundary).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[non_exhaustive]
pub enum TranscribeSource {
    /// A file already accessible to the desktop process at `path`.
    /// `interview_id` is the AUOHP interview number used to prefix the
    /// minted job id, so events on the wire carry an obvious correlator
    /// back to the interview the user is working on.
    Local {
        path: PathBuf,
        interview_id: String,
    },
}

impl TranscribeSource {
    /// The interview number used to prefix the job id. Each variant must
    /// be able to answer this --- Vimeo URLs will carry it as a query
    /// param or be looked up from the URL when that variant lands.
    pub fn interview_id(&self) -> &str {
        match self {
            TranscribeSource::Local { interview_id, .. } => interview_id,
        }
    }
}

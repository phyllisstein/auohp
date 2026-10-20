//! Where a transcription's input comes from; both the Tauri command and the
//! HTTP route build one and hand it to `Registry::submit`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[non_exhaustive]
pub enum TranscribeSource {
    /// A file on disk. `interview_id` is the AUOHP interview number that
    /// prefixes the minted job id.
    Local {
        path: PathBuf,
        interview_id: String,
    },
}

impl TranscribeSource {
    /// The interview number that prefixes the job id.
    pub fn interview_id(&self) -> &str {
        match self {
            TranscribeSource::Local { interview_id, .. } => interview_id,
        }
    }
}

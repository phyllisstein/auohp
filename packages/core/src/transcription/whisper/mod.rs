//! Whisper ASR via whisper-rs (whisper.cpp FFI).
//!
//! Model files are downloaded by `scripts/download-models.py` into `$MODELS_DIR`;
//! the pipeline resolves paths, so there is no network I/O at inference time.
//!
//! ## Voice Activity Detection
//!
//! `FullParams::enable_vad` is ignored by `whisper_full_with_state`, the entry
//! point `WhisperState::full()` calls; whisper.cpp reads `params.vad` only in
//! `whisper_full` and `whisper_full_parallel`. [`vad::apply_vad`] therefore runs
//! Silero here and glues the speech regions with 0.1 s of silence, so Whisper's
//! timestamps refer to the filtered timeline. [`vad::VadTimeline`] maps every
//! segment and word time back to the original recording.
//!
//! ## Word-level timestamps
//!
//! `set_token_timestamps(true)` enables DTW over the cross-attention heads,
//! published as `whisper_token_data::t_dtw`: a centisecond *instant* per token,
//! unlike the coarse `t0`/`t1` interval. A word's end is therefore the start of
//! the next word, and only the last word takes the segment's end.

pub mod alignment;
pub mod model;
pub mod runner;
pub mod vad;

pub use model::{load_model, MODEL_FILE};
pub use runner::transcribe;
pub use vad::VAD_MODEL_FILE;

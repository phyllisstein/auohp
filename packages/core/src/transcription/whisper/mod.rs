//! Whisper ASR via whisper-rs (whisper.cpp FFI).
//!
//! whisper.cpp handles the full decode loop---mel spectrogram, encoder,
//! autoregressive decoder, and timestamp extraction---in highly optimised C++.
//! This module is a thin Rust wrapper decomposed into focused submodules:
//!   1. [`vad`]: Voice Activity Detection filtering via Silero VAD and timeline remapping.
//!   2. [`model`]: Whisper model context loading, lifecycle management, and DTW arena setup.
//!   3. [`alignment`]: Dynamic Time Warping token timestamping and byte-level BPE word assembly.
//!   4. [`runner`]: Inference execution loop transforming audio samples into timestamped segments.
//!
//! ## Model management
//!
//! Both ggml files are downloaded once by `scripts/download-models.sh` into
//! `$MODELS_DIR` (default `/opt/auohp/models`). The pipeline resolves paths
//! and passes them here---no network I/O at inference time.
//!
//! ## Voice Activity Detection---why it is applied here rather than by whisper.cpp
//!
//! Setting `FullParams::enable_vad(true)` does nothing on this code path.
//! whisper.cpp reads `params.vad` only in `whisper_full` and `whisper_full_parallel`;
//! `whisper_full_with_state`---the entry point `WhisperState::full()` calls---never
//! looks at it.
//!
//! Rather than switch to the context-owned `whisper_full` (which would give up
//! the caller-managed state this pipeline is built around), [`apply_vad`] runs
//! Silero explicitly and hands Whisper the filtered audio. That reproduces
//! whisper.cpp's own construction (`whisper.cpp:6641-6700`): concatenate the
//! detected speech regions, extend all but the last by `samples_overlap`, and
//! glue them with 0.1 s of silence.
//!
//! The cost of doing it ourselves is that **Whisper's timestamps then refer to
//! the filtered timeline**, which is shorter than the real recording and has the
//! silences removed. whisper.cpp keeps an internal `vad_mapping_table` for this;
//! we keep [`VadTimeline`] and map every segment and word time back before the
//! result leaves this module. Skipping that step would produce a transcript whose
//! timings drift further out of sync the more silence the recording contains---wrong
//! in a way that looks plausible right up until someone scrubs the video.
//!
//! ## Word-level timestamps
//!
//! `set_token_timestamps(true)` enables DTW: whisper.cpp runs Dynamic Time
//! Warping over its cross-attention heads to pin each BPE token to a single
//! centisecond-resolution instant, published as `whisper_token_data::t_dtw`.
//!
//! Note the shape difference, because it drives the whole word-assembly design.
//! `t0`/`t1` are an *interval* per token, produced by the coarse fallback
//! heuristic (~1 s resolution); `t_dtw` is a *point*. Assembling words from
//! points means a word's end is not carried by its own tokens at all---it is
//! the start of whatever comes next. So the assembler zips each word against its
//! successor rather than folding intervals, and the final word is the only one
//! that has to reach for the segment's end time.

pub mod alignment;
pub mod model;
pub mod runner;
pub mod vad;

#[allow(unused_imports)]
pub use alignment::{assemble_words, collect_words, round_to, strip_turn_dash};
#[allow(unused_imports)]
pub use model::{load_model, WhisperModel, MODEL_FILE};
#[allow(unused_imports)]
pub use runner::{transcribe, WhisperSegment};
#[allow(unused_imports)]
pub use vad::{apply_vad, VadRegion, VadTimeline, VAD_GLUE_SECONDS, VAD_MODEL_FILE};

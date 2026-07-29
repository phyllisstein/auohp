//! Tunable pipeline parameters, in one serialisable place.
//!
//! Everything the tuning campaign is allowed to vary lives here. The point is
//! that a run is described entirely by a `TranscribeConfig` plus a git SHA: if a
//! parameter can only be changed by editing `whisper.rs`, then the run manifest
//! no longer describes the code that produced the result, and the experiment
//! ledger quietly stops being reproducible.
//!
//! `Default` reproduces the pipeline's behaviour as of the baseline runs, so
//! `TranscribeConfig::default()` is always the parent of the search tree.

use serde::{Deserialize, Serialize};

/// Sinc interpolation kind, mirrored from `rubato` so it can be named in JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Interpolation {
    Nearest,
    #[default]
    Linear,
    Quadratic,
    Cubic,
}

/// Audio decode and resampling parameters.
///
/// These only bite when the input is not already 16 kHz mono. A WAV fixture at
/// 16 kHz mono skips this path entirely, which is exactly why the WAV cannot be
/// used to validate it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioConfig {
    pub resample_chunk: usize,
    pub sinc_len: usize,
    pub f_cutoff: f32,
    pub oversampling_factor: usize,
    pub interpolation: Interpolation,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            resample_chunk: 4096,
            sinc_len: 256,
            f_cutoff: 0.95,
            oversampling_factor: 256,
            interpolation: Interpolation::Linear,
        }
    }
}

/// Silero VAD parameters.
///
/// Only `max_speech_duration` was ever set before the baseline; the other five
/// sat at whisper.cpp's defaults. They are surfaced here because segment
/// boundaries quantising to VAD window edges was one of the observed defects,
/// and it cannot be investigated through a knob that does not exist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VadConfig {
    pub enabled: bool,
    pub threshold: Option<f32>,
    pub min_speech_duration_ms: Option<i32>,
    pub min_silence_duration_ms: Option<i32>,
    pub max_speech_duration_s: Option<f32>,
    pub speech_pad_ms: Option<i32>,
    pub samples_overlap_s: Option<f32>,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: None,
            min_speech_duration_ms: None,
            min_silence_duration_ms: None,
            max_speech_duration_s: Some(60.0),
            speech_pad_ms: None,
            samples_overlap_s: None,
        }
    }
}

/// Whisper decoding parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecodeConfig {
    pub language: Option<String>,
    pub beam_size: i32,
    pub patience: f32,
    pub entropy_thold: f32,
    pub logprob_thold: Option<f32>,
    pub no_speech_thold: Option<f32>,
    pub temperature: Option<f32>,
    pub temperature_inc: Option<f32>,
    /// **Nearly inert — not the context switch it appears to be.**
    ///
    /// whisper.cpp reads this once, at entry to `whisper_full_with_state`
    /// (`whisper.cpp:6900`), to clear the prompt-history buffers. The per-window
    /// refill at `7590-7601` has no `no_context` guard and pushes decoded tokens
    /// into the rolling buffer unconditionally. Since `transcribe` creates a
    /// fresh state per file, those buffers are already empty when the clear runs,
    /// so it is a no-op and rolling context accumulates within a file either way.
    ///
    /// Measured: toggling it produced byte-identical output (run `007-c1`).
    /// The real switch is `n_max_text_ctx`, which gates the take at `7090`/`7094`.
    pub no_context: bool,
    pub suppress_nst: Option<bool>,
    /// Seeds the decoder with domain vocabulary --- but **its reach is about the
    /// first 60 seconds of a file**, not the whole of it, until whisper-rs
    /// exposes `carry_initial_prompt`.
    ///
    /// whisper-rs 0.16 never sets `carry_initial_prompt`, so the prompt takes the
    /// `else` branch at `whisper.cpp:6939` into `prompt_past1` --- the *rolling*
    /// buffer, not the static one. The take at `7106` keeps only the last
    /// `max_prompt_ctx - 1` = 223 tokens (`min(n_max_text_ctx, n_text_ctx/2)`,
    /// large-v3), so once ~223 tokens have been decoded the prompt is evicted
    /// from the front of the window. That is roughly two decode windows.
    ///
    /// So on a 34-minute interview this primes the opening and then vanishes.
    /// Making it a real axis needs `carry_initial_prompt`, which would place the
    /// prompt in `prompt_past0` where it survives every window.
    pub initial_prompt: Option<String>,
    pub max_len: Option<i32>,
    pub split_on_word: Option<bool>,
    pub token_timestamps: bool,
}

impl Default for DecodeConfig {
    fn default() -> Self {
        Self {
            language: Some("en".into()),
            beam_size: 5,
            patience: 1.0,
            entropy_thold: 3.0,
            logprob_thold: None,
            no_speech_thold: None,
            temperature: None,
            temperature_inc: None,
            no_context: true,
            suppress_nst: None,
            initial_prompt: None,
            max_len: None,
            split_on_word: None,
            token_timestamps: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TranscribeConfig {
    pub audio: AudioConfig,
    pub decode: DecodeConfig,
    pub vad: VadConfig,
}

impl TranscribeConfig {
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

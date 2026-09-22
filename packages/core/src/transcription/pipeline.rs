//! Transcription pipeline: orchestrates audio decoding --> VAD --> Whisper ASR
//! --> speaker diarization into word-timed, speaker-labeled segments.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::audio;
use super::config::TranscribeConfig;
use super::diarize::{self, DiarizedSegment};
use super::types::*;
use super::whisper;

const WHISPER_MODEL_FILE: &str = "ggml-large-v3.bin";
const VAD_MODEL_FILE: &str = "ggml-silero-v6.2.0.bin";
const SEGMENTATION_MODEL_FILE: &str = "pyannote-segmentation-3.0.onnx";
const EMBEDDING_MODEL_FILE: &str = "wespeaker_en_voxceleb_ECAPA1024.onnx";
const DEFAULT_MODELS_DIR: &str = "/opt/auohp/models";

/// Run the transcription pipeline on an audio/video file.
///
/// Returns Whisper segments with per-word timestamps from DTW, each labeled
/// with a speaker turn from diarization when `cfg.diarize.enabled` (the
/// default). Diarization only assigns turn-level labels (`SPEAKER_00`,
/// `SPEAKER_01`, ...) --- mapping those labels to real names is still a job
/// for the manual labeling UI.
///
/// This is blocking (Whisper and diarization are both CPU-bound). Call from
/// `tokio::task::spawn_blocking` to avoid stalling the async runtime.
pub fn run(input_path: &Path) -> Result<TranscriptionResult> {
    run_with(input_path, &TranscribeConfig::default())
}

/// Run the pipeline under an explicit configuration.
///
/// Every tunable knob arrives through `cfg`, so a run is described completely by
/// this value plus a git SHA. That is what keeps the experiment ledger honest:
/// a parameter reachable only by editing source would make the recorded manifest
/// a description of something other than the code that ran.
pub fn run_with(input_path: &Path, cfg: &TranscribeConfig) -> Result<TranscriptionResult> {
    let decoded = audio::decode_file_with(input_path, &cfg.audio)
        .with_context(|| format!("failed to decode {}", input_path.display()))?;

    tracing::debug!(
        "Audio: {} Hz / {} ch source -> {} samples at {} Hz mono ({:.1}s){}",
        decoded.source_sample_rate,
        decoded.source_channels,
        decoded.samples.len(),
        decoded.sample_rate,
        decoded.samples.len() as f64 / decoded.sample_rate as f64,
        if decoded.source_sample_rate == decoded.sample_rate && decoded.source_channels == 1 {
            "  [transform chain bypassed]"
        } else {
            ""
        }
    );

    // Load the model *after* decoding, and keep it that way.
    //
    // This looks like incidental ordering and is a memory constraint. Decoding a
    // 2.6-hour stereo master peaks around 3.8 GB; the model side --- weights
    // 3094 MB, kv 343 MB, compute buffers 861 MB, DTW arena 128 MB --- is about
    // 4.4 GB. Sequenced, peak is roughly 5.0 GB because the interleaved buffer is
    // freed before the weights arrive. Overlapped, it is 8.2 GB.
    //
    // On CUDA that distinction is invisible: the model lives in VRAM and the
    // decode buffer in host RAM, two separate pools. Inference here is on Apple
    // Silicon, where unified memory means one pool, and 8 GB is a shipping
    // configuration. So hoisting this above the decode to fail fast on a missing
    // model --- a reasonable thing to want --- would cost 3.2 GB of headroom on
    // the hardware that matters. Validate the model *path* early if that is the
    // goal; do not load the weights early.
    //
    // All models live under $MODELS_DIR, pre-downloaded by download-models.sh.
    let models_dir = PathBuf::from(
        std::env::var("MODELS_DIR").unwrap_or_else(|_| DEFAULT_MODELS_DIR.to_string()),
    );

    let mut whisper_model = whisper::load_model(
        &models_dir.join(WHISPER_MODEL_FILE),
        &models_dir.join(VAD_MODEL_FILE),
    )?;
    let whisper_segments = whisper::transcribe(&mut whisper_model, &decoded.samples, cfg)?;

    let diarized = if cfg.diarize.enabled {
        diarize::diarize(
            &decoded.samples,
            decoded.sample_rate,
            &models_dir.join(SEGMENTATION_MODEL_FILE),
            &models_dir.join(EMBEDDING_MODEL_FILE),
            cfg.diarize.max_speakers,
        )?
    } else {
        Vec::new()
    };

    let segments: Vec<Segment> = whisper_segments
        .iter()
        .map(|s| Segment {
            speaker: best_speaker_overlap(s.start, s.end, &diarized),
            text: s.text.clone(),
            start_time: s.start,
            end_time: s.end,
            words: s.words.clone(),
        })
        .collect();

    Ok(TranscriptionResult { segments })
}

/// Assign a speaker label to a Whisper segment by maximum temporal overlap
/// with diarized turns. Returns `None` (rather than guessing) when there's no
/// diarization data at all, or no diarized turn overlaps this segment ---
/// which is the caption editor's existing signal that a speaker still needs a
/// human label.
fn best_speaker_overlap(start: f64, end: f64, diarized: &[DiarizedSegment]) -> Option<String> {
    diarized
        .iter()
        .map(|d| {
            let overlap = (end.min(d.end) - start.max(d.start)).max(0.0);
            (overlap, &d.speaker)
        })
        .filter(|(overlap, _)| *overlap > 0.0)
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, speaker)| speaker.clone())
}
